//! ffmpeg engine (external process via `crate::proc`). OWNER: Task C.
//! Every run: `-hide_banner -nostdin -y -loglevel error -progress pipe:1 -nostats`,
//! progress parsed from `out_time_us=` against the duration from ffprobe
//! (fall back to parsing `Duration:` from `ffmpeg -i` stderr when ffprobe is missing).
//! Cancel kills the process tree; the caller deletes the partial output.
//!
//! The muxer is always passed explicitly (`-f`), so the output file name/extension (the executor
//! hands in a temp path) never influences format selection.

use crate::jobs::{CompressLevel, ConvertOptions};
use crate::proc;
use crate::{CancelToken, Error, Result};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub struct Ffmpeg {
    pub ffmpeg: PathBuf,
    pub ffprobe: Option<PathBuf>,
}

/// Parsed `-progress` key/value block. Returns `Some(out_time_seconds)` when the block has it.
/// `out_time_us` and `out_time_ms` are both microseconds in current ffmpeg.
pub fn parse_progress_line(line: &str) -> Option<f64> {
    let (key, value) = line.trim().split_once('=')?;
    if key != "out_time_us" && key != "out_time_ms" {
        return None;
    }
    let micros: i64 = value.trim().parse().ok()?;
    (micros >= 0).then(|| micros as f64 / 1_000_000.0)
}

/// Format id (`mp4`, `mp3`, ...) for a file extension, when ffmpeg can write it.
fn container_of(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "mp4" | "m4v" => "mp4",
        "mov" | "qt" => "mov",
        "mkv" => "mkv",
        "webm" => "webm",
        "avi" => "avi",
        "mp3" => "mp3",
        "m4a" => "m4a",
        "opus" => "opus",
        "ogg" | "oga" => "ogg",
        "flac" => "flac",
        "wav" => "wav",
        "m4r" => "m4r",
        "wmv" => "wmv",
        "flv" => "flv",
        "mpg" | "mpeg" | "mpe" => "mpg",
        "3gp" | "3gpp" => "3gp",
        "ts" | "m2ts" | "mts" => "ts",
        "aac" => "aac",
        "wma" => "wma",
        "aiff" | "aif" | "aifc" => "aiff",
        "ac3" => "ac3",
        "mka" => "mka",
        _ => return None,
    })
}

fn is_video_container(id: &str) -> bool {
    matches!(
        id,
        "mp4" | "mov" | "mkv" | "webm" | "avi" | "wmv" | "flv" | "mpg" | "3gp" | "ts"
    )
}

fn muxer(id: &str) -> &'static str {
    match id {
        "mp4" => "mp4",
        "mov" => "mov",
        "mkv" => "matroska",
        "webm" => "webm",
        "avi" => "avi",
        "gif" => "gif",
        "mp3" => "mp3",
        "m4a" | "m4r" => "ipod",
        "opus" => "opus",
        "ogg" => "ogg",
        "flac" => "flac",
        "wmv" | "wma" => "asf",
        "flv" => "flv",
        "mpg" => "mpeg",
        "3gp" => "3gp",
        "ts" => "mpegts",
        "aac" => "adts",
        "aiff" => "aiff",
        "ac3" => "ac3",
        "mka" => "matroska",
        _ => "wav",
    }
}

fn strs(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_string()).collect()
}

/// x264 CRF for a 1..=100 quality (75 == the default 23, higher quality = lower CRF).
fn x264_crf(quality: Option<u8>) -> u32 {
    match quality {
        None => 23,
        Some(q) => 18 + u32::from(100 - q.clamp(1, 100)) / 5,
    }
}

/// Video encoder arguments for a container. `crf` overrides the quality mapping.
fn video_codec_args(id: &str, quality: Option<u8>, crf: Option<u32>) -> Vec<String> {
    match id {
        "webm" => {
            let c = crf.unwrap_or_else(|| x264_crf(quality) + 9).to_string();
            let mut v = strs(&["-c:v", "libvpx-vp9", "-crf"]);
            v.push(c);
            v.extend(strs(&[
                "-b:v",
                "0",
                "-row-mt",
                "1",
                "-deadline",
                "good",
                "-cpu-used",
                "5",
                "-pix_fmt",
                "yuv420p",
            ]));
            v
        }
        "avi" | "wmv" | "mpg" => {
            let codec = match id {
                "wmv" => "wmv2",
                "mpg" => "mpeg2video",
                _ => "mpeg4",
            };
            let q = 2 + u32::from(100 - quality.unwrap_or(75).clamp(1, 100)) / 10;
            let mut v = strs(&["-c:v", codec, "-q:v"]);
            v.push(q.to_string());
            if id == "mpg" {
                // mpeg2video only accepts a handful of frame rates; 25 is always valid.
                v.extend(strs(&["-r", "25"]));
            }
            v.extend(strs(&["-pix_fmt", "yuv420p"]));
            v
        }
        _ => x264_args(
            crf.unwrap_or_else(|| x264_crf(quality)),
            matches!(id, "mp4" | "mov"),
        ),
    }
}

fn x264_args(crf: u32, faststart: bool) -> Vec<String> {
    let mut v = strs(&["-c:v", "libx264", "-crf"]);
    v.push(crf.to_string());
    v.extend(strs(&["-preset", "veryfast", "-pix_fmt", "yuv420p"]));
    if faststart {
        v.extend(strs(&["-movflags", "+faststart"]));
    }
    v
}

/// Even dimensions (yuv420p encoders reject odd sizes).
const EVEN_SCALE: &str = "scale=trunc(iw/2)*2:trunc(ih/2)*2";

/// Audio encoder arguments for a container. `bitrate_k` overrides the default bitrate.
fn audio_codec_args(id: &str, bitrate_k: Option<u32>) -> Vec<String> {
    let br = |default: u32| format!("{}k", bitrate_k.unwrap_or(default));
    match id {
        "mp4" | "mov" | "mkv" | "flv" | "3gp" | "ts" => {
            vec!["-c:a".into(), "aac".into(), "-b:a".into(), br(160)]
        }
        "m4a" | "m4r" | "aac" => vec!["-c:a".into(), "aac".into(), "-b:a".into(), br(192)],
        "webm" | "opus" | "ogg" | "mka" => {
            vec!["-c:a".into(), "libopus".into(), "-b:a".into(), br(128)]
        }
        // wmav2 and mp2 encode at most stereo.
        "wmv" | "wma" => strs(&["-c:a", "wmav2", "-ac", "2", "-b:a", &br(192)]),
        "mpg" => strs(&["-c:a", "mp2", "-ac", "2", "-b:a", &br(192)]),
        "ac3" => strs(&["-c:a", "ac3", "-b:a", &br(192)]),
        "aiff" => strs(&["-c:a", "pcm_s16be"]),
        "avi" => vec!["-c:a".into(), "libmp3lame".into(), "-b:a".into(), br(160)],
        "mp3" => match bitrate_k {
            Some(k) => vec![
                "-c:a".into(),
                "libmp3lame".into(),
                "-b:a".into(),
                format!("{k}k"),
            ],
            None => strs(&["-c:a", "libmp3lame", "-q:a", "2"]),
        },
        "flac" => strs(&["-c:a", "flac"]),
        _ => strs(&["-c:a", "pcm_s16le"]),
    }
}

/// Scale filter: even dimensions, optionally limited to `max_width` x `max_height`, never upscaling.
fn scale_filter(options: &ConvertOptions) -> String {
    let w = options
        .max_width
        .map_or("iw".to_string(), |w| format!("min(iw,{w})"));
    let h = options
        .max_height
        .map_or("ih".to_string(), |h| format!("min(ih,{h})"));
    format!("scale=w='{w}':h='{h}':force_original_aspect_ratio=decrease:force_divisible_by=2")
}

/// `atempo` chain for 0.25..=4.0 (each stage within 0.5..=2.0).
fn atempo_chain(factor: f32) -> String {
    let mut f = f64::from(factor);
    let mut stages: Vec<f64> = Vec::new();
    while f > 2.0 {
        stages.push(2.0);
        f /= 2.0;
    }
    while f < 0.5 {
        stages.push(0.5);
        f /= 0.5;
    }
    stages.push(f);
    stages
        .iter()
        .map(|s| format!("atempo={}", trim_float(*s)))
        .collect::<Vec<_>>()
        .join(",")
}

fn trim_float(v: f64) -> String {
    let s = format!("{v:.5}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn secs(ms: u64) -> String {
    format!("{:.3}", ms as f64 / 1000.0)
}

fn os(items: Vec<String>) -> Vec<OsString> {
    items.into_iter().map(OsString::from).collect()
}

/// `[pre] -i input [body] -f muxer output`
fn assemble(
    pre: Vec<String>,
    input: &Path,
    body: Vec<String>,
    mux: &str,
    output: &Path,
) -> Vec<OsString> {
    let mut args = os(pre);
    args.push("-i".into());
    args.push(input.into());
    args.extend(os(body));
    args.push("-f".into());
    args.push(mux.into());
    args.push(output.into());
    args
}

const MAP_AV: [&str; 4] = ["-map", "0:v:0", "-map", "0:a:0?"];

/// iPhone ringtones may be at most 40 seconds long.
pub const RINGTONE_MAX_SECS: u32 = 40;

/// Arguments for a re-encoding format conversion to `to`.
fn convert_args(input: &Path, to: &str, output: &Path, options: &ConvertOptions) -> Vec<OsString> {
    let mut body = Vec::new();
    if is_video_container(to) {
        body.extend(strs(&MAP_AV));
        body.extend(strs(&["-vf"]));
        body.push(scale_filter(options));
        body.extend(video_codec_args(to, options.quality, None));
        body.extend(audio_codec_args(to, None));
    } else {
        body.extend(strs(&["-vn", "-map", "0:a:0"]));
        if to == "m4r" {
            body.push("-t".into());
            body.push(RINGTONE_MAX_SECS.to_string());
        }
        body.extend(audio_codec_args(to, None));
    }
    assemble(Vec::new(), input, body, muxer(to), output)
}

/// First subtitle stream (ASS/SSA input) -> SubRip or WebVTT.
/// `legacy_8bit`: the file has no BOM and is not UTF-8, so it is read as Windows-1254 (as the
/// in-process SRT/VTT reader does); ffmpeg otherwise assumes UTF-8.
fn subtitle_args(input: &Path, to: &str, output: &Path, legacy_8bit: bool) -> Vec<OsString> {
    let (codec, mux) = if to == "vtt" {
        ("webvtt", "webvtt")
    } else {
        ("srt", "srt")
    };
    let pre = if legacy_8bit {
        strs(&["-sub_charenc", "CP1254"])
    } else {
        Vec::new()
    };
    let body = strs(&["-map", "0:s:0", "-c:s", codec]);
    assemble(pre, input, body, mux, output)
}

/// Stream copy into mp4/mov/mkv (no re-encode).
fn copy_args(input: &Path, to: &str, output: &Path) -> Vec<OsString> {
    let mut body = strs(&MAP_AV);
    body.extend(strs(&["-c", "copy"]));
    if matches!(to, "mp4" | "mov") {
        body.extend(strs(&["-movflags", "+faststart"]));
    }
    assemble(Vec::new(), input, body, muxer(to), output)
}

/// GIF encoding parameters.
struct GifSpec {
    max_width: u32,
    max_height: Option<u32>,
    fps: u32,
    max_seconds: u32,
}

/// Shared by both passes so the palette matches the frames it is applied to.
fn gif_chain(spec: &GifSpec) -> String {
    let h = spec
        .max_height
        .map_or("ih".to_string(), |h| format!("min(ih,{h})"));
    format!(
        "fps={},scale=w='min(iw,{})':h='{h}':force_original_aspect_ratio=decrease:force_divisible_by=2:flags=lanczos",
        spec.fps, spec.max_width
    )
}

/// Pass 1: write a single palette PNG.
fn gif_palette_args(input: &Path, palette: &Path, spec: &GifSpec) -> Vec<OsString> {
    let body = vec![
        "-t".to_string(),
        spec.max_seconds.to_string(),
        "-vf".into(),
        format!("{},palettegen=stats_mode=diff", gif_chain(spec)),
        "-frames:v".into(),
        "1".into(),
        "-update".into(),
        "1".into(),
    ];
    assemble(Vec::new(), input, body, "image2", palette)
}

/// Pass 2: apply the palette (a second input, so no path appears inside the filter string).
fn gif_encode_args(input: &Path, palette: &Path, output: &Path, spec: &GifSpec) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec!["-i".into(), input.into(), "-i".into(), palette.into()];
    args.extend(os(vec![
        "-t".to_string(),
        spec.max_seconds.to_string(),
        "-filter_complex".into(),
        format!(
            "[0:v]{}[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=5",
            gif_chain(spec)
        ),
        "-an".into(),
        "-loop".into(),
        "0".into(),
        "-f".into(),
        "gif".into(),
    ]));
    args.push(output.into());
    args
}

fn compress_args(input: &Path, id: &str, output: &Path, level: CompressLevel) -> Vec<OsString> {
    let (crf, kbps) = match level {
        CompressLevel::Light => (26, 160),
        CompressLevel::Balanced => (29, 128),
        CompressLevel::Strong => (33, 96),
    };
    let mut body = Vec::new();
    if is_video_container(id) {
        let crf = if id == "webm" { crf + 8 } else { crf };
        body.extend(strs(&MAP_AV));
        body.extend(strs(&["-vf", EVEN_SCALE]));
        if id == "avi" {
            body.extend(x264_args(crf, false));
        } else {
            body.extend(video_codec_args(id, None, Some(crf)));
        }
        body.extend(audio_codec_args(id, Some(kbps)));
    } else {
        body.extend(strs(&["-vn", "-map", "0:a:0"]));
        match id {
            "flac" => body.extend(strs(&["-c:a", "flac", "-compression_level", "12"])),
            "wav" => {
                let rate = match level {
                    CompressLevel::Light => "44100",
                    CompressLevel::Balanced => "32000",
                    CompressLevel::Strong => "22050",
                };
                body.extend(strs(&["-c:a", "pcm_s16le", "-ar", rate]));
            }
            _ => body.extend(audio_codec_args(id, Some(kbps))),
        }
    }
    assemble(Vec::new(), input, body, muxer(id), output)
}

fn trim_args(input: &Path, id: &str, output: &Path, start_ms: u64, end_ms: u64) -> Vec<OsString> {
    let pre = vec!["-ss".to_string(), secs(start_ms)];
    let mut body = vec!["-t".to_string(), secs(end_ms - start_ms)];
    if is_video_container(id) {
        body.extend(strs(&MAP_AV));
        body.extend(strs(&["-vf", EVEN_SCALE]));
        body.extend(video_codec_args(id, None, None));
    } else {
        body.extend(strs(&["-vn", "-map", "0:a:0"]));
    }
    body.extend(audio_codec_args(id, None));
    assemble(pre, input, body, muxer(id), output)
}

fn mute_args(input: &Path, id: &str, output: &Path) -> Vec<OsString> {
    let body = strs(&["-map", "0:v:0", "-an", "-c:v", "copy"]);
    assemble(Vec::new(), input, body, muxer(id), output)
}

fn speed_args(input: &Path, id: &str, output: &Path, factor: f32) -> Vec<OsString> {
    let mut body = Vec::new();
    if is_video_container(id) {
        body.extend(strs(&MAP_AV));
        body.push("-vf".into());
        body.push(format!(
            "setpts=PTS/{},{EVEN_SCALE}",
            trim_float(f64::from(factor))
        ));
        // Keep every frame at its new timestamp instead of resampling to the source frame rate
        // (cfr resampling pads the tail and distorts the duration).
        body.extend(strs(&["-fps_mode", "passthrough"]));
        body.extend(video_codec_args(id, None, None));
    } else {
        body.extend(strs(&["-vn", "-map", "0:a:0"]));
    }
    body.push("-af".into());
    body.push(atempo_chain(factor));
    body.extend(audio_codec_args(id, None));
    assemble(Vec::new(), input, body, muxer(id), output)
}

fn crop_args(
    input: &Path,
    id: &str,
    output: &Path,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
) -> Vec<OsString> {
    let mut body = strs(&MAP_AV);
    body.push("-vf".into());
    body.push(format!("crop={}:{}:{}:{}", w & !1, h & !1, x & !1, y & !1));
    body.extend(video_codec_args(id, None, None));
    body.extend(strs(&["-c:a", "copy"]));
    assemble(Vec::new(), input, body, muxer(id), output)
}

/// Centre crop to `w:h`, as large as fits, even sizes (the crop filter centres by default).
fn crop_aspect_args(input: &Path, id: &str, output: &Path, w: u32, h: u32) -> Vec<OsString> {
    let mut body = strs(&MAP_AV);
    body.push("-vf".into());
    body.push(format!(
        "crop=w='trunc(min(iw,ih*{w}/{h})/2)*2':h='trunc(min(ih,iw*{h}/{w})/2)*2'"
    ));
    body.extend(video_codec_args(id, None, None));
    body.extend(strs(&["-c:a", "copy"]));
    assemble(Vec::new(), input, body, muxer(id), output)
}

/// What ffprobe told us about a file.
#[derive(Debug, Default)]
struct Probe {
    duration: Option<f64>,
    video_codec: Option<String>,
    audio_codec: Option<String>,
}

fn parse_probe_json(text: &str) -> Option<Probe> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let mut probe = Probe {
        duration: v["format"]["duration"]
            .as_str()
            .and_then(|d| d.parse::<f64>().ok())
            .filter(|d| d.is_finite() && *d > 0.0),
        ..Probe::default()
    };
    for s in v["streams"].as_array().into_iter().flatten() {
        let name = s["codec_name"].as_str().map(str::to_string);
        match s["codec_type"].as_str() {
            Some("video") if probe.video_codec.is_none() => probe.video_codec = name,
            Some("audio") if probe.audio_codec.is_none() => probe.audio_codec = name,
            _ => {}
        }
    }
    Some(probe)
}

/// `Duration: 00:00:02.00,` from `ffmpeg -i` stderr.
fn parse_duration_stderr(text: &str) -> Option<f64> {
    let rest = text.split("Duration:").nth(1)?.trim_start();
    let stamp = rest.split(',').next()?.trim();
    let mut parts = stamp.split(':');
    let h: f64 = parts.next()?.parse().ok()?;
    let m: f64 = parts.next()?.parse().ok()?;
    let s: f64 = parts.next()?.parse().ok()?;
    let total = h * 3600.0 + m * 60.0 + s;
    (total > 0.0).then_some(total)
}

fn tail(text: &str) -> String {
    let t = text.trim();
    let start = t.char_indices().rev().nth(599).map_or(0, |(i, _)| i);
    t[start..].to_string()
}

impl Ffmpeg {
    fn probe(&self, input: &Path) -> Option<Probe> {
        let ffprobe = self.ffprobe.as_ref()?;
        let mut cmd = proc::command(ffprobe);
        cmd.args([
            "-v",
            "error",
            "-show_entries",
            "format=duration:stream=codec_type,codec_name",
            "-of",
            "json",
        ])
        .arg(input);
        let out = proc::run(cmd, &CancelToken::new()).ok()?;
        out.status
            .success()
            .then(|| parse_probe_json(&out.stdout))?
    }

    /// Media duration in seconds (None if unknown, e.g. still image).
    pub fn duration(&self, input: &Path) -> Option<f64> {
        if self.ffprobe.is_some() {
            return self.probe(input).and_then(|p| p.duration);
        }
        let mut cmd = proc::command(&self.ffmpeg);
        cmd.args(["-hide_banner", "-nostdin", "-i"]).arg(input);
        let out = proc::run(cmd, &CancelToken::new()).ok()?;
        parse_duration_stderr(&out.stderr)
    }

    fn run(
        &self,
        args: Vec<OsString>,
        total: Option<f64>,
        cancel: &CancelToken,
        progress: &dyn Fn(f32),
    ) -> Result<()> {
        let mut cmd = proc::command(&self.ffmpeg);
        cmd.args([
            "-hide_banner",
            "-nostdin",
            "-y",
            "-loglevel",
            "error",
            "-progress",
            "pipe:1",
            "-nostats",
        ])
        .args(args);
        let out = proc::run_streaming(
            cmd,
            cancel,
            &mut |line| {
                if let (Some(t), Some(total)) = (parse_progress_line(line), total) {
                    progress(((t / total) as f32).clamp(0.0, 1.0));
                }
            },
            &mut |_| {},
        )?;
        if !out.status.success() {
            return Err(Error::ToolFailed(tail(&out.stderr)));
        }
        progress(1.0);
        Ok(())
    }

    /// Real two-pass GIF: palette PNG next to the output (deleted afterwards, also on
    /// error/cancel), then the encode. Progress: pass 1 = 0..0.5, pass 2 = 0.5..1.
    fn gif(
        &self,
        input: &Path,
        output: &Path,
        spec: &GifSpec,
        total: Option<f64>,
        cancel: &CancelToken,
        progress: &dyn Fn(f32),
    ) -> Result<()> {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let palette = output.with_file_name(format!(
            ".fc-palette-{}-{}.png",
            std::process::id(),
            COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let result = self
            .run(
                gif_palette_args(input, &palette, spec),
                total,
                cancel,
                &|p| progress(p * 0.5),
            )
            .and_then(|()| {
                self.run(
                    gif_encode_args(input, &palette, output, spec),
                    total,
                    cancel,
                    &|p| progress(0.5 + p * 0.5),
                )
            });
        let _ = std::fs::remove_file(&palette);
        result
    }

    fn require_input(input: &Path) -> Result<()> {
        if input.is_file() {
            Ok(())
        } else {
            Err(Error::NotFound(input.to_path_buf()))
        }
    }

    fn container_for(input: &Path) -> Result<&'static str> {
        container_of(input).ok_or_else(|| Error::Unsupported(format!("{}", input.display())))
    }

    /// Format conversion. Codec choices:
    /// mp4/mov -> libx264 (CRF from quality, default 23, preset veryfast, yuv420p, +faststart) + aac 160k;
    /// mkv -> libx264 + aac; webm -> libvpx-vp9 (CRF 32, -b:v 0, row-mt) + libopus 128k; avi -> mpeg4 q:v 4 + mp3;
    /// gif -> real two-pass palette (palette PNG next to the output, deleted afterwards) (fps 15, width min(src,480), lanczos, max 60 s);
    /// audio: mp3 libmp3lame q:a 2, m4a aac 192k, opus/ogg libopus 128k, flac, wav pcm_s16le; video->audio drops video (-vn);
    /// m4r (ringtone) = m4a cut to the first 40 s; srt/vtt from the first subtitle stream (ASS/SSA input).
    /// Try stream copy first when the target container accepts the source codecs (mp4<->mov<->mkv with h264/aac).
    /// Honors `max_width`/`max_height` with scale filter (even dimensions).
    ///
    /// The GIF palette is generated and applied in one filter graph (`split` + `palettegen` +
    /// `paletteuse`), so no temporary palette file is needed.
    pub fn convert(
        &self,
        input: &Path,
        to: &str,
        output: &Path,
        options: &ConvertOptions,
        cancel: &CancelToken,
        progress: &dyn Fn(f32),
    ) -> Result<()> {
        Self::require_input(input)?;
        // m4v is MP4 with another extension.
        let to = if to == "m4v" { "mp4" } else { to };
        if to == "txt" {
            // ASS/SSA -> plain text: SubRip through ffmpeg, then the in-process SRT -> TXT.
            let srt = output.with_extension("fc-ass.srt");
            let legacy = std::fs::read(input)
                .is_ok_and(|b| super::subtitle::sniff(&b) == super::subtitle::TextEncoding::Cp1254);
            let r = self
                .run(
                    subtitle_args(input, "srt", &srt, legacy),
                    None,
                    cancel,
                    progress,
                )
                .and_then(|()| super::subtitle::convert(&srt, "srt", "txt", output));
            let _ = std::fs::remove_file(&srt);
            return r;
        }
        if matches!(to, "srt" | "vtt") {
            let legacy = std::fs::read(input)
                .is_ok_and(|b| super::subtitle::sniff(&b) == super::subtitle::TextEncoding::Cp1254);
            return self.run(
                subtitle_args(input, to, output, legacy),
                None,
                cancel,
                progress,
            );
        }
        let probe = self.probe(input);
        let duration = probe
            .as_ref()
            .and_then(|p| p.duration)
            .or_else(|| self.duration(input));
        if to == "gif" {
            let spec = GifSpec {
                max_width: options.max_width.map_or(480, |w| w.min(480)),
                max_height: options.max_height,
                fps: 15,
                max_seconds: 60,
            };
            let total = duration.map(|d| d.min(60.0));
            return self.gif(input, output, &spec, total, cancel, progress);
        }
        if !matches!(
            to,
            "mp4"
                | "mov"
                | "mkv"
                | "webm"
                | "avi"
                | "mp3"
                | "m4a"
                | "opus"
                | "ogg"
                | "flac"
                | "wav"
                | "m4r"
                | "wmv"
                | "flv"
                | "mpg"
                | "3gp"
                | "ts"
                | "aac"
                | "wma"
                | "aiff"
                | "ac3"
                | "mka"
        ) {
            return Err(Error::Unsupported(format!("ffmpeg target {to}")));
        }
        let duration = if to == "m4r" {
            duration.map(|d| d.min(f64::from(RINGTONE_MAX_SECS)))
        } else {
            duration
        };
        if let (Some(p), Some(src)) = (&probe, container_of(input))
            && matches!(to, "mp4" | "mov" | "mkv")
            && matches!(src, "mp4" | "mov" | "mkv")
            && p.video_codec.as_deref() == Some("h264")
            && p.audio_codec.as_deref().is_none_or(|a| a == "aac")
            && options.quality.is_none()
            && options.max_width.is_none()
            && options.max_height.is_none()
        {
            match self.run(copy_args(input, to, output), duration, cancel, progress) {
                Ok(()) => return Ok(()),
                Err(Error::Cancelled) => return Err(Error::Cancelled),
                Err(_) => {} // fall back to a full encode
            }
        }
        self.run(
            convert_args(input, to, output, options),
            duration,
            cancel,
            progress,
        )
    }

    /// Same container, smaller: video CRF Light 26 / Balanced 29 / Strong 33 (x264 for mp4/mov/mkv/avi,
    /// vp9 CRF +8 for webm); audio bitrate Light 160k / Balanced 128k / Strong 96k.
    pub fn compress(
        &self,
        input: &Path,
        output: &Path,
        level: CompressLevel,
        cancel: &CancelToken,
        progress: &dyn Fn(f32),
    ) -> Result<()> {
        Self::require_input(input)?;
        let id = Self::container_for(input)?;
        let total = self.duration(input);
        self.run(
            compress_args(input, id, output, level),
            total,
            cancel,
            progress,
        )
    }

    /// Keep `[start_ms, end_ms)`; accurate seek (re-encode) so cuts are frame-exact.
    pub fn trim(
        &self,
        input: &Path,
        output: &Path,
        start_ms: u64,
        end_ms: u64,
        cancel: &CancelToken,
        progress: &dyn Fn(f32),
    ) -> Result<()> {
        Self::require_input(input)?;
        if end_ms <= start_ms {
            return Err(Error::Unsupported("empty trim range".into()));
        }
        let id = Self::container_for(input)?;
        let total = (end_ms - start_ms) as f64 / 1000.0;
        self.run(
            trim_args(input, id, output, start_ms, end_ms),
            Some(total),
            cancel,
            progress,
        )
    }

    /// Remove audio (`-an -c:v copy`).
    pub fn mute(
        &self,
        input: &Path,
        output: &Path,
        cancel: &CancelToken,
        progress: &dyn Fn(f32),
    ) -> Result<()> {
        Self::require_input(input)?;
        let id = Self::container_for(input)?;
        if !is_video_container(id) {
            return Err(Error::Unsupported("mute needs a video file".into()));
        }
        let total = self.duration(input);
        self.run(mute_args(input, id, output), total, cancel, progress)
    }

    /// setpts + chained atempo (each 0.5..=2.0) for 0.25..=4.0.
    pub fn speed(
        &self,
        input: &Path,
        output: &Path,
        factor: f32,
        cancel: &CancelToken,
        progress: &dyn Fn(f32),
    ) -> Result<()> {
        Self::require_input(input)?;
        if !(0.25..=4.0).contains(&factor) {
            return Err(Error::Unsupported(format!("speed factor {factor}")));
        }
        let id = Self::container_for(input)?;
        let total = self.duration(input).map(|d| d / f64::from(factor));
        self.run(
            speed_args(input, id, output, factor),
            total,
            cancel,
            progress,
        )
    }

    /// crop=w:h:x:y (even values), audio copied.
    #[allow(clippy::too_many_arguments)]
    pub fn crop(
        &self,
        input: &Path,
        output: &Path,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        cancel: &CancelToken,
        progress: &dyn Fn(f32),
    ) -> Result<()> {
        Self::require_input(input)?;
        let id = Self::container_for(input)?;
        if !is_video_container(id) {
            return Err(Error::Unsupported("crop needs a video file".into()));
        }
        if width < 2 || height < 2 {
            return Err(Error::Unsupported("crop size too small".into()));
        }
        let total = self.duration(input);
        self.run(
            crop_args(input, id, output, x, y, width, height),
            total,
            cancel,
            progress,
        )
    }

    /// Centre crop to the aspect ratio `w:h` (even sizes), audio copied.
    pub fn crop_aspect(
        &self,
        input: &Path,
        output: &Path,
        w: u32,
        h: u32,
        cancel: &CancelToken,
        progress: &dyn Fn(f32),
    ) -> Result<()> {
        Self::require_input(input)?;
        let id = Self::container_for(input)?;
        if !is_video_container(id) {
            return Err(Error::Unsupported("crop needs a video file".into()));
        }
        if w == 0 || h == 0 {
            return Err(Error::Unsupported("empty aspect ratio".into()));
        }
        let total = self.duration(input);
        self.run(
            crop_aspect_args(input, id, output, w, h),
            total,
            cancel,
            progress,
        )
    }

    /// Video -> GIF for the downloader's GIF mode (same palette pipeline as `convert` to gif).
    #[allow(clippy::too_many_arguments)]
    pub fn to_gif(
        &self,
        input: &Path,
        output: &Path,
        max_width: u32,
        fps: u32,
        max_seconds: u32,
        cancel: &CancelToken,
        progress: &dyn Fn(f32),
    ) -> Result<()> {
        Self::require_input(input)?;
        let total = self.duration(input).map(|d| d.min(f64::from(max_seconds)));
        let spec = GifSpec {
            max_width: max_width.max(16),
            max_height: None,
            fps: fps.clamp(1, 60),
            max_seconds: max_seconds.max(1),
        };
        self.gif(input, output, &spec, total, cancel, progress)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(args: &[OsString]) -> String {
        args.iter()
            .map(|a| a.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn progress_lines() {
        assert_eq!(parse_progress_line("out_time_us=1500000"), Some(1.5));
        assert_eq!(parse_progress_line("out_time_ms=2000000"), Some(2.0));
        assert_eq!(parse_progress_line("out_time_us=N/A"), None);
        assert_eq!(
            parse_progress_line("out_time_us=-9223372036854775807"),
            None
        );
        assert_eq!(parse_progress_line("progress=end"), None);
        assert_eq!(parse_progress_line("frame=12"), None);
    }

    #[test]
    fn mp4_convert_args() {
        let a = line(&convert_args(
            Path::new("in.mkv"),
            "mp4",
            Path::new("out.tmp"),
            &ConvertOptions::default(),
        ));
        assert!(a.contains(
            "-c:v libx264 -crf 23 -preset veryfast -pix_fmt yuv420p -movflags +faststart"
        ));
        assert!(a.contains("-c:a aac -b:a 160k"));
        assert!(a.contains("force_divisible_by=2"));
        assert!(a.ends_with("-f mp4 out.tmp"));
    }

    #[test]
    fn wave3_container_and_codec_args() {
        let conv = |to: &str| {
            line(&convert_args(
                Path::new("in.mkv"),
                to,
                Path::new("o"),
                &ConvertOptions::default(),
            ))
        };
        let a = conv("wmv");
        assert!(
            a.contains("-c:v wmv2") && a.contains("wmav2") && a.ends_with("-f asf o"),
            "{a}"
        );
        let a = conv("flv");
        assert!(
            a.contains("libx264") && a.contains("-c:a aac") && a.ends_with("-f flv o"),
            "{a}"
        );
        let a = conv("mpg");
        assert!(
            a.contains("mpeg2video") && a.contains("-c:a mp2") && a.ends_with("-f mpeg o"),
            "{a}"
        );
        assert!(conv("mpg").contains("-r 25") && conv("mpg").contains("mp2 -ac 2"));
        assert!(conv("wma").contains("wmav2 -ac 2"));
        assert!(conv("3gp").ends_with("-f 3gp o"));
        assert!(conv("ts").ends_with("-f mpegts o"));
        let a = conv("aac");
        assert!(
            a.contains("-vn") && a.contains("-c:a aac") && a.ends_with("-f adts o"),
            "{a}"
        );
        assert!(conv("wma").ends_with("-f asf o") && conv("wma").contains("wmav2"));
        assert!(conv("aiff").contains("pcm_s16be") && conv("aiff").ends_with("-f aiff o"));
        assert!(conv("ac3").contains("-c:a ac3") && conv("ac3").ends_with("-f ac3 o"));
        let a = conv("mka");
        assert!(a.contains("libopus") && a.ends_with("-f matroska o"), "{a}");
        assert_eq!(container_of(Path::new("a.M4V")), Some("mp4"));
        assert_eq!(container_of(Path::new("a.aac")), Some("aac"));
        assert_eq!(container_of(Path::new("a.mpeg")), Some("mpg"));
        assert_eq!(container_of(Path::new("a.amr")), None);
    }

    #[test]
    fn quality_and_scale() {
        let o = ConvertOptions {
            quality: Some(50),
            max_width: Some(640),
            max_height: Some(360),
            ..ConvertOptions::default()
        };
        let a = line(&convert_args(
            Path::new("a.mp4"),
            "webm",
            Path::new("o"),
            &o,
        ));
        assert!(a.contains("-crf 37"), "{a}");
        assert!(a.contains("min(iw,640)") && a.contains("min(ih,360)"));
        assert!(a.contains("libvpx-vp9") && a.contains("libopus"));
        assert_eq!(x264_crf(Some(75)), 23);
        assert_eq!(x264_crf(None), 23);
    }

    #[test]
    fn audio_targets_drop_video() {
        let a = line(&convert_args(
            Path::new("a.mp4"),
            "mp3",
            Path::new("o"),
            &ConvertOptions::default(),
        ));
        assert!(a.contains("-vn -map 0:a:0 -c:a libmp3lame -q:a 2"));
        assert!(a.ends_with("-f mp3 o"));
        let w = line(&convert_args(
            Path::new("a.flac"),
            "wav",
            Path::new("o"),
            &ConvertOptions::default(),
        ));
        assert!(w.contains("pcm_s16le"));
        let m = line(&convert_args(
            Path::new("a.wav"),
            "m4a",
            Path::new("o"),
            &ConvertOptions::default(),
        ));
        assert!(m.contains("aac -b:a 192k") && m.ends_with("-f ipod o"));
    }

    #[test]
    fn gif_two_pass() {
        let spec = GifSpec {
            max_width: 480,
            max_height: Some(300),
            fps: 15,
            max_seconds: 60,
        };
        let p1 = line(&gif_palette_args(
            Path::new("a.mp4"),
            Path::new("pal.png"),
            &spec,
        ));
        assert!(p1.contains("palettegen") && p1.contains("min(ih,300)") && p1.contains("fps=15"));
        assert!(p1.ends_with("-f image2 pal.png"));
        let p2 = line(&gif_encode_args(
            Path::new("a.mp4"),
            Path::new("pal.png"),
            Path::new("o.gif"),
            &spec,
        ));
        assert!(p2.starts_with("-i a.mp4 -i pal.png"));
        assert!(p2.contains("paletteuse") && p2.contains("min(iw,480)") && p2.contains("lanczos"));
        assert!(p2.contains("-t 60") && p2.ends_with("-f gif o.gif"));
    }

    #[test]
    fn compress_levels() {
        let l = line(&compress_args(
            Path::new("a.mp4"),
            "mp4",
            Path::new("o"),
            CompressLevel::Light,
        ));
        assert!(l.contains("-crf 26") && l.contains("-b:a 160k"));
        let s = line(&compress_args(
            Path::new("a.mp4"),
            "mp4",
            Path::new("o"),
            CompressLevel::Strong,
        ));
        assert!(s.contains("-crf 33") && s.contains("-b:a 96k"));
        let w = line(&compress_args(
            Path::new("a.webm"),
            "webm",
            Path::new("o"),
            CompressLevel::Balanced,
        ));
        assert!(w.contains("-crf 37") && w.contains("-b:a 128k"));
        let au = line(&compress_args(
            Path::new("a.mp3"),
            "mp3",
            Path::new("o"),
            CompressLevel::Strong,
        ));
        assert!(au.contains("-b:a 96k") && !au.contains("libx264"));
    }

    #[test]
    fn avi_compress_uses_x264_and_even_scale() {
        let a = line(&compress_args(
            Path::new("a.avi"),
            "avi",
            Path::new("o"),
            CompressLevel::Balanced,
        ));
        assert!(a.contains("libx264 -crf 29") && !a.contains("mpeg4"), "{a}");
        assert!(a.contains("trunc(iw/2)*2") && !a.contains("faststart"));
        let t = line(&trim_args(
            Path::new("a.mp4"),
            "mp4",
            Path::new("o"),
            0,
            1000,
        ));
        assert!(t.contains("trunc(iw/2)*2"));
        let sp = line(&speed_args(Path::new("a.mp4"), "mp4", Path::new("o"), 2.0));
        assert!(sp.contains("setpts=PTS/2,scale=trunc(iw/2)*2"));
    }

    #[test]
    fn trim_uses_accurate_seek_args() {
        let a = line(&trim_args(
            Path::new("a.mp4"),
            "mp4",
            Path::new("o"),
            1500,
            4000,
        ));
        assert!(a.starts_with("-ss 1.500 -i a.mp4 -t 2.500"));
        assert!(a.contains("libx264"));
    }

    #[test]
    fn mute_copies_video_only() {
        let a = line(&mute_args(Path::new("a.mp4"), "mp4", Path::new("o")));
        assert!(a.contains("-map 0:v:0 -an -c:v copy"));
        assert!(!a.contains("0:a"));
    }

    #[test]
    fn atempo_chains() {
        assert_eq!(atempo_chain(2.0), "atempo=2");
        assert_eq!(atempo_chain(1.5), "atempo=1.5");
        assert_eq!(atempo_chain(4.0), "atempo=2,atempo=2");
        assert_eq!(atempo_chain(0.25), "atempo=0.5,atempo=0.5");
        assert_eq!(atempo_chain(3.0), "atempo=2,atempo=1.5");
        let a = line(&speed_args(Path::new("a.mp4"), "mp4", Path::new("o"), 2.0));
        assert!(a.contains("setpts=PTS/2"));
    }

    #[test]
    fn crop_is_even() {
        let a = line(&crop_args(
            Path::new("a.mp4"),
            "mp4",
            Path::new("o"),
            3,
            5,
            101,
            51,
        ));
        assert!(a.contains("crop=100:50:2:4"));
        assert!(a.contains("-c:a copy"));
    }

    #[test]
    fn ringtone_subtitle_and_aspect_args() {
        let r = line(&convert_args(
            Path::new("a.mp4"),
            "m4r",
            Path::new("o"),
            &ConvertOptions::default(),
        ));
        assert!(r.contains("-vn -map 0:a:0 -t 40 -c:a aac -b:a 192k"), "{r}");
        assert!(r.ends_with("-f ipod o"));
        let s = line(&subtitle_args(
            Path::new("a.ass"),
            "vtt",
            Path::new("o"),
            false,
        ));
        assert_eq!(s, "-i a.ass -map 0:s:0 -c:s webvtt -f webvtt o");
        let s = line(&subtitle_args(
            Path::new("a.ass"),
            "srt",
            Path::new("o"),
            true,
        ));
        assert_eq!(
            s,
            "-sub_charenc CP1254 -i a.ass -map 0:s:0 -c:s srt -f srt o"
        );
        let c = line(&crop_aspect_args(
            Path::new("a.mp4"),
            "mp4",
            Path::new("o"),
            9,
            16,
        ));
        assert!(
            c.contains("crop=w='trunc(min(iw,ih*9/16)/2)*2':h='trunc(min(ih,iw*16/9)/2)*2'"),
            "{c}"
        );
        assert!(c.contains("-c:a copy") && c.contains("libx264"));
    }

    #[test]
    fn copy_args_have_no_encoder() {
        let a = line(&copy_args(Path::new("a.mkv"), "mp4", Path::new("o")));
        assert!(a.contains("-c copy") && a.contains("+faststart"));
    }

    #[test]
    fn probe_json() {
        let p = parse_probe_json(
            r#"{"streams":[{"codec_name":"h264","codec_type":"video"},{"codec_name":"aac","codec_type":"audio"}],"format":{"duration":"2.000000"}}"#,
        )
        .unwrap();
        assert_eq!(p.duration, Some(2.0));
        assert_eq!(p.video_codec.as_deref(), Some("h264"));
        assert_eq!(p.audio_codec.as_deref(), Some("aac"));
        let none = parse_probe_json(r#"{"streams":[],"format":{"duration":"N/A"}}"#).unwrap();
        assert_eq!(none.duration, None);
    }

    #[test]
    fn duration_from_stderr() {
        let t = "Input #0, mov\n  Duration: 00:01:02.50, start: 0.000000, bitrate: 1 kb/s";
        assert_eq!(parse_duration_stderr(t), Some(62.5));
        assert_eq!(parse_duration_stderr("Duration: N/A, bitrate: N/A"), None);
    }

    #[test]
    fn invalid_requests_are_rejected_without_ffmpeg() {
        let f = Ffmpeg {
            ffmpeg: PathBuf::from("nonexistent-ffmpeg"),
            ffprobe: None,
        };
        let c = CancelToken::new();
        let missing = f.mute(Path::new("nope.mp4"), Path::new("o"), &c, &|_| {});
        assert!(matches!(missing, Err(Error::NotFound(_))));
    }
}

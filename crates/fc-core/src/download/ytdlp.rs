//! yt-dlp wrapper: probe (`-J`) and download, with pure argument builders and parsers
//! (unit-tested against the fixtures in `tests/fixtures/`). OWNER: Task F.
//!
//! Every run passes `--ignore-config --no-update --no-playlist --newline`, the ffmpeg
//! location, a scratch folder for partial files and, for YouTube, a JS runtime.
//! yt-dlp writes into `<staging>/dl`; the finished file is then moved into the destination
//! under a free name, so partial files never appear in the user's folder.

use super::{DlMode, DlProgress, DlQuality, DownloadRequest, MediaInfo, classify};
use crate::engine_mgr::which;
use crate::engines::ToolLocator;
use crate::engines::ffmpeg::Ffmpeg;
use crate::jobs::naming;
use crate::{CancelToken, Error, Result, proc};
use serde_json::Value;
use std::path::{Path, PathBuf};

const YTDLP: &str = "yt-dlp";
const YTDLP_EXE: &str = "yt-dlp.exe";
/// Message of the `Unsupported` error for posts that carry no video (see [`is_no_video`]).
const NO_VIDEO: &str = "no video in this post";
const OUTPUT_TEMPLATE: &str = "%(title).150B [%(id)s].%(ext)s";
const PROGRESS_TEMPLATE: &str = "download:FC %(progress.downloaded_bytes)s %(progress.total_bytes)s %(progress.total_bytes_estimate)s %(progress.speed)s %(progress.eta)s";
const POSTPROCESS_TEMPLATE: &str = "postprocess:FCPP %(progress.status)s";
const IMAGE_EXT: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "bmp", "avif", "heic"];

// ------------------------------------------------------------------ argument builders

fn is_youtube(url: &str) -> bool {
    classify::classify_url(url).is_some_and(|l| l.site == Some("YouTube"))
}

/// `node:<path>` when Node is available, else `deno:<path>`; only YouTube needs one.
pub fn js_runtime(url: &str, node: Option<&Path>, tools: &dyn ToolLocator) -> Option<String> {
    if !is_youtube(url) {
        return None;
    }
    if let Some(n) = node {
        return Some(format!("node:{}", n.display()));
    }
    tools
        .locate("deno", "deno.exe")
        .map(|d| format!("deno:{}", d.display()))
}

fn ffmpeg_dir(tools: &dyn ToolLocator) -> Option<PathBuf> {
    tools
        .locate("ffmpeg", "ffmpeg.exe")
        .and_then(|p| p.parent().map(Path::to_path_buf))
}

fn common_args(tools: &dyn ToolLocator, url: &str, cookies: Option<&str>) -> Vec<String> {
    let mut a: Vec<String> = [
        "--ignore-config",
        "--no-update",
        "--no-playlist",
        "--playlist-items",
        "1",
        "--newline",
        "--no-colors",
    ]
    .map(String::from)
    .into();
    if let Some(dir) = ffmpeg_dir(tools) {
        a.push("--ffmpeg-location".into());
        a.push(dir.display().to_string());
    }
    if let Some(rt) = js_runtime(url, which("node.exe").as_deref(), tools) {
        a.push("--js-runtimes".into());
        a.push(rt);
    }
    if let Some(browser) = cookies.map(str::trim).filter(|b| !b.is_empty()) {
        a.push("--cookies-from-browser".into());
        a.push(browser.into());
    }
    a
}

/// `yt-dlp -J` arguments (JSON about the single video, no download).
pub fn probe_args(tools: &dyn ToolLocator, url: &str, cookies: Option<&str>) -> Vec<String> {
    let mut a = common_args(tools, url, cookies);
    a.push("--no-warnings".into());
    a.push("-J".into());
    a.push("--".into());
    a.push(url.into());
    a
}

fn height_cap(q: DlQuality) -> Option<u32> {
    match q {
        DlQuality::Best => None,
        DlQuality::P1080 => Some(1080),
        DlQuality::P720 => Some(720),
    }
}

/// yt-dlp `-f` selector. `Audio` returns the audio-only selector; `Gif` is a video at <= 720p.
pub fn format_selector(mode: DlMode, quality: DlQuality) -> String {
    let (mode, quality) = match mode {
        DlMode::Gif => (DlMode::Video, DlQuality::P720),
        m => (m, quality),
    };
    let cap = height_cap(quality).map(|h| format!("[height<={h}]"));
    let cap = cap.as_deref().unwrap_or("");
    match mode {
        DlMode::Original => format!("bv*{cap}+ba/b{cap}"),
        // H.264 first: "Video" promises an MP4 that plays everywhere, and an mp4 container alone
        // can hold AV1, which older devices and players cannot decode.
        DlMode::Video => format!(
            "bv*{cap}[vcodec^=avc1]+ba[ext=m4a]/bv*{cap}[ext=mp4]+ba[ext=m4a]/bv*{cap}+ba/b{cap}"
        ),
        DlMode::Audio | DlMode::Gif => "ba/b".into(),
    }
}

/// Full download arguments. `staging` is the scratch folder (see the module docs).
pub fn download_args(
    req: &DownloadRequest,
    tools: &dyn ToolLocator,
    staging: &Path,
    cookies: Option<&str>,
) -> Vec<String> {
    let mut a = common_args(tools, &req.url, cookies);
    a.push("--windows-filenames".into());
    a.push("--progress".into());
    a.push("--progress-template".into());
    a.push(PROGRESS_TEMPLATE.into());
    a.push("--progress-template".into());
    a.push(POSTPROCESS_TEMPLATE.into());
    a.push("--paths".into());
    a.push(format!("home:{}", staging.join("dl").display()));
    a.push("--paths".into());
    a.push(format!("temp:{}", staging.join("part").display()));
    a.push("-o".into());
    a.push(OUTPUT_TEMPLATE.into());
    a.push("-f".into());
    a.push(format_selector(req.mode, req.quality));
    match req.mode {
        DlMode::Video | DlMode::Gif => {
            a.extend(["--merge-output-format", "mp4", "--remux-video", "mp4"].map(String::from));
        }
        DlMode::Audio => {
            a.extend(["-x", "--audio-format", "mp3", "--audio-quality", "0"].map(String::from));
        }
        DlMode::Original => {}
    }
    a.push("--print".into());
    a.push("after_move:filepath".into());
    a.push("--".into());
    a.push(req.url.clone());
    a
}

// ------------------------------------------------------------------ parsers

/// One `FC ...` progress line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProgressLine {
    pub downloaded: u64,
    /// Exact total, else yt-dlp's estimate.
    pub total: Option<u64>,
    pub speed_bps: Option<u64>,
    pub eta_secs: Option<u64>,
}

impl ProgressLine {
    pub fn fraction(&self) -> Option<f32> {
        super::direct::fraction(self.downloaded, self.total)
    }
}

fn number(s: &str) -> Option<u64> {
    let v: f64 = s.parse().ok()?;
    (v.is_finite() && v >= 0.0).then_some(v as u64)
}

/// Parse `FC <downloaded> <total> <total_estimate> <speed> <eta>`; any value may be `NA`.
pub fn parse_progress(line: &str) -> Option<ProgressLine> {
    let rest = line.trim().strip_prefix("FC ")?;
    let f: Vec<&str> = rest.split_whitespace().collect();
    if f.len() < 5 {
        return None;
    }
    Some(ProgressLine {
        downloaded: number(f[0]).unwrap_or(0),
        total: number(f[1])
            .filter(|t| *t > 0)
            .or_else(|| number(f[2]).filter(|t| *t > 0)),
        speed_bps: number(f[3]),
        eta_secs: number(f[4]),
    })
}

fn str_of<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str).filter(|s| !s.is_empty())
}

fn num_of(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(Value::as_f64)
}

fn size_of(v: &Value) -> Option<u64> {
    num_of(v, "filesize")
        .or_else(|| num_of(v, "filesize_approx"))
        .filter(|s| *s > 0.0)
        .map(|s| s as u64)
}

fn is_image_entry(v: &Value) -> bool {
    str_of(v, "ext").is_some_and(|e| IMAGE_EXT.contains(&e))
}

/// Turn the `-J` JSON into the preview card data. Rejects live streams and DRM.
pub fn parse_probe_json(json: &str, url: &str) -> Result<MediaInfo> {
    let root: Value = serde_json::from_str(json.trim())
        .map_err(|e| Error::Failed(format!("unreadable yt-dlp output: {e}")))?;

    // A multi-item post (carousel): describe the first entry, list the image URLs.
    let entries: Vec<&Value> = root
        .get("entries")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter(|e| e.is_object()).collect())
        .unwrap_or_default();
    let item = entries.first().copied().unwrap_or(&root);

    for v in [&root, item] {
        let live = v.get("is_live").and_then(Value::as_bool) == Some(true)
            || matches!(str_of(v, "live_status"), Some("is_live" | "is_upcoming"));
        if live {
            return Err(Error::Unsupported("live streams are not supported".into()));
        }
        if v.get("has_drm").and_then(Value::as_bool) == Some(true) {
            return Err(Error::Unsupported("DRM-protected video".into()));
        }
    }

    let requested: Vec<&Value> = item
        .get("requested_formats")
        .and_then(Value::as_array)
        .map(|a| a.iter().collect())
        .unwrap_or_default();
    let formats: Vec<&Value> = item
        .get("formats")
        .and_then(Value::as_array)
        .map(|a| a.iter().collect())
        .unwrap_or_default();

    let dim = |key: &str| -> Option<u32> {
        num_of(item, key)
            .or_else(|| {
                requested
                    .iter()
                    .chain(formats.iter())
                    .filter_map(|f| num_of(f, key))
                    .reduce(f64::max)
            })
            .filter(|d| *d > 0.0)
            .map(|d| d as u32)
    };

    let size = size_of(item).or_else(|| {
        let parts: Vec<u64> = requested.iter().filter_map(|f| size_of(f)).collect();
        (!parts.is_empty()).then(|| parts.iter().sum())
    });

    let image_urls: Vec<String> = entries
        .iter()
        .filter(|e| is_image_entry(e))
        .filter_map(|e| str_of(e, "url").map(String::from))
        .collect();
    let kind = if is_image_entry(item) {
        "image"
    } else if str_of(item, "vcodec") == Some("none") && str_of(item, "acodec") != Some("none") {
        "audio"
    } else {
        "video"
    };

    let link = classify::classify_url(url);
    let site = link
        .as_ref()
        .and_then(|l| l.site.map(String::from))
        .or_else(|| str_of(&root, "extractor_key").map(String::from))
        .or_else(|| link.as_ref().map(|l| l.host.clone()))
        .unwrap_or_default();
    let title = str_of(item, "title")
        .or_else(|| str_of(&root, "title"))
        .unwrap_or("")
        .to_string();

    Ok(MediaInfo {
        url: url.to_string(),
        title,
        site,
        kind: kind.into(),
        duration_secs: num_of(item, "duration").filter(|d| *d > 0.0),
        width: dim("width"),
        height: dim("height"),
        size_bytes: size,
        thumbnail: str_of(item, "thumbnail")
            .or_else(|| str_of(&root, "thumbnail"))
            .map(String::from),
        uploader: str_of(item, "uploader")
            .or_else(|| str_of(item, "channel"))
            .or_else(|| str_of(&root, "uploader"))
            .map(String::from),
        direct: false,
        image_urls,
    })
}

/// Map yt-dlp's stderr to the most useful error.
pub fn map_failure(stderr: &str, url: &str, had_js_runtime: bool) -> Error {
    // Warnings (e.g. "some formats were skipped as DRM protected") must not decide the error.
    let relevant: Vec<&str> = stderr
        .lines()
        .filter(|l| !l.trim_start().starts_with("WARNING:"))
        .collect();
    let low = relevant.join("\n").to_ascii_lowercase();
    let detail = stderr
        .lines()
        .rev()
        .find_map(|l| l.trim().strip_prefix("ERROR:"))
        .map(|l| l.trim().to_string())
        .or_else(|| {
            stderr
                .lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .map(|l| l.trim().to_string())
        })
        .unwrap_or_else(|| "yt-dlp failed".into());
    if low.contains("drm") {
        return Error::Unsupported("DRM-protected video".into());
    }
    if low.contains("this live event") || low.contains("is live") || low.contains("live stream") {
        return Error::Unsupported("live streams are not supported".into());
    }
    if low.contains("there is no video in this post") || low.contains("no video could be found") {
        return Error::Unsupported(NO_VIDEO.into());
    }
    if low.contains("unsupported url") {
        return Error::Unsupported("this link is not supported".into());
    }
    if is_youtube(url)
        && !had_js_runtime
        && (low.contains("javascript runtime")
            || low.contains("js runtime")
            || low.contains("n challenge")
            || low.contains("requested format is not available")
            || low.contains("signature solving"))
    {
        return Error::MissingTool("deno".into());
    }
    if [
        "unable to download",
        "getaddrinfo",
        "timed out",
        "network is unreachable",
        "connection reset",
        "http error 5",
        "name or service not known",
    ]
    .iter()
    .any(|m| low.contains(m))
    {
        return Error::Network(detail);
    }
    Error::ToolFailed(detail)
}

/// True for the error that means "this post has no video" (an image post).
pub fn is_no_video(e: &Error) -> bool {
    matches!(e, Error::Unsupported(m) if m == NO_VIDEO)
}

// ------------------------------------------------------------------ running yt-dlp

fn ytdlp_command(tools: &dyn ToolLocator, args: &[String]) -> Result<std::process::Command> {
    let exe = tools
        .locate(YTDLP, YTDLP_EXE)
        .ok_or_else(|| Error::MissingTool(YTDLP.into()))?;
    let mut cmd = proc::command(exe);
    cmd.args(args)
        .env("PYTHONIOENCODING", "utf-8")
        .env("PYTHONUTF8", "1");
    Ok(cmd)
}

/// `yt-dlp -J` for a page link. `Err(MissingTool("yt-dlp"))` when it is not installed.
pub fn probe(
    tools: &dyn ToolLocator,
    url: &str,
    cookies_browser: Option<&str>,
    cancel: &CancelToken,
) -> Result<MediaInfo> {
    let args = probe_args(tools, url, cookies_browser);
    let out = proc::run(ytdlp_command(tools, &args)?, cancel)?;
    if !out.status.success() {
        return Err(map_failure(
            &out.stderr,
            url,
            args.iter().any(|a| a == "--js-runtimes"),
        ));
    }
    parse_probe_json(&out.stdout, url)
}

/// Removes the scratch folder when the download ends, however it ends.
struct Staging(PathBuf);

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The finished file's path: the last stdout line that is not progress output.
fn final_path(stdout: &str) -> Option<PathBuf> {
    stdout
        .lines()
        .rev()
        .map(str::trim)
        .find(|l| {
            !l.is_empty() && !l.starts_with("FC ") && !l.starts_with("FCPP ") && !l.starts_with('[')
        })
        .map(PathBuf::from)
}

/// Download `req` into `req.out_dir` and return the final file. `staging` is a scratch folder
/// owned by this call (removed afterwards). Gif mode downloads <= 720p video, converts it with
/// ffmpeg (480 px wide, 15 fps, first 60 s) and deletes the video.
pub fn download(
    tools: &dyn ToolLocator,
    req: &DownloadRequest,
    staging: &Path,
    cookies_browser: Option<&str>,
    progress: &mut dyn FnMut(DlProgress),
    cancel: &CancelToken,
) -> Result<PathBuf> {
    let ffmpeg_exe = tools
        .locate("ffmpeg", "ffmpeg.exe")
        .ok_or_else(|| Error::MissingTool("ffmpeg".into()))?;
    let args = download_args(req, tools, staging, cookies_browser);
    let cmd = ytdlp_command(tools, &args)?;
    std::fs::create_dir_all(staging)?;
    let _guard = Staging(staging.to_path_buf());

    let gif = req.mode == DlMode::Gif;
    // The GIF step takes the last 30 % of the bar.
    let scale = |f: f32| if gif { f * 0.7 } else { f };
    let mut best = 0f32;
    let mut processing = false;
    let mut on_out = |line: &str| {
        if line.starts_with("FCPP ") {
            processing = true;
            progress(DlProgress {
                fraction: Some(scale(best)),
                processing: true,
                ..DlProgress::default()
            });
        } else if let Some(p) = parse_progress(line) {
            // A merged download fetches video then audio: never move the bar backwards.
            best = best.max(p.fraction().unwrap_or(0.0));
            progress(DlProgress {
                fraction: p.fraction().map(|_| scale(best)),
                speed_bps: p.speed_bps,
                eta_secs: p.eta_secs,
                processing,
            });
        }
    };
    let out = proc::run_streaming(cmd, cancel, &mut on_out, &mut |_| {})?;
    if !out.status.success() {
        let had_runtime = args.iter().any(|a| a == "--js-runtimes");
        return Err(map_failure(&out.stderr, &req.url, had_runtime));
    }
    let produced = final_path(&out.stdout)
        .filter(|p| p.is_file())
        .ok_or_else(|| Error::Failed("yt-dlp produced no file".into()))?;
    std::fs::create_dir_all(&req.out_dir)?;

    if !gif {
        let target = req.out_dir.join(produced.file_name().unwrap_or_default());
        return Ok(naming::commit(&produced, &target)?);
    }

    let target = naming::output_path(&produced, "gif", Some(&req.out_dir), None);
    let temp = naming::temp_path(&target);
    let ffmpeg = Ffmpeg {
        ffmpeg: ffmpeg_exe,
        ffprobe: tools.locate("ffmpeg", "ffprobe.exe"),
    };
    let progress = std::cell::RefCell::new(progress);
    let result = ffmpeg.to_gif(&produced, &temp, 480, 15, 60, cancel, &|f| {
        (progress.borrow_mut())(DlProgress {
            fraction: Some(0.7 + 0.3 * f),
            processing: true,
            ..DlProgress::default()
        });
    });
    match result {
        Ok(()) => Ok(naming::commit(&temp, &target)?),
        Err(e) => {
            let _ = std::fs::remove_file(&temp);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn final_path_skips_progress_noise() {
        let out = "FC 10 100 NA 5 3\nFCPP started\n\
                   C:\\stage\\dl\\First [a].mp4\n\
                   FC 100 100 NA 5 0\n[download] done\n\n";
        assert_eq!(
            final_path(out),
            Some(PathBuf::from("C:\\stage\\dl\\First [a].mp4"))
        );
        // several printed paths: the last one wins
        let out = "C:\\stage\\dl\\One.mp4\nFC 1 2 NA 1 1\nC:\\stage\\dl\\Two.mp4\nFCPP finished\n";
        assert_eq!(
            final_path(out),
            Some(PathBuf::from("C:\\stage\\dl\\Two.mp4"))
        );
        assert_eq!(final_path("FC 1 2 NA 1 1\n[x] y\n"), None);
    }
}

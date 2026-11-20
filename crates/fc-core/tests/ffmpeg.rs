use fc_core::engine_mgr::which;
use fc_core::engines::ffmpeg::Ffmpeg;
use fc_core::jobs::{CompressLevel, ConvertOptions};
use fc_core::{CancelToken, Error, proc};
use std::path::{Path, PathBuf};

fn tool() -> Option<Ffmpeg> {
    let Some(ffmpeg) = which("ffmpeg.exe") else {
        eprintln!("skipping: ffmpeg not on PATH");
        return None;
    };
    let Some(ffprobe) = which("ffprobe.exe") else {
        eprintln!("skipping: ffprobe not on PATH");
        return None;
    };
    Some(Ffmpeg {
        ffmpeg,
        ffprobe: Some(ffprobe),
    })
}

fn make(f: &Ffmpeg, dir: &Path, name: &str, extra: &[&str]) -> PathBuf {
    let out = dir.join(name);
    let mut cmd = proc::command(&f.ffmpeg);
    cmd.args(["-hide_banner", "-loglevel", "error", "-y"])
        .args(extra)
        .arg(&out);
    let o = proc::run(cmd, &CancelToken::new()).unwrap();
    assert!(o.status.success(), "{}", o.stderr);
    out
}

/// 2 s, 320x240 (or custom size) test video with a sine audio track.
fn sample_mp4(f: &Ffmpeg, dir: &Path, size: &str) -> PathBuf {
    make(
        f,
        dir,
        "in.mp4",
        &[
            "-f",
            "lavfi",
            "-i",
            &format!("testsrc=duration=2:size={size}:rate=15"),
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=2",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
        ],
    )
}

fn streams(f: &Ffmpeg, path: &Path) -> String {
    let probe = f.ffprobe.as_ref().expect("ffprobe");
    let mut cmd = proc::command(probe);
    cmd.args([
        "-v",
        "error",
        "-show_entries",
        "stream=codec_type,codec_name,width,height",
        "-of",
        "csv=p=0",
    ])
    .arg(path);
    proc::run(cmd, &CancelToken::new()).unwrap().stdout
}

fn nop(_: f32) {}

#[test]
fn mp4_to_webm_gif_mp3() {
    let Some(f) = tool() else { return };
    let d = tempfile::tempdir().unwrap();
    let src = sample_mp4(&f, d.path(), "320x240");
    let c = CancelToken::new();
    let o = ConvertOptions::default();

    let webm = d.path().join("o.webm.tmp"); // odd extension: muxer is explicit
    f.convert(&src, "webm", &webm, &o, &c, &nop).unwrap();
    let s = streams(&f, &webm);
    assert!(s.contains("vp9") && s.contains("opus"), "{s}");

    let gif = d.path().join("o.gif");
    f.convert(&src, "gif", &gif, &o, &c, &nop).unwrap();
    assert!(streams(&f, &gif).contains("gif"));
    assert_eq!(&std::fs::read(&gif).unwrap()[..3], b"GIF");

    let mp3 = d.path().join("o.mp3");
    f.convert(&src, "mp3", &mp3, &o, &c, &nop).unwrap();
    let s = streams(&f, &mp3);
    assert!(s.contains("mp3") && !s.contains("video"), "{s}");
}

#[test]
fn wav_to_flac() {
    let Some(f) = tool() else { return };
    let d = tempfile::tempdir().unwrap();
    let wav = make(
        &f,
        d.path(),
        "in.wav",
        &["-f", "lavfi", "-i", "sine=frequency=330:duration=1"],
    );
    let out = d.path().join("o.flac");
    f.convert(
        &wav,
        "flac",
        &out,
        &ConvertOptions::default(),
        &CancelToken::new(),
        &nop,
    )
    .unwrap();
    assert!(streams(&f, &out).contains("flac"));
}

#[test]
fn mkv_to_mp4_uses_stream_copy_and_reports_progress() {
    let Some(f) = tool() else { return };
    let d = tempfile::tempdir().unwrap();
    let mp4 = sample_mp4(&f, d.path(), "320x240");
    let mkv = d.path().join("in.mkv");
    f.convert(
        &mp4,
        "mkv",
        &mkv,
        &ConvertOptions::default(),
        &CancelToken::new(),
        &nop,
    )
    .unwrap();
    let out = d.path().join("o.mp4");
    let last = std::sync::Mutex::new(0f32);
    f.convert(
        &mkv,
        "mp4",
        &out,
        &ConvertOptions::default(),
        &CancelToken::new(),
        &|p| *last.lock().unwrap() = p,
    )
    .unwrap();
    assert_eq!(*last.lock().unwrap(), 1.0);
    let s = streams(&f, &out);
    assert!(s.contains("h264") && s.contains("aac"), "{s}");
}

#[test]
fn odd_sizes_become_even_and_max_width_downscales() {
    let Some(f) = tool() else { return };
    let d = tempfile::tempdir().unwrap();
    // yuv444 allows odd dimensions in the source.
    let src = make(
        &f,
        d.path(),
        "odd.mkv",
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=1:size=321x241:rate=10",
            "-c:v",
            "ffv1",
            "-pix_fmt",
            "yuv444p",
        ],
    );
    let out = d.path().join("o.mp4");
    f.convert(
        &src,
        "mp4",
        &out,
        &ConvertOptions::default(),
        &CancelToken::new(),
        &nop,
    )
    .unwrap();
    assert!(
        streams(&f, &out).contains("320,240"),
        "{}",
        streams(&f, &out)
    );

    let small = d.path().join("s.mp4");
    let o = ConvertOptions {
        max_width: Some(100),
        ..ConvertOptions::default()
    };
    f.convert(&src, "mp4", &small, &o, &CancelToken::new(), &nop)
        .unwrap();
    let s = streams(&f, &small);
    // 241 * 100 / 321 = 75.08, rounded to an even height by force_divisible_by=2.
    assert!(s.contains("100,76"), "{s}");
}

#[test]
fn compress_is_smaller_for_noisy_video() {
    let Some(f) = tool() else { return };
    let d = tempfile::tempdir().unwrap();
    let src = make(
        &f,
        d.path(),
        "noisy.mp4",
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=duration=2:size=640x360:rate=25",
            "-vf",
            "noise=alls=40:allf=t",
            "-c:v",
            "libx264",
            "-crf",
            "14",
            "-pix_fmt",
            "yuv420p",
        ],
    );
    let out = d.path().join("c.mp4");
    f.compress(&src, &out, CompressLevel::Strong, &CancelToken::new(), &nop)
        .unwrap();
    let (a, b) = (
        std::fs::metadata(&src).unwrap().len(),
        std::fs::metadata(&out).unwrap().len(),
    );
    assert!(b < a, "compressed {b} !< original {a}");
}

#[test]
fn trim_duration() {
    let Some(f) = tool() else { return };
    let d = tempfile::tempdir().unwrap();
    let src = sample_mp4(&f, d.path(), "320x240");
    let out = d.path().join("t.mp4");
    f.trim(&src, &out, 500, 1500, &CancelToken::new(), &nop)
        .unwrap();
    let dur = f.duration(&out).unwrap();
    assert!((dur - 1.0).abs() < 0.15, "{dur}");
    let bad = f.trim(&src, &out, 1500, 500, &CancelToken::new(), &nop);
    assert!(matches!(bad, Err(Error::Unsupported(_))));
}

#[test]
fn mute_removes_audio() {
    let Some(f) = tool() else { return };
    let d = tempfile::tempdir().unwrap();
    let src = sample_mp4(&f, d.path(), "320x240");
    assert!(streams(&f, &src).contains("audio"));
    let out = d.path().join("m.mp4");
    f.mute(&src, &out, &CancelToken::new(), &nop).unwrap();
    let s = streams(&f, &out);
    assert!(s.contains("video") && !s.contains("audio"), "{s}");
}

#[test]
fn speed_two_halves_duration() {
    let Some(f) = tool() else { return };
    let d = tempfile::tempdir().unwrap();
    let src = sample_mp4(&f, d.path(), "320x240");
    let out = d.path().join("s.mp4");
    f.speed(&src, &out, 2.0, &CancelToken::new(), &nop).unwrap();
    let (a, b) = (f.duration(&src).unwrap(), f.duration(&out).unwrap());
    assert!((b - a / 2.0).abs() < a / 2.0 * 0.1, "{a} -> {b}");
    let bad = f.speed(&src, &out, 8.0, &CancelToken::new(), &nop);
    assert!(matches!(bad, Err(Error::Unsupported(_))));
}

#[test]
fn crop_and_to_gif() {
    let Some(f) = tool() else { return };
    let d = tempfile::tempdir().unwrap();
    let src = sample_mp4(&f, d.path(), "320x240");
    let out = d.path().join("c.mp4");
    f.crop(&src, &out, 11, 11, 101, 61, &CancelToken::new(), &nop)
        .unwrap();
    assert!(
        streams(&f, &out).contains("100,60"),
        "{}",
        streams(&f, &out)
    );
    let gif = d.path().join("g.gif");
    f.to_gif(&src, &gif, 160, 10, 1, &CancelToken::new(), &nop)
        .unwrap();
    assert!(streams(&f, &gif).contains("160,120"));
    assert!(f.duration(&gif).unwrap() <= 1.5);
}

#[test]
fn cancel_returns_cancelled_and_touches_nothing_else() {
    let Some(f) = tool() else { return };
    let d = tempfile::tempdir().unwrap();
    let src = make(
        &f,
        d.path(),
        "long.mp4",
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=duration=60:size=1280x720:rate=30",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-pix_fmt",
            "yuv420p",
        ],
    );
    let out = d.path().join("never.webm");
    let cancel = CancelToken::new();
    let c2 = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(800));
        c2.cancel();
    });
    let t = std::time::Instant::now();
    let r = f.convert(
        &src,
        "webm",
        &out,
        &ConvertOptions::default(),
        &cancel,
        &nop,
    );
    assert!(matches!(r, Err(Error::Cancelled)), "{r:?}");
    assert!(t.elapsed().as_secs() < 5);
    assert!(src.is_file(), "source must survive");
    let names: Vec<_> = std::fs::read_dir(d.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert!(names.len() <= 2, "{names:?}"); // source + (partial) output only
}

#[test]
fn missing_input_and_failure_messages() {
    let Some(f) = tool() else { return };
    let d = tempfile::tempdir().unwrap();
    let r = f.convert(
        &d.path().join("nope.mp4"),
        "webm",
        &d.path().join("o"),
        &ConvertOptions::default(),
        &CancelToken::new(),
        &nop,
    );
    assert!(matches!(r, Err(Error::NotFound(_))));
    let junk = d.path().join("junk.mp4");
    std::fs::write(&junk, b"not a video").unwrap();
    let r = f.convert(
        &junk,
        "webm",
        &d.path().join("o"),
        &ConvertOptions::default(),
        &CancelToken::new(),
        &nop,
    );
    assert!(matches!(r, Err(Error::ToolFailed(_))), "{r:?}");
}

#[test]
fn duration_without_ffprobe_falls_back_to_ffmpeg_stderr() {
    let Some(mut f) = tool() else { return };
    let d = tempfile::tempdir().unwrap();
    let src = sample_mp4(&f, d.path(), "320x240");
    f.ffprobe = None;
    let dur = f.duration(&src).unwrap();
    assert!((dur - 2.0).abs() < 0.3, "{dur}");
}

/// 321x241 lossless source (yuv444p allows odd sizes), 1 s with a sine track.
fn odd_source(f: &Ffmpeg, dir: &Path, name: &str) -> PathBuf {
    make(
        f,
        dir,
        name,
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=1:size=321x241:rate=10",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=1",
            "-c:v",
            "ffv1",
            "-pix_fmt",
            "yuv444p",
            "-c:a",
            "pcm_s16le",
        ],
    )
}

#[test]
fn compress_trim_speed_handle_odd_dimensions() {
    let Some(f) = tool() else { return };
    let d = tempfile::tempdir().unwrap();
    let c = CancelToken::new();
    let src = odd_source(&f, d.path(), "odd.mkv");

    let comp = d.path().join("c.mkv");
    f.compress(&src, &comp, CompressLevel::Balanced, &c, &nop)
        .unwrap();
    assert!(
        streams(&f, &comp).contains("320,240"),
        "{}",
        streams(&f, &comp)
    );

    let trimmed = d.path().join("t.mkv");
    f.trim(&src, &trimmed, 0, 500, &c, &nop).unwrap();
    assert!(streams(&f, &trimmed).contains("320,240"));

    let fast = d.path().join("s.mkv");
    f.speed(&src, &fast, 2.0, &c, &nop).unwrap();
    assert!(streams(&f, &fast).contains("320,240"));
}

#[test]
fn avi_compress_uses_h264() {
    let Some(f) = tool() else { return };
    let d = tempfile::tempdir().unwrap();
    let c = CancelToken::new();
    let src = odd_source(&f, d.path(), "odd.mkv");
    let avi = d.path().join("in.avi");
    f.convert(&src, "avi", &avi, &ConvertOptions::default(), &c, &nop)
        .unwrap();
    assert!(streams(&f, &avi).contains("mpeg4"));
    let out = d.path().join("c.avi");
    f.compress(&avi, &out, CompressLevel::Light, &c, &nop)
        .unwrap();
    assert!(streams(&f, &out).contains("h264"), "{}", streams(&f, &out));
}

fn leftovers(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with(".fc-palette"))
        .collect()
}

#[test]
fn gif_two_pass_progress_height_limit_and_palette_cleanup() {
    let Some(f) = tool() else { return };
    let d = tempfile::tempdir().unwrap();
    let src = sample_mp4(&f, d.path(), "320x240");
    let out = d.path().join("g.gif");
    let seen = std::sync::Mutex::new(Vec::<f32>::new());
    let o = ConvertOptions {
        max_height: Some(100),
        ..ConvertOptions::default()
    };
    f.convert(&src, "gif", &out, &o, &CancelToken::new(), &|p| {
        seen.lock().unwrap().push(p)
    })
    .unwrap();
    let s = streams(&f, &out);
    assert!(s.contains("gif") && s.contains(",100"), "{s}");
    let seen = seen.lock().unwrap();
    assert!(seen.windows(2).all(|w| w[0] <= w[1] + 1e-6), "{seen:?}");
    assert!(seen.iter().any(|p| (0.45..=0.55).contains(p)), "{seen:?}");
    assert_eq!(*seen.last().unwrap(), 1.0);
    assert!(leftovers(d.path()).is_empty());
}

#[test]
fn gif_failure_and_cancel_remove_the_palette() {
    let Some(f) = tool() else { return };
    let d = tempfile::tempdir().unwrap();
    let junk = d.path().join("junk.mp4");
    std::fs::write(&junk, b"nope").unwrap();
    let r = f.to_gif(
        &junk,
        &d.path().join("g.gif"),
        100,
        10,
        5,
        &CancelToken::new(),
        &nop,
    );
    assert!(r.is_err());
    assert!(leftovers(d.path()).is_empty());

    let src = sample_mp4(&f, d.path(), "320x240");
    let cancel = CancelToken::new();
    cancel.cancel();
    let r = f.to_gif(&src, &d.path().join("g2.gif"), 100, 10, 5, &cancel, &nop);
    assert!(matches!(r, Err(Error::Cancelled)));
    assert!(leftovers(d.path()).is_empty());
}

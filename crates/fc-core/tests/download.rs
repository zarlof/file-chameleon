//! Downloader tests that need no network and no yt-dlp: argument builders, parsers
//! (against hand-trimmed `-J` fixtures) and the dispatcher's offline error paths.

use fc_core::download::ytdlp::{
    download_args, format_selector, is_no_video, js_runtime, map_failure, parse_probe_json,
    parse_progress, probe_args,
};
use fc_core::download::{DlMode, DlQuality, DownloadRequest, probe_any};
use fc_core::engines::ToolLocator;
use fc_core::{CancelToken, Error};
use std::path::{Path, PathBuf};

struct Fake {
    ffmpeg: bool,
    deno: bool,
}

impl ToolLocator for Fake {
    fn locate(&self, tool: &str, file: &str) -> Option<PathBuf> {
        match (tool, self.ffmpeg, self.deno) {
            ("ffmpeg", true, _) => Some(PathBuf::from(format!("C:\\tools\\ffmpeg\\{file}"))),
            ("deno", _, true) => Some(PathBuf::from("C:\\tools\\deno\\deno.exe")),
            _ => None,
        }
    }
}

fn fixture(name: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    std::fs::read_to_string(p).unwrap()
}

fn req(url: &str, mode: DlMode, quality: DlQuality) -> DownloadRequest {
    DownloadRequest {
        url: url.into(),
        mode,
        quality,
        out_dir: PathBuf::from("C:\\Users\\me\\Downloads"),
        title: None,
    }
}

fn arg_after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

// ---------------------------------------------------------------- probe parsing

#[test]
fn youtube_probe_fixture() {
    let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
    let m = parse_probe_json(&fixture("ytdlp-youtube.json"), url).unwrap();
    assert_eq!(m.title, "Sample Video Title ★ with unicode");
    assert_eq!(m.site, "YouTube");
    assert_eq!(m.kind, "video");
    assert_eq!(m.duration_secs, Some(212.0));
    assert_eq!((m.width, m.height), (Some(1920), Some(1080)));
    assert_eq!(m.size_bytes, Some(52_428_800 + 3_355_443));
    assert_eq!(m.uploader.as_deref(), Some("Sample Channel"));
    assert!(m.thumbnail.as_deref().unwrap().starts_with("https://"));
    assert!(!m.direct);
    assert!(m.image_urls.is_empty());
}

#[test]
fn instagram_probe_fixture() {
    let url = "https://www.instagram.com/reel/C1a2B3c4D5e/";
    let m = parse_probe_json(&fixture("ytdlp-instagram.json"), url).unwrap();
    assert_eq!(m.site, "Instagram");
    assert_eq!(m.kind, "video");
    assert_eq!(m.duration_secs, Some(14.5));
    assert_eq!((m.width, m.height), (Some(720), Some(1280)));
    assert_eq!(m.size_bytes, Some(2_411_724));
    assert_eq!(m.uploader.as_deref(), Some("someone"));
}

#[test]
fn carousel_lists_image_urls() {
    let url = "https://www.instagram.com/p/C9z8Y7x6W5v/";
    let m = parse_probe_json(&fixture("ytdlp-carousel.json"), url).unwrap();
    assert_eq!(m.kind, "image");
    assert_eq!(m.image_urls.len(), 2);
    assert_eq!((m.width, m.height), (Some(1080), Some(1350)));
}

#[test]
fn live_streams_are_rejected() {
    let url = "https://www.youtube.com/watch?v=jfKfPfyJRdk";
    let e = parse_probe_json(&fixture("ytdlp-live.json"), url).unwrap_err();
    assert!(matches!(e, Error::Unsupported(_)), "{e:?}");
    assert_eq!(e.code(), "unsupported");
}

#[test]
fn drm_and_garbage_are_rejected() {
    let drm = r#"{"title":"x","has_drm":true}"#;
    assert!(matches!(
        parse_probe_json(drm, "https://vimeo.com/1"),
        Err(Error::Unsupported(_))
    ));
    assert!(parse_probe_json("not json", "https://vimeo.com/1").is_err());
}

// ---------------------------------------------------------------- progress parsing

#[test]
fn progress_lines() {
    let p = parse_progress("FC 1048576 10485760 NA 524288.5 18").unwrap();
    assert_eq!(p.downloaded, 1_048_576);
    assert_eq!(p.total, Some(10_485_760));
    assert_eq!(p.speed_bps, Some(524_288));
    assert_eq!(p.eta_secs, Some(18));
    assert!((p.fraction().unwrap() - 0.1).abs() < 1e-6);

    // estimate used when the exact total is NA
    let p = parse_progress("FC 500 NA 1000.0 NA NA").unwrap();
    assert_eq!(p.total, Some(1000));
    assert_eq!(p.speed_bps, None);
    assert_eq!(p.eta_secs, None);

    // everything unknown
    let p = parse_progress("FC NA NA NA NA NA").unwrap();
    assert_eq!(p.downloaded, 0);
    assert_eq!(p.fraction(), None);

    assert!(parse_progress("[download] 12%").is_none());
    assert!(parse_progress("FC 1 2").is_none());
    assert!(parse_progress("C:\\out\\video.mp4").is_none());
}

// ---------------------------------------------------------------- argument builders

#[test]
fn format_selectors() {
    use DlMode::*;
    use DlQuality::*;
    assert_eq!(format_selector(Original, Best), "bv*+ba/b");
    assert_eq!(
        format_selector(Original, P1080),
        "bv*[height<=1080]+ba/b[height<=1080]"
    );
    assert_eq!(
        format_selector(Video, Best),
        "bv*[vcodec^=avc1]+ba[ext=m4a]/bv*[ext=mp4]+ba[ext=m4a]/bv*+ba/b"
    );
    assert_eq!(
        format_selector(Video, P720),
        "bv*[height<=720][vcodec^=avc1]+ba[ext=m4a]/bv*[height<=720][ext=mp4]+ba[ext=m4a]/bv*[height<=720]+ba/b[height<=720]"
    );
    assert_eq!(format_selector(Audio, Best), "ba/b");
    // GIF always works from <= 720p video, whatever the quality choice
    assert_eq!(format_selector(Gif, Best), format_selector(Video, P720));
}

#[test]
fn probe_args_are_safe_and_complete() {
    let tools = Fake {
        ffmpeg: true,
        deno: false,
    };
    let a = probe_args(&tools, "https://vimeo.com/123", Some("edge"));
    for flag in [
        "--ignore-config",
        "--no-update",
        "--no-playlist",
        "--newline",
        "-J",
    ] {
        assert!(a.iter().any(|x| x == flag), "{flag}");
    }
    assert_eq!(
        arg_after(&a, "--ffmpeg-location"),
        Some("C:\\tools\\ffmpeg")
    );
    assert_eq!(arg_after(&a, "--cookies-from-browser"), Some("edge"));
    // the URL comes last, after `--`, so it can never be read as an option
    assert_eq!(&a[a.len() - 2..], ["--", "https://vimeo.com/123"]);
    // not YouTube: no JS runtime needed
    assert!(!a.iter().any(|x| x == "--js-runtimes"));
}

#[test]
fn only_the_first_item_of_multi_item_posts() {
    let tools = Fake {
        ffmpeg: true,
        deno: false,
    };
    let url = "https://www.instagram.com/p/C9z8Y7x6W5v/";
    let p = probe_args(&tools, url, None);
    assert_eq!(arg_after(&p, "--playlist-items"), Some("1"));
    let d = download_args(
        &req(url, DlMode::Original, DlQuality::Best),
        &tools,
        Path::new("C:\\stage"),
        None,
    );
    assert_eq!(arg_after(&d, "--playlist-items"), Some("1"));
}

#[test]
fn warnings_do_not_decide_the_error() {
    let yt = "https://www.youtube.com/watch?v=abc";
    let stderr = "WARNING: [youtube] abc: Some formats have been skipped as they are DRM protected\n\
                  ERROR: [youtube] abc: Private video. Sign in if you've been granted access";
    match map_failure(stderr, yt, true) {
        Error::ToolFailed(m) => assert!(m.contains("Private video"), "{m}"),
        e => panic!("{e:?}"),
    }
    // a real DRM error is still reported as such
    assert!(matches!(
        map_failure("ERROR: [x] This video is DRM protected", yt, true),
        Error::Unsupported(_)
    ));
}

#[test]
fn no_cookies_no_ffmpeg() {
    let tools = Fake {
        ffmpeg: false,
        deno: false,
    };
    let a = probe_args(&tools, "https://vimeo.com/123", None);
    assert!(!a.iter().any(|x| x == "--cookies-from-browser"));
    assert!(!a.iter().any(|x| x == "--ffmpeg-location"));
}

#[test]
fn download_args_per_mode() {
    let tools = Fake {
        ffmpeg: true,
        deno: false,
    };
    let staging = Path::new("C:\\stage\\7");
    let url = "https://vimeo.com/123";

    let a = download_args(
        &req(url, DlMode::Video, DlQuality::P1080),
        &tools,
        staging,
        None,
    );
    assert_eq!(
        arg_after(&a, "-f"),
        Some(
            "bv*[height<=1080][vcodec^=avc1]+ba[ext=m4a]/bv*[height<=1080][ext=mp4]+ba[ext=m4a]/bv*[height<=1080]+ba/b[height<=1080]"
        )
    );
    assert_eq!(arg_after(&a, "--merge-output-format"), Some("mp4"));
    assert_eq!(arg_after(&a, "--remux-video"), Some("mp4"));
    assert_eq!(arg_after(&a, "--print"), Some("after_move:filepath"));
    assert_eq!(arg_after(&a, "-o"), Some("%(title).150B [%(id)s].%(ext)s"));
    assert!(a.iter().any(|x| x == "--progress"));
    assert!(a.iter().any(|x| {
        x.starts_with("download:FC %(progress.downloaded_bytes)s %(progress.total_bytes)s")
    }));
    // work happens in the scratch folder, not the destination
    assert!(a.iter().any(|x| x.starts_with("home:C:\\stage\\7")));
    assert!(a.iter().any(|x| x.starts_with("temp:C:\\stage\\7")));
    assert!(!a.iter().any(|x| x.contains("Downloads")));

    let a = download_args(
        &req(url, DlMode::Audio, DlQuality::Best),
        &tools,
        staging,
        None,
    );
    assert_eq!(arg_after(&a, "-f"), Some("ba/b"));
    assert!(a.iter().any(|x| x == "-x"));
    assert_eq!(arg_after(&a, "--audio-format"), Some("mp3"));
    assert_eq!(arg_after(&a, "--audio-quality"), Some("0"));
    assert!(!a.iter().any(|x| x == "--merge-output-format"));

    let a = download_args(
        &req(url, DlMode::Original, DlQuality::Best),
        &tools,
        staging,
        None,
    );
    assert_eq!(arg_after(&a, "-f"), Some("bv*+ba/b"));
    assert!(!a.iter().any(|x| x == "--remux-video" || x == "-x"));

    let a = download_args(
        &req(url, DlMode::Gif, DlQuality::Best),
        &tools,
        staging,
        None,
    );
    assert!(arg_after(&a, "-f").unwrap().contains("height<=720"));
    assert_eq!(arg_after(&a, "--merge-output-format"), Some("mp4"));
}

#[test]
fn js_runtime_selection() {
    let yt = "https://www.youtube.com/watch?v=abc";
    let with_deno = Fake {
        ffmpeg: false,
        deno: true,
    };
    let none = Fake {
        ffmpeg: false,
        deno: false,
    };
    // Node wins over Deno
    assert_eq!(
        js_runtime(yt, Some(Path::new("C:\\node\\node.exe")), &with_deno).as_deref(),
        Some("node:C:\\node\\node.exe")
    );
    assert_eq!(
        js_runtime(yt, None, &with_deno).as_deref(),
        Some("deno:C:\\tools\\deno\\deno.exe")
    );
    assert_eq!(js_runtime(yt, None, &none), None);
    // other sites never need one
    assert_eq!(
        js_runtime("https://vimeo.com/1", Some(Path::new("n")), &with_deno),
        None
    );
}

// ---------------------------------------------------------------- error mapping

#[test]
fn failure_mapping() {
    let yt = "https://www.youtube.com/watch?v=abc";
    let vimeo = "https://vimeo.com/1";
    assert!(matches!(
        map_failure(
            "ERROR: [youtube] abc: This video is DRM protected",
            yt,
            true
        ),
        Error::Unsupported(_)
    ));
    assert!(matches!(
        map_failure("ERROR: This live event will begin in 2 hours", yt, true),
        Error::Unsupported(_)
    ));
    assert!(matches!(
        map_failure("ERROR: Unsupported URL: https://example.com/", vimeo, false),
        Error::Unsupported(_)
    ));
    let nv = map_failure(
        "ERROR: [Instagram] C1: There is no video in this post",
        vimeo,
        false,
    );
    assert!(is_no_video(&nv));
    assert!(!is_no_video(&Error::Unsupported("other".into())));

    // YouTube without a JS runtime points at deno; with one it is a plain tool failure
    let msg = "WARNING: [youtube] No supported JavaScript runtime could be found\nERROR: Requested format is not available";
    match map_failure(msg, yt, false) {
        Error::MissingTool(t) => assert_eq!(t, "deno"),
        e => panic!("{e:?}"),
    }
    assert!(matches!(map_failure(msg, yt, true), Error::ToolFailed(_)));
    assert!(matches!(
        map_failure(
            "ERROR: Unable to download webpage: <urlopen error getaddrinfo failed>",
            vimeo,
            false
        ),
        Error::Network(_)
    ));
    match map_failure("noise\nERROR: Private video\n", vimeo, false) {
        Error::ToolFailed(m) => assert_eq!(m, "Private video"),
        e => panic!("{e:?}"),
    }
}

// ---------------------------------------------------------------- offline dispatcher paths

#[test]
fn missing_ytdlp_is_a_missing_tool_error() {
    let tools = Fake {
        ffmpeg: true,
        deno: false,
    };
    let e = probe_any(&tools, "https://vimeo.com/123", None, &CancelToken::new()).unwrap_err();
    assert_eq!(e.code(), "missingTool");
    match e {
        Error::MissingTool(t) => assert_eq!(t, "yt-dlp"),
        e => panic!("{e:?}"),
    }
}

#[test]
fn non_links_are_unsupported() {
    let tools = Fake {
        ffmpeg: true,
        deno: false,
    };
    let e = probe_any(&tools, "hello there", None, &CancelToken::new()).unwrap_err();
    assert_eq!(e.code(), "unsupported");
}

#[test]
fn download_without_tools_fails_before_spawning() {
    let tools = Fake {
        ffmpeg: true,
        deno: false,
    };
    let dir = std::env::temp_dir().join("fc-dl-test-missing");
    let r = req("https://vimeo.com/123", DlMode::Video, DlQuality::Best);
    let e =
        fc_core::download::download_any(&tools, &r, &dir, None, &mut |_| {}, &CancelToken::new())
            .unwrap_err();
    assert_eq!(e.code(), "missingTool");
    let no_ffmpeg = Fake {
        ffmpeg: false,
        deno: false,
    };
    let e = fc_core::download::download_any(
        &no_ffmpeg,
        &r,
        &dir,
        None,
        &mut |_| {},
        &CancelToken::new(),
    )
    .unwrap_err();
    assert!(matches!(e, Error::MissingTool(t) if t == "ffmpeg"));
    assert!(!dir.exists());
}

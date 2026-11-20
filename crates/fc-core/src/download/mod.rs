//! Media downloader: classify a link, probe it (yt-dlp `-J` or HTTP HEAD for direct media),
//! then download with progress and cancel. OWNER: Task F.

pub mod classify;
pub mod direct;
pub mod ytdlp;

use crate::engines::ToolLocator;
use crate::{CancelToken, Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub use classify::{LinkInfo, LinkKind, classify_url};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DlMode {
    /// Best original file (images: the image; video: best merged, original container).
    #[default]
    Original,
    /// MP4 (H.264/AAC preferred, remux/re-encode when needed).
    Video,
    /// MP3 (VBR quality 0).
    Audio,
    /// Animated GIF made from the video (fps 15, width <= 480, first 60 s).
    Gif,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DlQuality {
    #[default]
    Best,
    P1080,
    P720,
}

/// What the preview card shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub url: String,
    pub title: String,
    /// Human site name ("YouTube", "Instagram", ...) or the host.
    pub site: String,
    /// "video" | "audio" | "image"
    pub kind: String,
    pub duration_secs: Option<f64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Best-effort size of the default download.
    pub size_bytes: Option<u64>,
    pub thumbnail: Option<String>,
    pub uploader: Option<String>,
    /// Direct links skip yt-dlp entirely.
    pub direct: bool,
    /// Image posts with several images (Instagram carousel via og:image is 1).
    pub image_urls: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadRequest {
    pub url: String,
    #[serde(default)]
    pub mode: DlMode,
    #[serde(default)]
    pub quality: DlQuality,
    /// Destination folder (the UI fills in the user's Downloads folder by default).
    pub out_dir: PathBuf,
    /// Title from the preview, shown in the list while downloading (the URL is used if absent).
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DlState {
    Probing,
    Downloading,
    Processing,
    Done,
    Failed,
    Cancelled,
}

/// Sent on every change (event `download://update`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadSnapshot {
    pub id: u64,
    pub url: String,
    pub title: String,
    pub state: DlState,
    /// 0.0..=1.0, None while unknown.
    pub progress: Option<f32>,
    pub speed_bps: Option<u64>,
    pub eta_secs: Option<u64>,
    pub output: Option<PathBuf>,
    pub error_code: Option<String>,
    pub error: Option<String>,
    pub created_at: u64,
}

/// Progress of one transfer, reported by the downloaders (rate limited by the producer).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DlProgress {
    /// 0.0..=1.0, `None` while unknown.
    pub fraction: Option<f32>,
    pub speed_bps: Option<u64>,
    pub eta_secs: Option<u64>,
    /// Post-processing (merging, converting) rather than transferring.
    pub processing: bool,
}

fn link_of(url: &str) -> Result<LinkInfo> {
    classify_url(url).ok_or_else(|| Error::Unsupported("not a web link".into()))
}

/// Probe any link: direct files by HTTP HEAD, everything else through yt-dlp. When yt-dlp
/// reports a post without video (an Instagram photo), fall back to the page's `og:image`.
pub fn probe_any(
    tools: &dyn ToolLocator,
    url: &str,
    cookies_browser: Option<&str>,
    cancel: &CancelToken,
) -> Result<MediaInfo> {
    let link = link_of(url)?;
    if matches!(link.kind, LinkKind::DirectImage | LinkKind::DirectMedia) {
        return direct::probe_direct(&link.url);
    }
    match ytdlp::probe(tools, &link.url, cookies_browser, cancel) {
        Err(e) if ytdlp::is_no_video(&e) => {
            let Some(image) = direct::og_image(&link.url) else {
                return Err(e);
            };
            let Ok(mut info) = direct::probe_direct(&image) else {
                return Err(e);
            };
            info.kind = "image".into();
            info.site = link.site.map(str::to_string).unwrap_or(link.host);
            Ok(info)
        }
        other => other,
    }
}

/// Download any link into `req.out_dir` (routing as in [`probe_any`]). `staging` is a scratch
/// folder owned by this download; it is removed afterwards. Direct links ignore the mode.
/// The finished file carries the Mark of the Web (see [`mark_of_the_web`]).
pub fn download_any(
    tools: &dyn ToolLocator,
    req: &DownloadRequest,
    staging: &Path,
    cookies_browser: Option<&str>,
    progress: &mut dyn FnMut(DlProgress),
    cancel: &CancelToken,
) -> Result<PathBuf> {
    let link = link_of(&req.url)?;
    let mut req = req.clone();
    req.url = link.url.clone();
    let result = if matches!(link.kind, LinkKind::DirectImage | LinkKind::DirectMedia) {
        direct::download_direct(&req.url, &req.out_dir, progress, cancel)
    } else {
        match ytdlp::download(tools, &req, staging, cookies_browser, progress, cancel) {
            Err(e) if ytdlp::is_no_video(&e) => match direct::og_image(&req.url) {
                Some(image) => direct::download_direct(&image, &req.out_dir, progress, cancel),
                None => Err(e),
            },
            other => other,
        }
    };
    if let Ok(path) = &result {
        // Best effort: FAT/exFAT volumes have no alternate data streams.
        let _ = mark_of_the_web(path, &req.url);
    }
    result
}

/// Mark a downloaded file as coming from the internet, like browsers do: an NTFS alternate data
/// stream `<file>:Zone.Identifier` with `ZoneId=3`, so Windows (SmartScreen, Office protected
/// view) treats it with care. No-op on other systems.
pub fn mark_of_the_web(path: &Path, url: &str) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        let host_url: String = url.chars().filter(|c| !c.is_control()).collect();
        let mut stream = path.as_os_str().to_owned();
        stream.push(":Zone.Identifier");
        std::fs::write(
            PathBuf::from(stream),
            format!("[ZoneTransfer]\r\nZoneId=3\r\nHostUrl={host_url}\r\n"),
        )
    }
    #[cfg(not(windows))]
    {
        let _ = (path, url);
        Ok(())
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn mark_of_the_web_writes_the_zone_stream() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("clip.mp4");
        std::fs::write(&file, b"data").unwrap();
        mark_of_the_web(&file, "https://example.com/a.mp4?x=1\r\nZoneId=0").unwrap();
        let mut stream = file.as_os_str().to_owned();
        stream.push(":Zone.Identifier");
        assert_eq!(
            std::fs::read_to_string(PathBuf::from(stream)).unwrap(),
            "[ZoneTransfer]\r\nZoneId=3\r\nHostUrl=https://example.com/a.mp4?x=1ZoneId=0\r\n"
        );
        // The file itself is untouched and no extra directory entry appears.
        assert_eq!(std::fs::read(&file).unwrap(), b"data");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}

//! Media downloader: classify a link, probe it (yt-dlp `-J` or HTTP HEAD for direct media),
//! then download with progress and cancel. OWNER: Task F.

pub mod classify;
pub mod direct;
pub mod ytdlp;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

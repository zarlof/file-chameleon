//! Job model shared by the queue, the engines and the UI.

pub mod naming;
pub mod queue;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub type JobId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CompressLevel {
    Light,
    #[default]
    Balanced,
    Strong,
}

/// What a job does. `Convert` is the format change; the rest are Tools-panel actions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Operation {
    /// Change format to `to` (a format id). One job per input file.
    Convert { to: String },
    /// All inputs (pdf and/or raster images, in order) -> one PDF. One job total.
    MergePdf,
    /// One PDF -> one PDF per page, into a folder named after the input.
    SplitPdf,
    /// Re-encode smaller, same format. Image (quality), video (CRF), audio (bitrate), pdf (lopdf compress).
    Compress { level: CompressLevel },
    /// Video/audio: keep `[start_ms, end_ms)`.
    Trim { start_ms: u64, end_ms: u64 },
    /// Video: drop audio track.
    Mute,
    /// Video/audio: playback speed factor, 0.25..=4.0.
    Speed { factor: f32 },
    /// Image/video: crop rectangle in source pixels.
    Crop {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ConvertOptions {
    /// 1..=100, `None` = engine default (images 85, video CRF 23 equivalent).
    pub quality: Option<u8>,
    /// Downscale to fit, keep aspect ratio. Never upscales.
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    /// `None` = next to the source file.
    pub output_dir: Option<PathBuf>,
    /// Base name (no extension, no folders) for outputs instead of the input's stem. The shell
    /// sets it for clipboard images, whose stored file names mean nothing to the user.
    pub output_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobRequest {
    pub inputs: Vec<PathBuf>,
    pub op: Operation,
    #[serde(default)]
    pub options: ConvertOptions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobState {
    Queued,
    Running,
    Done,
    Failed,
    Cancelled,
}

impl JobState {
    pub fn is_finished(self) -> bool {
        matches!(
            self,
            JobState::Done | JobState::Failed | JobState::Cancelled
        )
    }
}

/// Full job state, sent to the UI on every change (event `job://update`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobSnapshot {
    pub id: JobId,
    pub inputs: Vec<PathBuf>,
    pub op: Operation,
    pub state: JobState,
    /// 0.0..=1.0
    pub progress: f32,
    pub outputs: Vec<PathBuf>,
    /// Error code from `Error::code()` plus detail, when `state == Failed`.
    pub error_code: Option<String>,
    pub error: Option<String>,
    pub created_at: u64,
    pub finished_at: Option<u64>,
}

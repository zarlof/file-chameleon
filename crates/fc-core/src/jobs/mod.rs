//! Job model shared by the queue, the engines and the UI.

pub mod naming;
pub mod queue;

use crate::{Error, Result};
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
    /// Raster image edits applied in order, then `options` max size. `to` = output format id,
    /// `None` = keep the input format. One job per input.
    Edit {
        ops: Vec<ImageOp>,
        to: Option<String>,
    },
    /// Centre crop to the aspect ratio `w:h` (images in-process, video with ffmpeg).
    CropAspect { w: u32, h: u32 },
    /// Copy (or move) each input into `dir`. `None` = the shell asks for a folder first; such a
    /// request is never queued.
    CopyTo {
        dir: Option<PathBuf>,
        move_files: bool,
    },
    /// All inputs -> one `.zip` next to the first input. One job total.
    Zip,
    /// `.zip` -> a new folder named after the archive.
    Extract,
    /// Opens the image editor in the shell. Never queued.
    Annotate,
    /// Copies the input paths to the clipboard as text. Handled by the shell, never queued.
    CopyPath,
    /// Opens a terminal in the input's folder. Handled by the shell, never queued.
    OpenTerminal,
    /// Pins the image(s) on screen in a floating window. Handled by the shell, never queued.
    Float,
}

/// One image edit step (`Operation::Edit`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ImageOp {
    Grayscale,
    RotateLeft,
    RotateRight,
    Rotate180,
    FlipH,
    FlipV,
    /// Crop away fully transparent borders (no-op when there are none).
    Trim,
    /// Square blocks of `block` px, 2..=256.
    Pixelate {
        block: u32,
    },
    /// Gaussian blur, sigma 0.5..=50.
    Blur {
        sigma: f32,
    },
    /// Resize to `percent` of the size, 1..=400.
    Scale {
        percent: u32,
    },
}

fn invalid(msg: impl Into<String>) -> Error {
    Error::Invalid(msg.into())
}

impl ImageOp {
    /// Parameter ranges.
    pub fn validate(&self) -> Result<()> {
        match *self {
            ImageOp::Pixelate { block } if !(2..=256).contains(&block) => {
                Err(invalid(format!("pixelate block {block} (2..=256)")))
            }
            ImageOp::Blur { sigma } if !(0.5..=50.0).contains(&sigma) => {
                Err(invalid(format!("blur sigma {sigma} (0.5..=50)")))
            }
            ImageOp::Scale { percent } if !(1..=400).contains(&percent) => {
                Err(invalid(format!("scale {percent}% (1..=400)")))
            }
            _ => Ok(()),
        }
    }
}

/// Raster formats `Operation::Edit` can write.
fn is_raster(id: &str) -> bool {
    crate::formats::by_id(id).is_some_and(|f| f.kind == crate::formats::Kind::Image)
}

impl Operation {
    /// Whether the queue can run this operation. `Annotate` (opens the editor) and `CopyTo`
    /// without a folder (needs a folder picker) are resolved by the shell first and are
    /// never queued.
    pub fn queueable(&self) -> Result<()> {
        match self {
            Operation::Annotate => Err(invalid("annotate opens the image editor")),
            Operation::CopyPath | Operation::OpenTerminal | Operation::Float => {
                Err(invalid("file actions are handled by the shell"))
            }
            Operation::CopyTo { dir: None, .. } => Err(invalid("no target folder chosen")),
            _ => Ok(()),
        }
    }

    /// Parameter ranges and known format ids (used for user presets before they are saved).
    pub fn validate(&self) -> Result<()> {
        match self {
            Operation::Convert { to } if crate::formats::by_id(to).is_none() => {
                Err(invalid(format!("unknown format {to}")))
            }
            Operation::Edit { ops, to } => {
                if let Some(to) = to
                    && !is_raster(to)
                {
                    return Err(invalid(format!("edit output must be an image, not {to}")));
                }
                ops.iter().try_for_each(ImageOp::validate)
            }
            Operation::CropAspect { w, h }
                if !(1..=1000).contains(w) || !(1..=1000).contains(h) =>
            {
                Err(invalid(format!("aspect {w}:{h}")))
            }
            Operation::Speed { factor } if !(0.25..=4.0).contains(factor) => {
                Err(invalid(format!("speed {factor} (0.25..=4)")))
            }
            Operation::Trim { start_ms, end_ms } if end_ms <= start_ms => {
                Err(invalid("empty trim range"))
            }
            Operation::Crop { width, height, .. } if *width == 0 || *height == 0 => {
                Err(invalid("empty crop rectangle"))
            }
            Operation::CopyTo { dir: Some(d), .. } if d.as_os_str().is_empty() => {
                Err(invalid("empty folder"))
            }
            _ => Ok(()),
        }
    }
}

impl ConvertOptions {
    /// quality 1..=100, max sizes at least 1 px.
    pub fn validate(&self) -> Result<()> {
        if let Some(q) = self.quality
            && !(1..=100).contains(&q)
        {
            return Err(invalid(format!("quality {q} (1..=100)")));
        }
        if self.max_width == Some(0) || self.max_height == Some(0) {
            return Err(invalid("max size 0"));
        }
        Ok(())
    }
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

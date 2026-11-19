//! Conversion engines and the job executor that chains them along a planned route.
//!
//! Ownership: `mod.rs` (DefaultExecutor) = Task A; `image.rs`, `svg.rs`, `pdf.rs`,
//! `data.rs` = Task B; `ffmpeg.rs` = Task C. Engine functions are plain functions
//! with fixed signatures (see each file) so tasks can be built in parallel.
//!
//! Contract for every engine function:
//! - Read only from the given inputs; write only to the given `output` path
//!   (the executor hands a temp path and commits it afterwards).
//! - Check `cancel` between units of work; return `Err(Error::Cancelled)` promptly.
//! - Report progress in 0.0..=1.0 through `progress` when meaningful.
//! - Never panic on malformed input; return `Error::Failed`/`Unsupported`.

pub mod data;
pub mod ffmpeg;
pub mod image;
pub mod pdf;
pub mod svg;

use crate::jobs::{ConvertOptions, Operation};
use crate::{CancelToken, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Finds external tool files. Implemented by `engine_mgr::EngineManager`.
/// Queried per job so a tool installed while the app runs is picked up.
pub trait ToolLocator: Send + Sync {
    /// `tool` is a manifest id ("ffmpeg", "pdfium"); `file` a file it provides
    /// ("ffmpeg.exe", "ffprobe.exe", "pdfium.dll").
    fn locate(&self, tool: &str, file: &str) -> Option<PathBuf>;
}

/// Runs one job to completion and returns the committed output paths.
pub trait Executor: Send + Sync {
    fn run(
        &self,
        inputs: &[PathBuf],
        op: &Operation,
        options: &ConvertOptions,
        cancel: &CancelToken,
        progress: &dyn Fn(f32),
    ) -> Result<Vec<PathBuf>>;
}

/// The real executor.
/// - `Convert`: detect the input format, `formats::planner::plan` a route, run each step
///   into a temp file (intermediates in a temp dir removed afterwards), name the final file
///   with `jobs::naming::output_path`, write to `naming::temp_path`, then `naming::commit`.
///   Overall progress = step index + step progress, divided by step count.
///   pdf -> raster renders every page: 1 page -> one file; N pages -> a new folder named
///   after the input (unique) holding `<stem>-<n>.<ext>`.
/// - `MergePdf` -> `pdf::merge`; output `<first stem> (merged).pdf`.
/// - `SplitPdf` -> `pdf::split` into a unique folder named after the input.
/// - `Compress` -> by input kind: image -> `image::compress`, video/audio -> `ffmpeg`
///   `compress`, pdf -> `pdf::compress`; suffix " (compressed)".
/// - `Trim`/`Mute`/`Speed` -> ffmpeg; suffixes " (trimmed)", " (muted)", " (speed)".
/// - `Crop` -> image or ffmpeg by kind; suffix " (cropped)".
/// - Any panic inside an engine is caught (`catch_unwind`) and reported as `Error::Failed`.
/// - On error or cancel, temp/partial files are removed.
pub struct DefaultExecutor {
    pub tools: Arc<dyn ToolLocator>,
}

impl Executor for DefaultExecutor {
    fn run(
        &self,
        inputs: &[PathBuf],
        op: &Operation,
        options: &ConvertOptions,
        cancel: &CancelToken,
        progress: &dyn Fn(f32),
    ) -> Result<Vec<PathBuf>> {
        let _ = (inputs, op, options, cancel, progress, &self.tools);
        todo!("Task A")
    }
}

/// Helper used by engines: resolve the output encoding for raster images.
pub(crate) fn quality_or(options: &ConvertOptions, default: u8) -> u8 {
    options.quality.unwrap_or(default).clamp(1, 100)
}

/// Convenience used in tests and the executor.
pub fn ext_of(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
}

//! ffmpeg engine (external process via `crate::proc`). OWNER: Task C.
//! Every run: `-hide_banner -nostdin -y -loglevel error -progress pipe:1 -nostats`,
//! progress parsed from `out_time_us=` against the duration from ffprobe
//! (fall back to parsing `Duration:` from `ffmpeg -i` stderr when ffprobe is missing).
//! Cancel kills the process tree; the caller deletes the partial output.

use crate::jobs::{CompressLevel, ConvertOptions};
use crate::{CancelToken, Result};
use std::path::{Path, PathBuf};

pub struct Ffmpeg {
    pub ffmpeg: PathBuf,
    pub ffprobe: Option<PathBuf>,
}

/// Parsed `-progress` key/value block. Returns `Some(out_time_seconds)` when the block has it.
pub fn parse_progress_line(line: &str) -> Option<f64> {
    let _ = line;
    todo!("Task C")
}

impl Ffmpeg {
    /// Media duration in seconds (None if unknown, e.g. still image).
    pub fn duration(&self, input: &Path) -> Option<f64> {
        let _ = input;
        todo!("Task C")
    }

    /// Format conversion. Codec choices:
    /// mp4/mov -> libx264 (CRF from quality, default 23, preset veryfast, yuv420p, +faststart) + aac 160k;
    /// mkv -> libx264 + aac; webm -> libvpx-vp9 (CRF 32, -b:v 0, row-mt) + libopus 128k; avi -> mpeg4 q:v 4 + mp3;
    /// gif -> two-pass palette (fps 15, width min(src,480), lanczos, max 60 s);
    /// audio: mp3 libmp3lame q:a 2, m4a aac 192k, opus/ogg libopus 128k, flac, wav pcm_s16le; video->audio drops video (-vn).
    /// Try stream copy first when the target container accepts the source codecs (mp4<->mov<->mkv with h264/aac).
    /// Honors `max_width`/`max_height` with scale filter (even dimensions).
    pub fn convert(
        &self,
        input: &Path,
        to: &str,
        output: &Path,
        options: &ConvertOptions,
        cancel: &CancelToken,
        progress: &dyn Fn(f32),
    ) -> Result<()> {
        let _ = (input, to, output, options, cancel, progress);
        todo!("Task C")
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
        let _ = (input, output, level, cancel, progress);
        todo!("Task C")
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
        let _ = (input, output, start_ms, end_ms, cancel, progress);
        todo!("Task C")
    }

    /// Remove audio (`-an -c:v copy`).
    pub fn mute(
        &self,
        input: &Path,
        output: &Path,
        cancel: &CancelToken,
        progress: &dyn Fn(f32),
    ) -> Result<()> {
        let _ = (input, output, cancel, progress);
        todo!("Task C")
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
        let _ = (input, output, factor, cancel, progress);
        todo!("Task C")
    }

    /// crop=w:h:x:y (even values), audio copied.
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
        let _ = (input, output, x, y, width, height, cancel, progress);
        todo!("Task C")
    }

    /// Video -> GIF for the downloader's GIF mode (same palette pipeline as `convert` to gif).
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
        let _ = (input, output, max_width, fps, max_seconds, cancel, progress);
        todo!("Task C")
    }
}

//! Raster image engine (`image` crate; lossy WebP via `webp`). OWNER: Task B.
//! Limits: refuse inputs over 200 megapixels (`Error::TooLarge`) before decoding pixels
//! (read dimensions first). GIF input decodes the first frame only.

use crate::jobs::{CompressLevel, ConvertOptions};
use crate::{CancelToken, Result};
use std::path::Path;

pub const MAX_PIXELS: u64 = 200_000_000;

/// Convert `input` to raster format `to` ("png","jpg","webp","gif","bmp","tiff","ico").
/// Applies `max_width`/`max_height` (downscale only, Lanczos3). JPG has no alpha:
/// flatten onto white. WebP: quality 100 => lossless, else lossy at `quality` (default 85).
/// ICO: multi-size icon (16, 32, 48, 64, 128, 256 — only sizes <= source's smaller side, at least 16).
pub fn convert(
    input: &Path,
    to: &str,
    output: &Path,
    options: &ConvertOptions,
    cancel: &CancelToken,
) -> Result<()> {
    let _ = (input, to, output, options, cancel);
    todo!("Task B")
}

/// Encode an already-decoded image to `to`, same rules as [`convert`]. Used by svg/pdfium.
pub fn encode(
    img: &image::DynamicImage,
    to: &str,
    output: &Path,
    options: &ConvertOptions,
) -> Result<()> {
    let _ = (img, to, output, options);
    todo!("Task B")
}

/// Same format, smaller file. jpg/webp: re-encode at quality Light 85 / Balanced 72 / Strong 55;
/// png: re-encode with best compression (Strong also quantizes nothing — keep lossless);
/// other formats: re-encode as-is. If the result is not smaller, copy the original bytes.
pub fn compress(
    input: &Path,
    output: &Path,
    level: CompressLevel,
    cancel: &CancelToken,
) -> Result<()> {
    let _ = (input, output, level, cancel);
    todo!("Task B")
}

/// Crop to the rectangle (clamped to the image bounds; empty result => `Error::Failed`).
pub fn crop(input: &Path, output: &Path, x: u32, y: u32, width: u32, height: u32) -> Result<()> {
    let _ = (input, output, x, y, width, height);
    todo!("Task B")
}

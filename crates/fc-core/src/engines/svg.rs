//! SVG rasterizer (`resvg`). OWNER: Task B.

use crate::jobs::ConvertOptions;
use crate::{CancelToken, Result};
use std::path::Path;

/// Render `input` (.svg) and encode it to `to` via `image::encode`.
/// Size: the SVG's intrinsic size, scaled so the longer side is at least 1024 px
/// (vector art should not come out tiny), then `max_width`/`max_height` fit applies.
/// Hard cap 8192 px on the longer side.
pub fn rasterize(
    input: &Path,
    to: &str,
    output: &Path,
    options: &ConvertOptions,
    cancel: &CancelToken,
) -> Result<()> {
    let _ = (input, to, output, options, cancel);
    todo!("Task B")
}

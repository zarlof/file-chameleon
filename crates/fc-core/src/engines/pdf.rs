//! PDF engine. lopdf (in-process): images -> pdf, merge, split, compress.
//! pdfium (on-demand dll, dynamic binding): pdf -> raster pages. OWNER: Task B.

use crate::jobs::ConvertOptions;
use crate::{CancelToken, Result};
use std::path::{Path, PathBuf};

/// Raster images -> one PDF, one image per page, page size = image size at 72 dpi
/// (so 1 px = 1 pt), in input order. JPEG embedded as-is (DCTDecode); everything else
/// decoded and embedded as Flate RGB (alpha flattened onto white).
pub fn images_to_pdf(
    inputs: &[PathBuf],
    output: &Path,
    cancel: &CancelToken,
    progress: &dyn Fn(f32),
) -> Result<()> {
    let _ = (inputs, output, cancel, progress);
    todo!("Task B")
}

/// PDFs and/or raster images (in order) -> one PDF.
pub fn merge(
    inputs: &[PathBuf],
    output: &Path,
    cancel: &CancelToken,
    progress: &dyn Fn(f32),
) -> Result<()> {
    let _ = (inputs, output, cancel, progress);
    todo!("Task B")
}

/// One PDF -> `<stem>-<n>.pdf` per page inside existing directory `out_dir`. Returns files in page order.
pub fn split(
    input: &Path,
    out_dir: &Path,
    cancel: &CancelToken,
    progress: &dyn Fn(f32),
) -> Result<Vec<PathBuf>> {
    let _ = (input, out_dir, cancel, progress);
    todo!("Task B")
}

/// Rewrite with object streams + compressed streams, drop unused objects.
pub fn compress(input: &Path, output: &Path) -> Result<()> {
    let _ = (input, output);
    todo!("Task B")
}

/// Number of pages (used by the executor to decide single file vs folder).
pub fn page_count(input: &Path) -> Result<usize> {
    let _ = input;
    todo!("Task B")
}

/// Render every page with pdfium loaded from `pdfium_dll`, at 150 dpi (longer side capped
/// at 4096 px, then `max_width`/`max_height` fit), encoded via `image::encode` to `to`
/// ("png" | "jpg"). `outputs[i]` is the destination for page i (len must equal page count).
pub fn render_pages(
    pdfium_dll: &Path,
    input: &Path,
    to: &str,
    outputs: &[PathBuf],
    options: &ConvertOptions,
    cancel: &CancelToken,
    progress: &dyn Fn(f32),
) -> Result<()> {
    let _ = (pdfium_dll, input, to, outputs, options, cancel, progress);
    todo!("Task B")
}

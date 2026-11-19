//! Output file naming. Sources are never overwritten; collisions get " (1)", " (2)", ...
//! Work is written to a temp path next to the destination, then renamed into place.
//! OWNER: Task A. Replace the `todo!()` bodies; keep the signatures.

use std::path::{Path, PathBuf};

/// Final output path for `input` converted to extension `ext` (no dot).
/// Directory: `out_dir` or the input's parent. Stem: input stem + optional `suffix`
/// (e.g. " (compressed)"). Picks the first non-existing name: `a.webp`, `a (1).webp`, ...
/// Never returns `input` itself. Windows reserved device names (CON, NUL, COM1...) get a trailing "_".
pub fn output_path(
    input: &Path,
    ext: &str,
    out_dir: Option<&Path>,
    suffix: Option<&str>,
) -> PathBuf {
    let _ = (input, ext, out_dir, suffix);
    todo!("Task A")
}

/// Temp path for writing `final_path`: same directory, `<stem>.fcpart.<ext>`, so tools
/// that pick a muxer by extension still work.
pub fn temp_path(final_path: &Path) -> PathBuf {
    let _ = final_path;
    todo!("Task A")
}

/// Move a finished temp file into place. If `final_path` appeared meanwhile, use the next
/// free " (n)" name. Returns the path actually written.
pub fn commit(temp: &Path, final_path: &Path) -> std::io::Result<PathBuf> {
    let _ = (temp, final_path);
    todo!("Task A")
}

//! Output file naming. Sources are never overwritten; collisions get " (1)", " (2)", ...
//! Work is written to a temp path next to the destination, then renamed into place.

use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io;
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
    let dir = dir_for(input, out_dir);
    let stem = stem_with_suffix(input, suffix);
    let ext = ext.trim_start_matches('.');
    let mut n = 0u32;
    loop {
        let candidate = dir.join(file_name(&stem, n, ext));
        if candidate != input && !taken(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

/// Temp path for writing `final_path`: same directory, `<stem>.fcpart.<ext>`, so tools
/// that pick a muxer by extension still work.
pub fn temp_path(final_path: &Path) -> PathBuf {
    let mut name = final_path
        .file_stem()
        .map(OsString::from)
        .unwrap_or_default();
    name.push(".fcpart");
    if let Some(ext) = final_path.extension() {
        name.push(".");
        name.push(ext);
    }
    final_path.with_file_name(name)
}

/// Move a finished temp file into place. If `final_path` appeared meanwhile, use the next
/// free " (n)" name. Returns the path actually written.
pub fn commit(temp: &Path, final_path: &Path) -> io::Result<PathBuf> {
    let stem = final_path
        .file_stem()
        .map(OsString::from)
        .unwrap_or_default();
    let ext = final_path
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut n = 0u32;
    loop {
        let candidate = if n == 0 {
            final_path.to_path_buf()
        } else {
            final_path.with_file_name(file_name(&stem, n, &ext))
        };
        n += 1;
        if taken(&candidate) {
            continue;
        }
        if place(temp, &candidate)? {
            return Ok(candidate);
        }
    }
}

/// Create a new, unique directory `<parent>/<name>` (`<name> (1)`, ... when taken) and
/// return its path. Reserved device names get a trailing "_".
pub fn unique_dir(parent: &Path, name: &str) -> io::Result<PathBuf> {
    let base = OsString::from(if is_reserved(name) {
        format!("{name}_")
    } else {
        name.to_string()
    });
    let mut n = 0u32;
    loop {
        let mut dir_name = base.clone();
        if n > 0 {
            dir_name.push(format!(" ({n})"));
        }
        let candidate = parent.join(dir_name);
        match fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => n += 1,
            Err(e) => return Err(e),
        }
    }
}

/// Directory outputs go to: `out_dir`, else the input's parent (empty parent = current dir).
pub fn dir_for(input: &Path, out_dir: Option<&Path>) -> PathBuf {
    match out_dir {
        Some(d) => d.to_path_buf(),
        None => input.parent().map(Path::to_path_buf).unwrap_or_default(),
    }
}

fn taken(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok()
}

fn stem_with_suffix(input: &Path, suffix: Option<&str>) -> OsString {
    let mut stem = input
        .file_stem()
        .map(OsString::from)
        .unwrap_or_else(|| OsString::from("output"));
    if let Some(s) = suffix {
        stem.push(s);
    }
    if is_reserved(&stem.to_string_lossy()) {
        stem.push("_");
    }
    stem
}

/// Windows reserves these names regardless of extension (the part before the first dot).
fn is_reserved(stem: &str) -> bool {
    let base = stem.split('.').next().unwrap_or(stem).trim_end();
    let up = base.to_ascii_uppercase();
    matches!(up.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((up.starts_with("COM") || up.starts_with("LPT"))
            && up.len() == 4
            && matches!(up.as_bytes()[3], b'1'..=b'9'))
}

fn file_name(stem: &OsString, n: u32, ext: &str) -> OsString {
    let mut name = stem.clone();
    if n > 0 {
        name.push(format!(" ({n})"));
    }
    if !ext.is_empty() {
        name.push(".");
        name.push(ext);
    }
    name
}

/// Try to put `temp` at `target` without overwriting anything.
/// `Ok(false)` = the name was taken meanwhile (try the next one).
fn place(temp: &Path, target: &Path) -> io::Result<bool> {
    // A hard link fails atomically if `target` exists, unlike rename (which replaces on Windows).
    match fs::hard_link(temp, target) {
        Ok(()) => {
            let _ = fs::remove_file(temp);
            return Ok(true);
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return Ok(false),
        Err(_) => {}
    }
    // Hard links unsupported (other volume, FAT, ...): claim the name, then rename or copy over.
    match OpenOptions::new().write(true).create_new(true).open(target) {
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return Ok(false),
        Err(e) => return Err(e),
    }
    if fs::rename(temp, target).is_ok() {
        return Ok(true);
    }
    match fs::copy(temp, target) {
        Ok(_) => {
            let _ = fs::remove_file(temp);
            Ok(true)
        }
        Err(e) => {
            let _ = fs::remove_file(target);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_names() {
        for n in ["con", "NUL", "Com1", "lpt9", "aux", "PRN", "con.tar"] {
            assert!(is_reserved(n), "{n}");
        }
        for n in ["com0", "COM10", "console", "a", "CON (compressed)"] {
            assert!(!is_reserved(n), "{n}");
        }
    }

    #[test]
    fn temp_keeps_extension() {
        assert_eq!(
            temp_path(Path::new("C:/x/a.webp")),
            PathBuf::from("C:/x/a.fcpart.webp")
        );
        assert_eq!(temp_path(Path::new("a")), PathBuf::from("a.fcpart"));
    }
}

//! File operations: copy or move into a folder, pack files into a ZIP, extract a ZIP.
//! Nothing existing is ever overwritten.
//!
//! - Copy: the executor names the target (`jobs::naming`, " (1)" on collisions) and writes
//!   it through a temp file with [`copy_file`]. A folder is copied recursively into a new,
//!   uniquely named folder ([`copy_dir_into`]).
//! - Move: [`move_into`] claims a free name and renames onto it; across volumes it copies, then
//!   deletes the source (a failed copy is removed and the source kept). Moving into the folder
//!   the item is already in is a no-op. Folders move the same way.
//! - ZIP: deflate, entry names are the file names, repeated names get " (1)" (compared
//!   case-insensitively, as Windows would on extract). A folder is added recursively under its
//!   own name (`photos/`, `photos/sub/b.jpg`), empty folders included.
//! - Folder walks skip symlinked folders (no loops) and read symlinked files through.
//! - Extract: every entry path is checked before anything is written; `..`, absolute paths,
//!   drive letters and reserved device names reject the whole archive (zip-slip).

use crate::jobs::naming;
use crate::{CancelToken, Error, Result};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;

const CHUNK: usize = 1 << 20;

fn zip_error(e: zip::result::ZipError) -> Error {
    match e {
        zip::result::ZipError::Io(io) => Error::Io(io),
        other => Error::Failed(format!("zip: {other}")),
    }
}

/// Copy with a cancel check per chunk; `on_bytes` gets each chunk's size.
fn copy_chunks(
    src: &mut impl Read,
    dst: &mut impl Write,
    cancel: &CancelToken,
    mut on_bytes: impl FnMut(u64),
) -> Result<()> {
    let mut buf = vec![0u8; CHUNK];
    loop {
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let n = match src.read(&mut buf) {
            Ok(0) => return Ok(()),
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        };
        dst.write_all(&buf[..n])?;
        on_bytes(n as u64);
    }
}

/// Byte copy of `input` to `output` (keeps the modification time, like Explorer).
pub fn copy_file(input: &Path, output: &Path) -> Result<()> {
    fs::copy(input, output)?;
    Ok(())
}

/// Whether `file` already lives directly in `dir`.
fn in_dir(file: &Path, dir: &Path) -> bool {
    let Some(parent) = file.parent() else {
        return false;
    };
    match (fs::canonicalize(parent), fs::canonicalize(dir)) {
        (Ok(a), Ok(b)) => a == b,
        _ => parent == dir,
    }
}

fn ext_of(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// `(path, path relative to the walk root, is folder)` for everything under `dir`, depth first
/// in name order; `rel` is the relative path of `dir` itself. `skip` (e.g. the archive being
/// written) is left out.
fn walk(
    dir: &Path,
    rel: &Path,
    skip: &Path,
    out: &mut Vec<(PathBuf, PathBuf, bool)>,
) -> Result<()> {
    let mut entries = fs::read_dir(dir)?.collect::<io::Result<Vec<_>>>()?;
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let path = e.path();
        if path == skip {
            continue;
        }
        let child = rel.join(e.file_name());
        let kind = e.file_type()?;
        if kind.is_dir() {
            out.push((path.clone(), child.clone(), true));
            walk(&path, &child, skip, out)?;
        } else if kind.is_file() || (kind.is_symlink() && path.is_file()) {
            out.push((path, child, false));
        }
    }
    Ok(())
}

/// The folder's own name (`None` for a drive root).
fn dir_name(dir: &Path) -> Result<String> {
    dir.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| Error::Unsupported(format!("{} has no folder name", dir.display())))
}

/// Refuse to copy or move `src` into itself or one of its subfolders.
fn reject_into_itself(src: &Path, dir: &Path) -> Result<()> {
    let inside = match (fs::canonicalize(src), fs::canonicalize(dir)) {
        (Ok(s), Ok(d)) => d.starts_with(s),
        _ => dir.starts_with(src),
    };
    if inside {
        return Err(Error::Invalid("a folder cannot go into itself".to_string()));
    }
    Ok(())
}

/// Copy the folder `src` into `dir` (created when missing) as a new folder with the same name,
/// " (1)" when taken. Returns the new folder; a failed copy is removed again.
pub fn copy_dir_into(src: &Path, dir: &Path, cancel: &CancelToken) -> Result<PathBuf> {
    if cancel.is_cancelled() {
        return Err(Error::Cancelled);
    }
    let name = dir_name(src)?;
    fs::create_dir_all(dir)?;
    reject_into_itself(src, dir)?;
    let mut items = Vec::new();
    walk(src, Path::new(""), Path::new(""), &mut items)?;
    let target = naming::unique_dir(dir, &name)?;
    let copied = items.iter().try_for_each(|(path, rel, is_dir)| {
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let to = target.join(rel);
        if *is_dir {
            fs::create_dir_all(&to)?;
        } else {
            copy_file(path, &to)?;
        }
        Ok(())
    });
    match copied {
        Ok(()) => Ok(target),
        Err(e) => {
            let _ = fs::remove_dir_all(&target);
            Err(e)
        }
    }
}

fn crosses_volumes(e: &io::Error) -> bool {
    // ERROR_NOT_SAME_DEVICE
    e.kind() == io::ErrorKind::CrossesDevices || e.raw_os_error() == Some(17)
}

/// Move a folder (see [`move_into`]). Across volumes: copy, then delete the source; if the
/// delete fails part-way the complete copy is kept (it may hold the only copy of some files)
/// and the error reported.
fn move_dir_into(src: &Path, dir: &Path, cancel: &CancelToken) -> Result<PathBuf> {
    let name = dir_name(src)?;
    reject_into_itself(src, dir)?;
    // Claim a free name, then free it for the rename (a folder cannot be renamed onto one).
    let target = naming::unique_dir(dir, &name)?;
    fs::remove_dir(&target)?;
    match fs::rename(src, &target) {
        Ok(()) => Ok(target),
        Err(e) if crosses_volumes(&e) => {
            let copy = copy_dir_into(src, dir, cancel)?;
            fs::remove_dir_all(src).map_err(|e| {
                Error::Failed(format!(
                    "copied to {} but could not remove the original: {e}",
                    copy.display()
                ))
            })?;
            Ok(copy)
        }
        Err(e) => Err(e.into()),
    }
}

/// Move `input` (a file or a folder) into `dir` (created when missing) and return where it
/// ended up. Same folder -> `input` unchanged, no error.
pub fn move_into(input: &Path, dir: &Path, cancel: &CancelToken) -> Result<PathBuf> {
    if cancel.is_cancelled() {
        return Err(Error::Cancelled);
    }
    fs::create_dir_all(dir)?;
    if in_dir(input, dir) {
        return Ok(input.to_path_buf());
    }
    if input.is_dir() {
        return move_dir_into(input, dir, cancel);
    }
    let ext = ext_of(input);
    // Claim a free name first: rename would replace an existing file.
    let target = loop {
        let candidate = naming::output_path(input, &ext, Some(dir), None);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(_) => break candidate,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    };
    if fs::rename(input, &target).is_ok() {
        return Ok(target);
    }
    // Another volume: copy, then remove the source; undo the copy if either step fails.
    let moved = copy_file(input, &target).and_then(|()| {
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        fs::remove_file(input).map_err(Error::from)
    });
    match moved {
        Ok(()) => Ok(target),
        Err(e) => {
            let _ = fs::remove_file(&target);
            Err(e)
        }
    }
}

/// `name`, or `stem (n).ext` (folders: `name (n)`), unique among `taken` (lowercased).
fn unique_entry(path: &Path, folder: bool, taken: &mut HashSet<String>) -> String {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".into());
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 && !folder => name.split_at(i),
        _ => (name.as_str(), ""),
    };
    let mut n = 0u32;
    loop {
        let candidate = if n == 0 {
            name.clone()
        } else {
            format!("{stem} ({n}){ext}")
        };
        if taken.insert(candidate.to_lowercase()) {
            return candidate;
        }
        n += 1;
    }
}

/// ZIP entry name for a relative path (`/` separators, folders end with `/`).
fn entry_name(rel: &Path, folder: bool) -> String {
    let mut name = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    if folder {
        name.push('/');
    }
    name
}

/// Pack `inputs` (files and folders) into a new archive at `output`.
pub fn zip(
    inputs: &[PathBuf],
    output: &Path,
    cancel: &CancelToken,
    progress: &dyn Fn(f32),
) -> Result<()> {
    // (source, entry name, is folder, size)
    let mut plan: Vec<(PathBuf, String, bool, u64)> = Vec::new();
    let mut taken = HashSet::new();
    for input in inputs {
        if input.is_dir() {
            let top = PathBuf::from(unique_entry(input, true, &mut taken));
            let mut items = vec![(input.clone(), top.clone(), true)];
            walk(input, &top, output, &mut items)?;
            for (path, rel, folder) in items {
                let size = if folder {
                    0
                } else {
                    fs::metadata(&path)?.len()
                };
                plan.push((path, entry_name(&rel, folder), folder, size));
            }
        } else {
            let size = fs::metadata(input)?.len();
            plan.push((
                input.clone(),
                unique_entry(input, false, &mut taken),
                false,
                size,
            ));
        }
    }
    let total = plan.iter().map(|p| p.3).sum::<u64>().max(1);
    let mut writer = zip::ZipWriter::new(BufWriter::new(File::create(output)?));
    let mut done = 0u64;
    for (input, name, folder, size) in plan {
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .large_file(size >= u64::from(u32::MAX));
        if folder {
            writer.add_directory(name, options).map_err(zip_error)?;
            continue;
        }
        writer.start_file(name, options).map_err(zip_error)?;
        let mut src = BufReader::new(File::open(&input)?);
        copy_chunks(&mut src, &mut writer, cancel, |n| {
            done += n;
            progress(done as f32 / total as f32);
        })?;
    }
    writer.finish().map_err(zip_error)?.flush()?;
    Ok(())
}

/// Relative path for an entry name, `None` when it could escape the target folder or name a
/// Windows device (`CON`, `nul.txt`, `COM1`...). An empty result means "nothing to write"
/// (e.g. `./`).
fn safe_path(name: &str) -> Option<PathBuf> {
    let name = name.replace('\\', "/");
    if name.starts_with('/') {
        return None;
    }
    let mut out = PathBuf::new();
    for part in name.split('/') {
        match part {
            "" | "." => {}
            ".." => return None,
            p if p.contains(':') || naming::is_reserved(p) => return None,
            p => out.push(p),
        }
    }
    Some(out)
}

/// Extract `archive` into the existing folder `dir`; returns the files written.
pub fn extract(
    archive: &Path,
    dir: &Path,
    cancel: &CancelToken,
    progress: &dyn Fn(f32),
) -> Result<Vec<PathBuf>> {
    let mut z = zip::ZipArchive::new(BufReader::new(File::open(archive)?)).map_err(zip_error)?;
    let count = z.len();
    let mut plan = Vec::with_capacity(count);
    for i in 0..count {
        let entry = z.by_index_raw(i).map_err(zip_error)?;
        let name = entry.name().map_err(zip_error)?.into_owned();
        let rel = safe_path(&name)
            .filter(|_| entry.enclosed_name().is_some())
            .ok_or_else(|| Error::Invalid(format!("unsafe path in archive: {name}")))?;
        plan.push((rel, entry.is_dir()));
    }
    let mut written = Vec::new();
    for (i, (rel, is_dir)) in plan.into_iter().enumerate() {
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        if rel.as_os_str().is_empty() {
            continue;
        }
        let target = dir.join(&rel);
        if is_dir {
            fs::create_dir_all(&target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut entry = z.by_index(i).map_err(zip_error)?;
        let mut out = BufWriter::new(File::create(&target)?);
        copy_chunks(&mut entry, &mut out, cancel, |_| {})?;
        out.flush()?;
        written.push(target);
        progress((i + 1) as f32 / count as f32);
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_names_are_unique_ignoring_case() {
        let mut taken = HashSet::new();
        let names: Vec<String> = ["x/a.txt", "y/A.txt", "z/a.txt", "q/.env", "r/.env", "noext"]
            .iter()
            .map(|p| unique_entry(Path::new(p), false, &mut taken))
            .collect();
        assert_eq!(
            names,
            [
                "a.txt",
                "A (1).txt",
                "a (2).txt",
                ".env",
                ".env (1)",
                "noext"
            ]
        );
        // Folders keep their whole name: "my.album (1)", not "my (1).album".
        assert_eq!(
            unique_entry(Path::new("x/my.album"), true, &mut taken),
            "my.album"
        );
        assert_eq!(
            unique_entry(Path::new("y/my.album"), true, &mut taken),
            "my.album (1)"
        );
        assert_eq!(
            entry_name(&Path::new("photos").join("sub").join("b.jpg"), false),
            "photos/sub/b.jpg"
        );
        assert_eq!(entry_name(Path::new("photos"), true), "photos/");
    }

    #[test]
    fn unsafe_entry_paths() {
        for bad in [
            "../x",
            "a/../../x",
            "/etc/x",
            "\\x",
            "C:/x",
            "a/b:c",
            "CON",
            "dir/nul.txt",
            "com1.log",
        ] {
            assert!(safe_path(bad).is_none(), "{bad}");
        }
        assert_eq!(safe_path("a/./b\\c.txt"), Some(PathBuf::from("a/b/c.txt")));
        assert_eq!(safe_path("./"), Some(PathBuf::new()));
        assert_eq!(safe_path("console.txt"), Some(PathBuf::from("console.txt")));
        assert_eq!(safe_path("com0"), Some(PathBuf::from("com0")));
    }
}

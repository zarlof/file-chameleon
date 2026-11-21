//! The files rope's store: `files.json` in the app local data folder. Newest first, at most
//! [`MAX_FILES`] entries, deduplicated by path (case-insensitive on Windows). Files that no
//! longer exist are pruned when listed.

use crate::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const MAX_FILES: usize = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileSource {
    Added,
    Converted,
    Downloaded,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: PathBuf,
    pub source: FileSource,
    /// Milliseconds since the epoch.
    pub at: u64,
}

pub struct FileStore {
    file: PathBuf,
    entries: Vec<FileEntry>,
}

/// Dedupe key: paths compare case-insensitively (Windows file names are).
fn key(p: &Path) -> String {
    p.to_string_lossy().replace('/', "\\").to_lowercase()
}

/// Does a failed `metadata` call prove the file is gone? Only `NotFound` does: a network or
/// offline drive, a permission problem or a timeout leaves the entry alone.
pub fn error_means_gone(kind: std::io::ErrorKind) -> bool {
    kind == std::io::ErrorKind::NotFound
}

/// Is `p` definitively missing? UNC paths are never stat-ed (an unreachable server would hang
/// while the store lock is held), and a path on a drive whose root does not answer is kept.
fn definitely_missing(p: &Path, roots: &mut std::collections::HashMap<String, bool>) -> bool {
    use std::path::Component;
    let mut comps = p.components();
    if let Some(Component::Prefix(prefix)) = comps.next() {
        match prefix.kind() {
            std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_) => {
                // Each drive root is probed once per call, not once per entry.
                let root: PathBuf = p.components().take(2).collect();
                let up = *roots
                    .entry(key(&root))
                    .or_insert_with(|| std::fs::metadata(&root).is_ok());
                if !up {
                    return false;
                }
            }
            _ => return false,
        }
    }
    match std::fs::metadata(p) {
        Ok(_) => false,
        Err(e) => error_means_gone(e.kind()),
    }
}

impl FileStore {
    /// Load `files.json` from `dir` (missing or corrupt file = empty store).
    pub fn open(dir: &Path) -> Result<FileStore> {
        let file = dir.join("files.json");
        let entries = std::fs::read(&file)
            .ok()
            .and_then(|b| serde_json::from_slice::<Vec<FileEntry>>(&b).ok())
            .unwrap_or_default();
        Ok(FileStore { file, entries })
    }

    fn save(&self) -> Result<()> {
        if let Some(dir) = self.file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.file.with_extension("json.tmp");
        std::fs::write(
            &tmp,
            serde_json::to_vec_pretty(&self.entries).map_err(std::io::Error::other)?,
        )?;
        std::fs::rename(&tmp, &self.file)?;
        Ok(())
    }

    /// Put `paths` at the front (in the given order), replacing earlier entries of the same
    /// path; keeps at most [`MAX_FILES`].
    pub fn record(&mut self, paths: &[PathBuf], source: FileSource, now: u64) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }
        let keys: Vec<String> = paths.iter().map(|p| key(p)).collect();
        self.entries.retain(|e| !keys.contains(&key(&e.path)));
        let mut seen = Vec::new();
        let mut fresh = Vec::new();
        for (p, k) in paths.iter().zip(keys) {
            if !seen.contains(&k) {
                seen.push(k);
                fresh.push(FileEntry {
                    path: p.clone(),
                    source,
                    at: now,
                });
            }
        }
        self.entries.splice(0..0, fresh);
        self.entries.truncate(MAX_FILES);
        self.save()
    }

    /// Entries of the given sources, newest first. Entries whose file is gone are dropped
    /// (and the store is saved when that happened).
    pub fn list(&mut self, sources: &[FileSource]) -> Vec<FileEntry> {
        let before = self.entries.len();
        let mut roots = std::collections::HashMap::new();
        self.entries
            .retain(|e| !definitely_missing(&e.path, &mut roots));
        if self.entries.len() != before {
            let _ = self.save();
        }
        self.entries
            .iter()
            .filter(|e| sources.contains(&e.source))
            .cloned()
            .collect()
    }

    pub fn remove(&mut self, paths: &[PathBuf]) -> Result<()> {
        let keys: Vec<String> = paths.iter().map(|p| key(p)).collect();
        let before = self.entries.len();
        self.entries.retain(|e| !keys.contains(&key(&e.path)));
        if self.entries.len() != before {
            self.save()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(dir: &Path, name: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, b"x").unwrap();
        p
    }

    const ALL: [FileSource; 3] = [
        FileSource::Added,
        FileSource::Converted,
        FileSource::Downloaded,
    ];

    #[test]
    fn files_store_dedupe_and_cap() {
        let d = tempfile::tempdir().unwrap();
        let a = touch(d.path(), "a.png");
        let mut s = FileStore::open(d.path()).unwrap();
        s.record(std::slice::from_ref(&a), FileSource::Added, 1)
            .unwrap();
        s.record(std::slice::from_ref(&a), FileSource::Converted, 5)
            .unwrap();
        let l = s.list(&ALL);
        assert_eq!(l.len(), 1);
        assert_eq!((l[0].at, l[0].source), (5, FileSource::Converted));
        // Many files: newest first, capped at 300.
        let many: Vec<PathBuf> = (0..310)
            .map(|i| touch(d.path(), &format!("f{i}.txt")))
            .collect();
        for (i, p) in many.iter().enumerate() {
            s.record(std::slice::from_ref(p), FileSource::Added, 100 + i as u64)
                .unwrap();
        }
        let l = s.list(&ALL);
        assert_eq!(l.len(), 300);
        assert_eq!(l[0].path, many[309]);
        // It persists.
        let mut s2 = FileStore::open(d.path()).unwrap();
        assert_eq!(s2.list(&ALL).len(), 300);
    }

    #[test]
    #[cfg(windows)]
    fn paths_differing_in_case_are_one_entry() {
        let d = tempfile::tempdir().unwrap();
        let mut s = FileStore::open(d.path()).unwrap();
        s.entries.push(FileEntry {
            path: PathBuf::from(r"C:\A\x.png"),
            source: FileSource::Added,
            at: 1,
        });
        s.record(&[PathBuf::from(r"c:\a\X.PNG")], FileSource::Added, 2)
            .unwrap();
        assert_eq!(s.entries.len(), 1);
        assert_eq!(s.entries[0].at, 2);
        s.record(&[PathBuf::from("C:/a/x.png")], FileSource::Added, 3)
            .unwrap();
        assert_eq!(s.entries.len(), 1);
    }

    #[test]
    fn prunes_missing() {
        let d = tempfile::tempdir().unwrap();
        let a = touch(d.path(), "a.png");
        let b = touch(d.path(), "b.png");
        let mut s = FileStore::open(d.path()).unwrap();
        s.record(&[a.clone(), b.clone()], FileSource::Added, 1)
            .unwrap();
        std::fs::remove_file(&a).unwrap();
        let l = s.list(&ALL);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].path, b);
        let mut again = FileStore::open(d.path()).unwrap();
        assert_eq!(again.entries.len(), 1, "pruning was saved");
        assert_eq!(again.list(&ALL).len(), 1);
    }

    #[test]
    fn only_not_found_prunes() {
        use std::io::ErrorKind::*;
        assert!(error_means_gone(NotFound));
        for k in [
            PermissionDenied,
            TimedOut,
            ConnectionRefused,
            Other,
            Interrupted,
        ] {
            assert!(!error_means_gone(k), "{k:?}");
        }
        let mut roots = std::collections::HashMap::new();
        // UNC paths and paths on a dead drive are kept, existing ones too.
        #[cfg(windows)]
        assert!(!definitely_missing(
            Path::new(r"\no-such-server\share.png"),
            &mut roots
        ));
        let d = tempfile::tempdir().unwrap();
        assert!(!definitely_missing(&touch(d.path(), "k.png"), &mut roots));
        assert!(definitely_missing(&d.path().join("nope.png"), &mut roots));
        assert_eq!(
            roots.len(),
            if cfg!(windows) { 1 } else { 0 },
            "one probe per root"
        );
    }

    #[test]
    fn files_filter_by_source() {
        let d = tempfile::tempdir().unwrap();
        let a = touch(d.path(), "a.png");
        let b = touch(d.path(), "b.mp3");
        let c = touch(d.path(), "c.mp4");
        let mut s = FileStore::open(d.path()).unwrap();
        s.record(std::slice::from_ref(&a), FileSource::Added, 1)
            .unwrap();
        s.record(std::slice::from_ref(&b), FileSource::Converted, 2)
            .unwrap();
        s.record(std::slice::from_ref(&c), FileSource::Downloaded, 3)
            .unwrap();
        let l = s.list(&[FileSource::Converted, FileSource::Downloaded]);
        assert_eq!(l.iter().map(|e| e.path.clone()).collect::<Vec<_>>(), [c, b]);
        assert!(s.list(&[]).is_empty());
        s.remove(&[a]).unwrap();
        assert_eq!(s.list(&ALL).len(), 2);
    }
}

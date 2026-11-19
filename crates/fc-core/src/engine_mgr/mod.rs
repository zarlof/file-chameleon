//! On-demand external tools. OWNER: Task C.
//!
//! Layout: `<root>/<id>/` holds the extracted files plus `installed.json`
//! (`{ "version": "...", "sha256": "..." }`). Install = download to `<root>/.staging/<id>.download`
//! (resumable not required), verify SHA-256, extract only the manifest `files` (archive entries are
//! matched by path *suffix*, so a top-level version folder in the zip is fine), then atomically
//! rename the staging folder to `<root>/<id>`. Any failure removes staging. Max download 400 MB.
//! Resolution (`locate`): managed install first, then the system PATH (`system` names),
//! then nothing. yt-dlp: verify against the line for `yt-dlp.exe` in `sha256Url`
//! (SHA2-256SUMS format: `<hex>  <name>`), and enforce `minVersion` on system copies
//! (`yt-dlp --version` prints `YYYY.MM.DD`).

pub mod manifest;

use crate::engines::ToolLocator;
use crate::{CancelToken, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

pub use manifest::{Manifest, ToolEntry};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolSource {
    Managed,
    System,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatus {
    pub id: String,
    pub name: String,
    pub purpose: String,
    pub license: String,
    pub source: ToolSource,
    /// Directory or file in use, when not missing.
    pub path: Option<PathBuf>,
    pub version: Option<String>,
    /// Download size in bytes (from the manifest).
    pub size: u64,
}

/// Progress callback: (bytes_done, bytes_total).
pub type InstallProgress<'a> = &'a dyn Fn(u64, u64);

pub struct EngineManager {
    _private: (),
}

impl EngineManager {
    /// `root` = `%LOCALAPPDATA%\FileChameleon\engines`. Creates it if missing.
    pub fn new(root: PathBuf, manifest: Manifest) -> Self {
        let _ = (root, manifest);
        todo!("Task C")
    }

    pub fn manifest(&self) -> &Manifest {
        todo!("Task C")
    }

    pub fn status(&self) -> Vec<ToolStatus> {
        todo!("Task C")
    }

    pub fn install(
        &self,
        id: &str,
        progress: InstallProgress<'_>,
        cancel: &CancelToken,
    ) -> Result<()> {
        let _ = (id, progress, cancel);
        todo!("Task C")
    }

    /// Removes only a managed install; system copies are never touched.
    pub fn uninstall(&self, id: &str) -> Result<()> {
        let _ = id;
        todo!("Task C")
    }

    /// Managed yt-dlp only: re-download latest if the installed one is older than 24 h
    /// since the last check (state kept in `installed.json` as `checkedAt`). Returns true if updated.
    pub fn update_ytdlp_if_stale(&self, cancel: &CancelToken) -> Result<bool> {
        let _ = cancel;
        todo!("Task C")
    }
}

impl ToolLocator for EngineManager {
    fn locate(&self, tool: &str, file: &str) -> Option<PathBuf> {
        let _ = (tool, file);
        todo!("Task C")
    }
}

/// Search `PATH` for an executable file name. Exposed for tests.
pub fn which(file: &str) -> Option<PathBuf> {
    let _ = file;
    todo!("Task C")
}

/// Lowercase hex SHA-256 of a file.
pub fn sha256_file(path: &Path) -> Result<String> {
    let _ = path;
    todo!("Task C")
}

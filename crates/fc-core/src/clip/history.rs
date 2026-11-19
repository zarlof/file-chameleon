//! Clipboard history store. OWNER: Task G.
//! Layout: `<dir>/history.json` (items, newest first) + `<dir>/img/<id>.png` (+ `<id>.thumb.png`,
//! longest side 256). Writes are atomic (temp file + rename). Text items over 1 MB are not stored.
//! Dedupe: adding content whose hash equals an existing item moves that item to the top
//! (keeps its id and pin) instead of adding a duplicate.

use super::ClipKind;
use crate::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const MAX_TEXT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipItem {
    pub id: u64,
    pub kind: ClipKind,
    /// Text or URL for text kinds.
    pub text: Option<String>,
    /// Absolute paths, image kind only.
    pub image: Option<PathBuf>,
    pub thumb: Option<PathBuf>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Site name for media links.
    pub site: Option<String>,
    /// Hex of a 64-bit content hash (FNV-1a over text bytes or PNG bytes).
    pub hash: String,
    pub created_at: u64,
    pub pinned: bool,
}

pub struct ClipStore {
    _private: (),
}

impl ClipStore {
    /// Load or create. A corrupt index is renamed to `history.json.bak` and a fresh store starts.
    pub fn open(dir: &Path) -> Result<Self> {
        let _ = dir;
        todo!("Task G")
    }
    /// Newest first.
    pub fn items(&self) -> &[ClipItem] {
        todo!("Task G")
    }
    /// Returns the added (or moved-to-top) item, `None` if skipped (empty/oversized).
    pub fn add_text(&mut self, text: &str, now: u64) -> Result<Option<ClipItem>> {
        let _ = (text, now);
        todo!("Task G")
    }
    /// `png` = encoded PNG bytes.
    pub fn add_image_png(
        &mut self,
        png: &[u8],
        width: u32,
        height: u32,
        now: u64,
    ) -> Result<Option<ClipItem>> {
        let _ = (png, width, height, now);
        todo!("Task G")
    }
    /// Case-insensitive substring match on text/site; empty query = all.
    pub fn search(&self, query: &str) -> Vec<ClipItem> {
        let _ = query;
        todo!("Task G")
    }
    pub fn set_pinned(&mut self, id: u64, pinned: bool) -> Result<bool> {
        let _ = (id, pinned);
        todo!("Task G")
    }
    /// Deletes the item and its image files.
    pub fn remove(&mut self, id: u64) -> Result<bool> {
        let _ = id;
        todo!("Task G")
    }
    /// Remove all unpinned items.
    pub fn clear_unpinned(&mut self) -> Result<()> {
        todo!("Task G")
    }
    /// Drop unpinned items older than `retention_days` and beyond `max_items` (pinned never count).
    pub fn prune(&mut self, now: u64, retention_days: u32, max_items: usize) -> Result<()> {
        let _ = (now, retention_days, max_items);
        todo!("Task G")
    }
}

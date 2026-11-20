//! Smart clipboard: classification + persisted history (JSON index + PNG files).
//! Platform capture lives in `src-tauri/src/platform/windows/clipboard.rs`.
//!
//! - [`history`]: the store (items, trash, retention, edits, export)
//! - [`folders`]: folders and tags (second `impl` block of the store)
//! - [`enrich`]: link titles fetched from the web

pub mod enrich;
pub mod folders;
pub mod history;

use serde::{Deserialize, Serialize};

pub use history::{ClipItem, ClipStore};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClipKind {
    Text,
    /// A link that is not a media page.
    Link,
    /// A link `download::classify_url` says is downloadable (MediaPage, DirectImage, DirectMedia).
    MediaLink,
    Image,
}

/// Classify copied text. Uses `download::classify_url`.
pub fn classify_text(text: &str) -> ClipKind {
    match crate::download::classify_url(text) {
        Some(info) => match info.kind {
            crate::download::LinkKind::Other => ClipKind::Link,
            _ => ClipKind::MediaLink,
        },
        None => ClipKind::Text,
    }
}

/// Shelf rows: images = image; texts = text; links = link + mediaLink.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shelf {
    Images,
    Texts,
    Links,
}

impl Shelf {
    pub fn matches(self, kind: ClipKind) -> bool {
        matches!(
            (self, kind),
            (Shelf::Images, ClipKind::Image)
                | (Shelf::Texts, ClipKind::Text)
                | (Shelf::Links, ClipKind::Link | ClipKind::MediaLink)
        )
    }
}

/// Filter for [`ClipStore::query`]. Newest first, live items only unless `trash`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClipQuery {
    pub text: Option<String>,
    pub shelf: Option<Shelf>,
    pub pinned: bool,
    pub folder: Option<u64>,
    pub tag: Option<String>,
    /// true = only trashed items; false = only live items.
    pub trash: bool,
    /// Only saved items (pinned, in a folder, tagged, titled or noted).
    pub saved: bool,
    pub limit: Option<usize>,
}

/// How long unsaved items live. 0 days = forever. `max_items` caps unsaved live items (0 = no cap).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Retention {
    pub text_days: u32,
    pub links_days: u32,
    pub images_days: u32,
    pub max_items: usize,
}

impl Retention {
    pub fn from_settings(s: &crate::settings::Settings) -> Self {
        Self {
            text_days: s.clipboard_retention_text,
            links_days: s.clipboard_retention_links,
            images_days: s.clipboard_retention_images,
            max_items: s.clipboard_max_items as usize,
        }
    }

    pub(crate) fn days_for(&self, kind: ClipKind) -> u32 {
        match kind {
            ClipKind::Text => self.text_days,
            ClipKind::Link | ClipKind::MediaLink => self.links_days,
            ClipKind::Image => self.images_days,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Folder {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub parent: Option<u64>,
}

/// A folder with the number of live items directly in it (TS `ClipFolder`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderInfo {
    pub id: u64,
    pub name: String,
    pub parent: Option<u64>,
    pub count: usize,
}

/// TS `ClipTag`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagInfo {
    pub tag: String,
    pub count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipStats {
    pub items: usize,
    pub trash: usize,
    pub bytes: u64,
}

/// Editable item field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Field {
    Title,
    Note,
    Text,
}

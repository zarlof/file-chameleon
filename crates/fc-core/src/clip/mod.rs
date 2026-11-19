//! Smart clipboard: classification + persisted history (JSON index + PNG files).
//! Platform capture lives in `src-tauri/src/platform/windows/clipboard.rs`. OWNER: Task G.

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

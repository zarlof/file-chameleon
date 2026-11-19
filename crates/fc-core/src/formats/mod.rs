//! Format table. Format ids are lowercase, stable, and cross IPC as plain strings.

pub mod planner;
pub mod registry;

use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Image,
    Vector,
    Video,
    Audio,
    Pdf,
    Data,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatInfo {
    pub id: &'static str,
    pub kind: Kind,
    pub label: &'static str,
    /// Lowercase, without dot. The first one is used for output files.
    pub extensions: &'static [&'static str],
}

macro_rules! f {
    ($id:literal, $kind:ident, $label:literal, [$($ext:literal),+]) => {
        FormatInfo { id: $id, kind: Kind::$kind, label: $label, extensions: &[$($ext),+] }
    };
}

/// Every format File Chameleon understands in v1 (input, output or both).
pub static FORMATS: &[FormatInfo] = &[
    // Raster images
    f!("png", Image, "PNG", ["png"]),
    f!("jpg", Image, "JPG", ["jpg", "jpeg", "jfif", "jpe"]),
    f!("webp", Image, "WebP", ["webp"]),
    f!("gif", Image, "GIF", ["gif"]),
    f!("bmp", Image, "BMP", ["bmp", "dib"]),
    f!("tiff", Image, "TIFF", ["tiff", "tif"]),
    f!("ico", Image, "ICO", ["ico"]),
    // Vector
    f!("svg", Vector, "SVG", ["svg"]),
    // Video
    f!("mp4", Video, "MP4", ["mp4", "m4v"]),
    f!("mkv", Video, "MKV", ["mkv"]),
    f!("webm", Video, "WebM", ["webm"]),
    f!("mov", Video, "MOV", ["mov", "qt"]),
    f!("avi", Video, "AVI", ["avi"]),
    // Audio
    f!("mp3", Audio, "MP3", ["mp3"]),
    f!("m4a", Audio, "M4A", ["m4a", "aac"]),
    f!("opus", Audio, "Opus", ["opus"]),
    f!("ogg", Audio, "OGG", ["ogg", "oga"]),
    f!("flac", Audio, "FLAC", ["flac"]),
    f!("wav", Audio, "WAV", ["wav"]),
    // Documents
    f!("pdf", Pdf, "PDF", ["pdf"]),
    // Text & data
    f!("md", Data, "Markdown", ["md", "markdown"]),
    f!("txt", Data, "TXT", ["txt"]),
    f!("html", Data, "HTML", ["html", "htm"]),
    f!("csv", Data, "CSV", ["csv"]),
    f!("json", Data, "JSON", ["json"]),
    f!("yaml", Data, "YAML", ["yaml", "yml"]),
    f!("toml", Data, "TOML", ["toml"]),
];

pub fn by_id(id: &str) -> Option<&'static FormatInfo> {
    FORMATS.iter().find(|f| f.id == id)
}

/// Extension-based detection, case-insensitive. Returns `None` for unknown files.
pub fn detect_path(path: &Path) -> Option<&'static FormatInfo> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    FORMATS
        .iter()
        .find(|f| f.extensions.contains(&ext.as_str()))
}

/// Primary output extension for a format id.
pub fn output_ext(id: &str) -> Option<&'static str> {
    by_id(id).map(|f| f.extensions[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_extensions_dont_collide() {
        let mut ids = std::collections::HashSet::new();
        let mut exts = std::collections::HashSet::new();
        for f in FORMATS {
            assert!(ids.insert(f.id), "duplicate id {}", f.id);
            for e in f.extensions {
                assert!(exts.insert(*e), "extension {e} claimed twice");
                assert_eq!(*e, e.to_ascii_lowercase());
            }
        }
    }

    #[test]
    fn detects_case_insensitively() {
        assert_eq!(detect_path(Path::new("C:/a/B.JPEG")).unwrap().id, "jpg");
        assert_eq!(detect_path(Path::new("clip.MkV")).unwrap().id, "mkv");
        assert!(detect_path(Path::new("noext")).is_none());
        assert!(detect_path(Path::new("x.unknown")).is_none());
    }
}

//! Format table. Format ids are lowercase, stable, and cross IPC as plain strings.

pub mod planner;
pub mod registry;

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Image,
    Vector,
    Video,
    Audio,
    Pdf,
    Data,
    Subtitle,
    Archive,
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

/// Every format File Chameleon understands (input, output or both).
pub static FORMATS: &[FormatInfo] = &[
    // Raster images
    f!("png", Image, "PNG", ["png"]),
    f!("jpg", Image, "JPG", ["jpg", "jpeg", "jfif", "jpe"]),
    f!("webp", Image, "WebP", ["webp"]),
    f!("gif", Image, "GIF", ["gif"]),
    f!("bmp", Image, "BMP", ["bmp", "dib"]),
    f!("tiff", Image, "TIFF", ["tiff", "tif"]),
    f!("ico", Image, "ICO", ["ico"]),
    f!("tga", Image, "TGA", ["tga"]),
    f!("qoi", Image, "QOI", ["qoi"]),
    f!("pnm", Image, "PNM", ["pnm", "pbm", "pgm", "ppm"]),
    // Vector
    f!("svg", Vector, "SVG", ["svg"]),
    // Video
    f!("mp4", Video, "MP4", ["mp4"]),
    f!("mkv", Video, "MKV", ["mkv"]),
    f!("webm", Video, "WebM", ["webm"]),
    f!("mov", Video, "MOV", ["mov", "qt"]),
    f!("avi", Video, "AVI", ["avi"]),
    f!("m4v", Video, "M4V", ["m4v"]),
    f!("wmv", Video, "WMV", ["wmv"]),
    f!("flv", Video, "FLV", ["flv"]),
    f!("mpg", Video, "MPG", ["mpg", "mpeg", "mpe"]),
    f!("3gp", Video, "3GP", ["3gp", "3gpp"]),
    f!("ts", Video, "TS", ["ts", "m2ts", "mts"]),
    f!("ogv", Video, "OGV", ["ogv"]),
    // Audio
    f!("mp3", Audio, "MP3", ["mp3"]),
    f!("m4a", Audio, "M4A", ["m4a"]),
    f!("opus", Audio, "Opus", ["opus"]),
    f!("ogg", Audio, "OGG", ["ogg", "oga"]),
    f!("flac", Audio, "FLAC", ["flac"]),
    f!("wav", Audio, "WAV", ["wav"]),
    f!("aac", Audio, "AAC", ["aac"]),
    f!("wma", Audio, "WMA", ["wma"]),
    f!("aiff", Audio, "AIFF", ["aiff", "aif", "aifc"]),
    f!("ac3", Audio, "AC3", ["ac3"]),
    f!("amr", Audio, "AMR", ["amr"]),
    f!("mka", Audio, "MKA", ["mka"]),
    // iPhone ringtone: AAC in an MP4 (ipod) container, at most 40 s.
    f!("m4r", Audio, "M4R", ["m4r"]),
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
    f!("tsv", Data, "TSV", ["tsv"]),
    // Subtitles
    f!("srt", Subtitle, "SRT", ["srt"]),
    f!("vtt", Subtitle, "WebVTT", ["vtt"]),
    f!("ass", Subtitle, "ASS", ["ass", "ssa"]),
    // Archives (input only: extract)
    f!("zip", Archive, "ZIP", ["zip"]),
];

/// Formats File Chameleon can read but never writes: the shipped ffmpeg build (gyan.dev
/// essentials) has no Theora encoder worth relying on (`ogv`) and no AMR encoder
/// (`libopencore-amrnb`, `amr`). They never appear as a conversion target.
pub const INPUT_ONLY: &[&str] = &["ogv", "amr"];

/// True when `id` has no encoder in the shipped engines (see [`INPUT_ONLY`]).
pub fn is_input_only(id: &str) -> bool {
    INPUT_ONLY.contains(&id)
}

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

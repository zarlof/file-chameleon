//! The conversion graph: which engine turns format A into format B in one step.
//! This table *is* the format matrix. Add an edge here and implement it in
//! the engine; the planner, the wheel and the Convert view pick it up.

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EngineId {
    /// In-process raster images (`image` + `webp` crates).
    Image,
    /// In-process SVG rasterizer (`resvg`).
    Svg,
    /// External ffmpeg/ffprobe (on demand or from PATH).
    Ffmpeg,
    /// In-process PDF assembly (`lopdf`): images -> pdf, merge, split.
    Pdf,
    /// pdfium.dll (on demand): pdf -> raster.
    Pdfium,
    /// In-process text/data conversions.
    Data,
    /// In-process subtitle conversions (srt <-> vtt, -> txt).
    Subtitle,
}

impl EngineId {
    /// External tool id in `engines.manifest.json` this engine needs, if any.
    pub fn required_tool(self) -> Option<&'static str> {
        match self {
            EngineId::Ffmpeg => Some("ffmpeg"),
            EngineId::Pdfium => Some("pdfium"),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edge {
    pub from: &'static str,
    pub to: &'static str,
    pub engine: EngineId,
    /// Lower is preferred by the planner. In-process = 1, external tool = 3.
    pub cost: u32,
    /// May be the first step of a 2-step route. Only lossless-enough hops are chainable
    /// (svg->png, pdf->png, gif->video), so odd routes like mp4->gif->pdf never appear.
    pub chain: bool,
}

const RASTER_IN: &[&str] = &[
    "png", "jpg", "webp", "gif", "bmp", "tiff", "ico", "tga", "qoi", "pnm",
];
const RASTER_OUT: &[&str] = &[
    "png", "jpg", "webp", "gif", "bmp", "tiff", "ico", "tga", "qoi", "pnm",
];
const SVG_OUT: &[&str] = &["png", "jpg", "webp", "ico"];
/// Video formats ffmpeg can both read and write. `ogv` (Theora) and `amr` below are read-only:
/// the shipped essentials ffmpeg build has no encoder for them (see `formats::INPUT_ONLY`).
const VIDEO_OUT: &[&str] = &[
    "mp4", "mkv", "webm", "mov", "avi", "m4v", "wmv", "flv", "mpg", "3gp", "ts",
];
const VIDEO_IN: &[&str] = &[
    "mp4", "mkv", "webm", "mov", "avi", "m4v", "wmv", "flv", "mpg", "3gp", "ts", "ogv",
];
/// `m4r` is an ordinary audio target (ffmpeg cuts it to 40 s) and input.
const AUDIO_OUT: &[&str] = &[
    "mp3", "m4a", "opus", "ogg", "flac", "wav", "m4r", "aac", "wma", "aiff", "ac3", "mka",
];
const AUDIO_IN: &[&str] = &[
    "mp3", "m4a", "opus", "ogg", "flac", "wav", "m4r", "aac", "wma", "aiff", "ac3", "mka", "amr",
];
const DATA_STRUCT: &[&str] = &["csv", "json", "yaml", "toml", "tsv"];

fn build() -> Vec<Edge> {
    let mut v = Vec::new();
    let mut add = |from: &'static str, to: &'static str, engine: EngineId, cost: u32| {
        if from != to {
            let chain = matches!(
                (from, to),
                ("svg", "png") | ("pdf", "png") | ("gif", "mp4") | ("gif", "webm")
            );
            v.push(Edge {
                from,
                to,
                engine,
                cost,
                chain,
            });
        }
    };
    for &a in RASTER_IN {
        for &b in RASTER_OUT {
            add(a, b, EngineId::Image, 1);
        }
        add(a, "pdf", EngineId::Pdf, 1);
    }
    for &b in SVG_OUT {
        add("svg", b, EngineId::Svg, 1);
    }
    for &a in VIDEO_IN {
        for &b in VIDEO_OUT {
            add(a, b, EngineId::Ffmpeg, 3);
        }
        add(a, "gif", EngineId::Ffmpeg, 3);
        for &b in AUDIO_OUT {
            add(a, b, EngineId::Ffmpeg, 3);
        }
    }
    // Animated GIF -> video (ffmpeg keeps all frames).
    add("gif", "mp4", EngineId::Ffmpeg, 3);
    add("gif", "webm", EngineId::Ffmpeg, 3);
    for &a in AUDIO_IN {
        for &b in AUDIO_OUT {
            add(a, b, EngineId::Ffmpeg, 3);
        }
    }
    add("pdf", "png", EngineId::Pdfium, 3);
    add("pdf", "jpg", EngineId::Pdfium, 3);
    add("md", "html", EngineId::Data, 1);
    add("txt", "html", EngineId::Data, 1);
    for &a in DATA_STRUCT {
        for &b in DATA_STRUCT {
            add(a, b, EngineId::Data, 1);
        }
    }
    add("srt", "vtt", EngineId::Subtitle, 1);
    add("vtt", "srt", EngineId::Subtitle, 1);
    add("srt", "txt", EngineId::Subtitle, 1);
    add("vtt", "txt", EngineId::Subtitle, 1);
    add("ass", "srt", EngineId::Ffmpeg, 3);
    add("ass", "vtt", EngineId::Ffmpeg, 3);
    add("ass", "txt", EngineId::Ffmpeg, 3);
    v
}

/// All one-step edges. Built once.
pub fn edges() -> &'static [Edge] {
    static EDGES: OnceLock<Vec<Edge>> = OnceLock::new();
    EDGES.get_or_init(build)
}

/// Direct edges leaving `from`.
pub fn edges_from(from: &str) -> impl Iterator<Item = &'static Edge> + '_ {
    edges().iter().filter(move |e| e.from == from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::by_id;

    const EDGE_COUNT: usize = 558;

    #[test]
    fn every_edge_uses_known_formats() {
        for e in edges() {
            assert!(by_id(e.from).is_some(), "unknown from {}", e.from);
            assert!(by_id(e.to).is_some(), "unknown to {}", e.to);
            assert_ne!(e.from, e.to);
        }
    }

    #[test]
    fn new_formats_edges() {
        use crate::formats::{Kind, by_id};
        let has = |from: &str, to: &str| edges().iter().any(|e| e.from == from && e.to == to);
        for (a, b) in [
            ("mp4", "wmv"),
            ("flv", "mp3"),
            ("aac", "flac"),
            ("tga", "png"),
            ("png", "qoi"),
            ("ass", "txt"),
        ] {
            assert!(has(a, b), "{a} -> {b}");
        }
        assert_eq!(by_id("wmv").unwrap().kind, Kind::Video);
        assert_eq!(by_id("aiff").unwrap().kind, Kind::Audio);
        assert_eq!(by_id("tga").unwrap().kind, Kind::Image);
        assert_eq!(edges().len(), EDGE_COUNT);
    }

    #[test]
    fn input_only_formats_not_targets() {
        for &x in crate::formats::INPUT_ONLY {
            assert!(crate::formats::is_input_only(x));
            assert!(crate::formats::by_id(x).is_some(), "{x} must be a format");
            assert!(
                edges_from(x).next().is_some(),
                "{x} must convert to something"
            );
            assert!(!edges().iter().any(|e| e.to == x), "{x} is input only");
        }
        assert!(!crate::formats::is_input_only("mp4"));
    }

    #[test]
    fn v2_edges() {
        let has = |from: &str, to: &str, engine: EngineId| {
            edges()
                .iter()
                .any(|e| e.from == from && e.to == to && e.engine == engine)
        };
        assert!(has("wav", "m4r", EngineId::Ffmpeg) && has("mp4", "m4r", EngineId::Ffmpeg));
        assert!(has("tsv", "csv", EngineId::Data) && has("toml", "tsv", EngineId::Data));
        assert!(has("srt", "vtt", EngineId::Subtitle) && has("vtt", "txt", EngineId::Subtitle));
        assert!(has("ass", "srt", EngineId::Ffmpeg) && has("ass", "vtt", EngineId::Ffmpeg));
        assert!(edges_from("zip").next().is_none(), "zip is input only");
        assert!(!edges().iter().any(|e| e.to == "zip" || e.to == "ass"));
    }
}

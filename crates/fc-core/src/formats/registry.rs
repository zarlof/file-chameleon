//! The conversion graph: which engine turns format A into format B in one step.
//! This table *is* the v1 format matrix. Add an edge here and implement it in
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

const RASTER_IN: &[&str] = &["png", "jpg", "webp", "gif", "bmp", "tiff", "ico"];
const RASTER_OUT: &[&str] = &["png", "jpg", "webp", "gif", "bmp", "tiff", "ico"];
const SVG_OUT: &[&str] = &["png", "jpg", "webp", "ico"];
const VIDEO: &[&str] = &["mp4", "mkv", "webm", "mov", "avi"];
const AUDIO: &[&str] = &["mp3", "m4a", "opus", "ogg", "flac", "wav"];
const DATA_STRUCT: &[&str] = &["csv", "json", "yaml", "toml"];

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
    for &a in VIDEO {
        for &b in VIDEO {
            add(a, b, EngineId::Ffmpeg, 3);
        }
        add(a, "gif", EngineId::Ffmpeg, 3);
        for &b in AUDIO {
            add(a, b, EngineId::Ffmpeg, 3);
        }
    }
    // Animated GIF -> video (ffmpeg keeps all frames).
    add("gif", "mp4", EngineId::Ffmpeg, 3);
    add("gif", "webm", EngineId::Ffmpeg, 3);
    for &a in AUDIO {
        for &b in AUDIO {
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

    #[test]
    fn every_edge_uses_known_formats() {
        for e in edges() {
            assert!(by_id(e.from).is_some(), "unknown from {}", e.from);
            assert!(by_id(e.to).is_some(), "unknown to {}", e.to);
            assert_ne!(e.from, e.to);
        }
    }
}

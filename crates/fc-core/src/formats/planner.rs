//! Route planner over [`super::registry`] edges and the target list for a drop.

use super::registry::{EngineId, edges_from};
use super::{FORMATS, Kind, by_id};
use serde::Serialize;
use std::collections::BTreeSet;

/// Max conversion steps in one route (e.g. svg -> png -> pdf is 2).
pub const MAX_HOPS: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub from: &'static str,
    pub to: &'static str,
    pub engine: EngineId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Route {
    pub steps: Vec<Step>,
}

impl Route {
    pub fn engines(&self) -> impl Iterator<Item = EngineId> + '_ {
        self.steps.iter().map(|s| s.engine)
    }
}

/// One choice shown in the wheel / Convert view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetOption {
    /// Format id (e.g. "webp").
    pub id: &'static str,
    pub label: &'static str,
    pub kind: Kind,
    /// External tool ids (manifest ids) any input's route needs, deduped, sorted.
    pub needs_tools: Vec<&'static str>,
}

/// Cheapest route `from -> to` with at most [`MAX_HOPS`] steps (Dijkstra on edge cost,
/// ties broken by fewer steps). `None` if unreachable or `from == to`.
pub fn plan(from: &str, to: &str) -> Option<Route> {
    if from == to {
        return None;
    }
    // With MAX_HOPS = 2 an exhaustive search over 1- and 2-step paths is exactly
    // hop-limited Dijkstra, and the graph is tiny.
    let mut best: Option<(u32, Vec<Step>)> = None;
    let mut consider = |cost: u32, steps: Vec<Step>| {
        let better = match &best {
            None => true,
            Some((c, s)) => cost < *c || (cost == *c && steps.len() < s.len()),
        };
        if better {
            best = Some((cost, steps));
        }
    };
    let step = |e: &super::registry::Edge| Step {
        from: e.from,
        to: e.to,
        engine: e.engine,
    };
    for e1 in edges_from(from) {
        if e1.to == to {
            consider(e1.cost, vec![step(e1)]);
        } else if MAX_HOPS >= 2 && e1.chain {
            for e2 in edges_from(e1.to) {
                if e2.to == to {
                    consider(e1.cost + e2.cost, vec![step(e1), step(e2)]);
                }
            }
        }
    }
    best.map(|(_, steps)| Route { steps })
}

fn kind_rank(k: Kind) -> u8 {
    match k {
        Kind::Image => 0,
        Kind::Vector => 1,
        Kind::Video => 2,
        Kind::Audio => 3,
        Kind::Pdf => 4,
        Kind::Data => 5,
        Kind::Subtitle => 6,
        Kind::Archive => 7,
    }
}

/// Targets valid for *every* input format, in a stable UI order:
/// grouped by `Kind` in the order Image, Vector, Video, Audio, Pdf, Data, Subtitle, Archive,
/// then by the order of `FORMATS`.
/// A target is valid when each input either already is that format or has a route to it,
/// and at least one input differs from it. Empty or unknown inputs -> empty list.
pub fn targets_for(input_formats: &[&str]) -> Vec<TargetOption> {
    if input_formats.is_empty() || input_formats.iter().any(|f| by_id(f).is_none()) {
        return Vec::new();
    }
    let mut out = Vec::new();
    'target: for f in FORMATS {
        let mut tools = BTreeSet::new();
        let mut differs = false;
        for &input in input_formats {
            if input == f.id {
                continue;
            }
            differs = true;
            match plan(input, f.id) {
                Some(route) => tools.extend(route.engines().filter_map(EngineId::required_tool)),
                None => continue 'target,
            }
        }
        if differs {
            out.push(TargetOption {
                id: f.id,
                label: f.label,
                kind: f.kind,
                needs_tools: tools.into_iter().collect(),
            });
        }
    }
    out.sort_by_key(|t| kind_rank(t.kind)); // stable: keeps FORMATS order within a kind
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(t: &[TargetOption]) -> Vec<&'static str> {
        t.iter().map(|t| t.id).collect()
    }

    #[test]
    fn direct_route_preferred() {
        let r = plan("png", "webp").unwrap();
        assert_eq!(r.steps.len(), 1);
        assert_eq!(r.steps[0].engine, EngineId::Image);
    }

    #[test]
    fn two_hop_route_svg_to_pdf() {
        let r = plan("svg", "pdf").unwrap();
        assert_eq!(r.steps.len(), 2);
        assert_eq!(r.steps[0].from, "svg");
        assert_eq!(r.steps[1].to, "pdf");
    }

    #[test]
    fn unreachable_and_identity() {
        assert!(plan("png", "png").is_none());
        assert!(plan("mp3", "png").is_none());
        assert!(plan("csv", "mp4").is_none());
    }

    #[test]
    fn png_targets_include_other_rasters_and_pdf_not_itself() {
        let t = targets_for(&["png"]);
        let v = ids(&t);
        assert!(v.contains(&"jpg") && v.contains(&"webp") && v.contains(&"pdf"));
        assert!(!v.contains(&"png"));
        assert!(!v.contains(&"mp4"));
        assert!(t.iter().all(|t| t.needs_tools.is_empty()));
    }

    #[test]
    fn mixed_inputs_intersect_and_keep_shared_format() {
        let v = ids(&targets_for(&["png", "jpg"]));
        assert!(
            v.contains(&"png"),
            "png valid: jpg converts, png already is"
        );
        assert!(v.contains(&"webp"));
        assert!(ids(&targets_for(&["png", "mp3"])).is_empty());
    }

    #[test]
    fn video_targets_need_ffmpeg_and_are_ordered_by_kind() {
        let t = targets_for(&["mp4"]);
        let v = ids(&t);
        assert_eq!(v[0], "gif", "image kinds first");
        assert!(v.contains(&"mp3"));
        assert!(
            !v.contains(&"pdf") && !v.contains(&"png"),
            "no odd routes through gif"
        );
        assert!(t.iter().all(|t| t.needs_tools == vec!["ffmpeg"]));
    }

    #[test]
    fn pdf_reaches_webp_through_png() {
        let r = plan("pdf", "webp").unwrap();
        assert_eq!(
            r.engines().collect::<Vec<_>>(),
            [EngineId::Pdfium, EngineId::Image]
        );
    }

    #[test]
    fn unknown_or_empty_inputs() {
        assert!(targets_for(&[]).is_empty());
        assert!(targets_for(&["nope"]).is_empty());
    }
}

//! Ctrl+Shift+1..9: paste a pinned clip into the window in front, without opening the line.
//! The keys count the pinned clips in the order the line shows them: rope by rope from the top,
//! left to right on each rope.

use super::paste::paste_keys_into;
use crate::state::AppState;
use fc_core::clip::{ClipItem, ClipKind};
use tauri::{AppHandle, Manager};

/// The line rope a clip hangs on (`ropeOf` in `src/line/logic.ts`).
fn rope_of(item: &ClipItem) -> &'static str {
    match item.kind {
        ClipKind::Image => "images",
        ClipKind::Text => "text",
        ClipKind::Link | ClipKind::MediaLink => "links",
    }
}

/// Where a clip sits on its rope (`clipRank`): where it was dragged to, else its time.
fn rank(item: &ClipItem) -> f64 {
    item.rank.unwrap_or(item.created_at as f64)
}

/// Ids of the pinned clips in line order: ropes in `rope_order` (only the `shown` ones), and on
/// each rope by rank, highest (leftmost) first, then the newer id. Trashed clips never count.
pub fn pinned_order(
    items: &[ClipItem],
    rope_order: &[String],
    shown: impl Fn(&str) -> bool,
) -> Vec<u64> {
    let mut out = Vec::new();
    for rope in rope_order.iter().filter(|r| shown(r)) {
        let mut on_rope: Vec<&ClipItem> = items
            .iter()
            .filter(|i| i.pinned && i.trashed_at.is_none() && rope_of(i) == rope)
            .collect();
        on_rope.sort_by(|a, b| rank(b).total_cmp(&rank(a)).then(b.id.cmp(&a.id)));
        out.extend(on_rope.iter().map(|i| i.id));
    }
    out
}

/// Ctrl+Shift+`n` (1-based): copy the `n`th pinned clip and paste it into the window in front.
/// Without such a clip a toast says so.
pub fn paste_pinned(app: &AppHandle, n: usize) {
    let state = app.state::<AppState>();
    let id = {
        let s = state.settings();
        let ropes = &s.line_ropes;
        let shown = |r: &str| match r {
            "images" => ropes.images,
            "text" => ropes.text,
            "links" => ropes.links,
            _ => false,
        };
        let store = state.clips().lock().unwrap_or_else(|e| e.into_inner());
        pinned_order(store.items(), &s.line_rope_order, shown)
            .get(n.wrapping_sub(1))
            .copied()
    };
    let Some(id) = id else {
        crate::windows::show_toast(app, "noPinned", serde_json::json!({ "n": n }), Some(3000));
        return;
    };
    if let Err(e) = crate::ipc::clipboard::copy_items(&state, &[id]) {
        eprintln!("pinned paste {n}: {}", e.message);
        return;
    }
    #[cfg(windows)]
    paste_keys_into(crate::platform::win::foreground_window());
    #[cfg(not(windows))]
    let _ = paste_keys_into;
}

#[cfg(test)]
mod tests {
    use super::pinned_order;
    use fc_core::clip::{ClipItem, ClipKind};

    fn clip(id: u64, kind: ClipKind, pinned: bool, created_at: u64, rank: Option<f64>) -> ClipItem {
        let mut v = serde_json::json!({
            "id": id, "kind": kind, "hash": format!("h{id}"), "createdAt": created_at,
            "pinned": pinned,
        });
        if let Some(r) = rank {
            v["rank"] = r.into();
        }
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn pinned_clips_in_line_order() {
        let items = vec![
            clip(1, ClipKind::Text, true, 100, None),
            clip(2, ClipKind::Text, true, 200, None), // newer: further left
            clip(3, ClipKind::Image, true, 50, None),
            clip(4, ClipKind::Link, true, 10, None),
            clip(5, ClipKind::MediaLink, true, 20, Some(5.0)), // dragged: rank 5 < 10
            clip(6, ClipKind::Text, false, 999, None),         // not pinned
            clip(7, ClipKind::Text, true, 300, Some(1.0)),     // dragged to the right end
        ];
        let order: Vec<String> = ["images", "text", "links", "files"]
            .map(String::from)
            .to_vec();
        assert_eq!(
            pinned_order(&items, &order, |_| true),
            vec![3, 2, 1, 7, 4, 5]
        );
        // Another rope order, the images rope hidden.
        let order: Vec<String> = ["links", "text", "images"].map(String::from).to_vec();
        assert_eq!(
            pinned_order(&items, &order, |r| r != "images"),
            vec![4, 5, 2, 1, 7]
        );
    }

    #[test]
    fn trashed_pins_do_not_count() {
        let mut items = vec![clip(1, ClipKind::Text, true, 1, None)];
        items[0].trashed_at = Some(5);
        let order = vec!["text".to_string()];
        assert!(pinned_order(&items, &order, |_| true).is_empty());
    }
}

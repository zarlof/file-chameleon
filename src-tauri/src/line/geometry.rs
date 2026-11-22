//! Pure layout numbers of the line window: band heights, the size that fits, the window
//! rectangle on a work area, and the full-screen guard. Mirrors `src/line/layout.ts`; the
//! numbers are the ones in `src/line/sizes.json` (checked by a test).

use super::*;

/// Logical metrics: the gap per band (6 above and 6 below, so the window is `n × (band + 12)`
/// high). These must equal the layout table the page uses (`src/line/sizes.json`), so nothing
/// transparent hangs under the last rope. The window spans the whole work-area width.
pub(super) const BAND_GAP: f64 = 12.0;
/// Logical height of a folded rope's band (its label and the gap below it). Equal to
/// `foldH` in `src/line/sizes.json`.
pub(super) const FOLD_H: f64 = 30.0;

/// Logical height of one rope's band for a `lineSize` ("small" | "medium" | "large"; anything
/// else is medium).
pub fn band_height(size: &str) -> f64 {
    match size {
        "small" => 144.0,
        "large" => 222.0,
        _ => 178.0,
    }
}

/// Logical height of the window for `ropes` visible ropes (at least one): `n × (band + 12)`.
pub fn window_height(ropes: usize, size: &str) -> f64 {
    ropes.max(1) as f64 * (band_height(size) + BAND_GAP)
}

/// Logical height of the window for `open` full ropes and `folded` folded ones: like
/// [`window_height`] when nothing is folded.
pub fn strip_height(open: usize, folded: usize, size: &str) -> f64 {
    if folded == 0 || open + folded == 0 {
        return window_height(open, size);
    }
    open as f64 * (band_height(size) + BAND_GAP) + folded as f64 * FOLD_H
}

/// The line's size for the ropes on a work area `work_h` logical px high: the chosen size, or
/// the next smaller one(s) when the ropes do not all fit (so every rope stays visible). The page
/// makes the same choice (`fitSize` in `src/line/layout.ts`).
pub fn fit_size(open: usize, folded: usize, size: &str, work_h: f64) -> &'static str {
    const ORDER: [&str; 3] = ["large", "medium", "small"];
    let start = ORDER.iter().position(|s| *s == size).unwrap_or(1);
    for s in &ORDER[start..] {
        if strip_height(open, folded, s) <= work_h + 0.5 {
            return s;
        }
    }
    "small"
}

/// The most whole ropes that fit a work area `work_h` logical pixels high at `size`
/// (at least one). The page uses the same rule (`maxRopes` in `src/line/layout.ts`).
pub fn max_ropes(work_h: f64, size: &str) -> usize {
    let n = (work_h / (band_height(size) + BAND_GAP) + 0.01).floor();
    if n.is_finite() && n >= 1.0 {
        n as usize
    } else {
        1
    }
}

/// How long the pointer must stay in the edge zone before the line reveals itself.
pub fn dwell(ms: u32) -> Duration {
    Duration::from_millis(u64::from(ms))
}

/// Physical `(x, y, width, height)` of the line in the work area `(left, top, right, bottom)`
/// (physical pixels) of a monitor with DPI `scale`: the full work-area width, height
/// [`window_height`] (at least one rope, never taller than the work area), at the top of the
/// work area, or on its bottom when `edge` is "bottom".
#[cfg(test)]
pub fn line_rect(
    work: (i32, i32, i32, i32),
    scale: f64,
    ropes: usize,
    size: &str,
    edge: &str,
) -> (i32, i32, i32, i32) {
    line_rect_folded(work, scale, ropes, 0, size, edge)
}

/// [`line_rect`] with `folded` of the ropes folded away (they take [`FOLD_H`] each). With folded
/// ropes the height is only capped by the work area; the page draws the ropes that fit.
pub fn line_rect_folded(
    work: (i32, i32, i32, i32),
    scale: f64,
    ropes: usize,
    folded: usize,
    size: &str,
    edge: &str,
) -> (i32, i32, i32, i32) {
    let (left, top, right, bottom) = work;
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let (work_w, work_h) = ((right - left).max(1), (bottom - top).max(1));
    let folded = folded.min(ropes);
    let size = fit_size(ropes - folded, folded, size, f64::from(work_h) / scale);
    let height = if folded == 0 {
        let ropes = ropes.min(max_ropes(f64::from(work_h) / scale, size));
        fit_height(ropes, size, scale, Some(work_h))
    } else {
        let h = (strip_height(ropes - folded, folded, size) * scale).round() as i32;
        h.min(work_h).max(1)
    };
    let y = if edge == "bottom" {
        bottom - height
    } else {
        top
    };
    (left, y, work_w, height)
}

/// Does the foreground window count as "full screen"? Its rectangle equals its monitor's and it
/// is not the desktop or the shell. A maximized window stops at the work area, so it is not.
pub fn fullscreen_guard(
    fg: (i32, i32, i32, i32),
    monitor: (i32, i32, i32, i32),
    class: &str,
) -> bool {
    fg == monitor && !matches!(class, "Progman" | "WorkerW" | "Shell_TrayWnd")
}

/// Physical window height for `ropes` ropes of `size` at DPI `scale`, at most the work area's
/// height `work_h`.
pub fn fit_height(ropes: usize, size: &str, scale: f64, work_h: Option<i32>) -> i32 {
    let mut height = (window_height(ropes, size) * scale).round() as i32;
    if let Some(h) = work_h {
        height = height.min(h.max(1));
    }
    height.max(1)
}

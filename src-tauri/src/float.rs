//! Floating image windows (`float.html?path=...`, labels `float-<n>`): "pin to screen".
//!
//! Each window is frameless, always on top and off the taskbar, sized to the image's aspect to
//! fit 40 % of the work area. They are built on a worker thread: a webview must not be built on
//! the main thread (see the `ipc` module docs), whoever calls [`open`].

use crate::state::AppState;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

/// At most this many float windows are open at once.
const MAX_WINDOWS: usize = 6;
/// Share of the work area a window may fill.
const WORK_SHARE: f64 = 0.4;
/// Tiny images are shown at least this large along their longer side (logical pixels).
const MIN_LONG_SIDE: f64 = 160.0;
/// Each further window is moved down and right by this much (logical pixels).
const CASCADE: f64 = 32.0;

static COUNTER: AtomicU32 = AtomicU32::new(1);

/// Logical `(width, height)` of a float window for an image of `img` pixels inside a box of
/// `max` logical pixels: shrunk to fit, never enlarged past 1:1 except that a tiny image is
/// brought up to [`MIN_LONG_SIDE`] (and still fits the box).
pub fn fit_size(img: (u32, u32), max: (f64, f64)) -> (u32, u32) {
    let (w, h) = (f64::from(img.0.max(1)), f64::from(img.1.max(1)));
    let (mw, mh) = (max.0.max(1.0), max.1.max(1.0));
    let long = w.max(h);
    let wanted = if long < MIN_LONG_SIDE {
        MIN_LONG_SIDE / long
    } else {
        1.0
    };
    let scale = wanted.min(mw / w).min(mh / h);
    (
        (w * scale).round().max(1.0) as u32,
        (h * scale).round().max(1.0) as u32,
    )
}

/// Percent-encode for a URL query value (everything but unreserved characters, UTF-8 bytes).
pub fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// How many more float windows may open when `existing` are already open.
pub fn slots_left(existing: usize) -> usize {
    MAX_WINDOWS.saturating_sub(existing)
}

/// Pixel size of an image file without decoding it.
fn image_size(path: &std::path::Path) -> Option<(u32, u32)> {
    image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
}

/// Open floating windows for `paths`, up to six open in total (the rest are skipped).
pub fn open(app: &AppHandle, paths: Vec<PathBuf>) {
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("fc-float".into())
        .spawn(move || {
            let existing = app
                .webview_windows()
                .keys()
                .filter(|l| l.starts_with("float-"))
                .count();
            for (i, path) in paths.iter().take(slots_left(existing)).enumerate() {
                if let Err(e) = open_one(&app, path, i) {
                    eprintln!("float window: {e}");
                }
            }
        });
    if let Err(e) = spawned {
        eprintln!("float window: {e}");
    }
}

fn open_one(app: &AppHandle, path: &std::path::Path, index: usize) -> Result<(), String> {
    let dims = image_size(path).ok_or_else(|| format!("not an image: {}", path.display()))?;
    // The static asset scope only covers the clip store: let the page show this one file.
    app.asset_protocol_scope()
        .allow_file(path)
        .map_err(|e| e.to_string())?;
    // Work area of the monitor under the cursor.
    #[cfg(windows)]
    let (work, scale) = {
        let (cx, cy) = crate::platform::win::cursor_pos();
        let work = crate::platform::win::work_area_at(cx, cy).unwrap_or((0, 0, 1920, 1040));
        let scale = app
            .monitor_from_point(f64::from(cx), f64::from(cy))
            .ok()
            .flatten()
            .map_or(1.0, |m| m.scale_factor());
        (work, scale)
    };
    #[cfg(not(windows))]
    let (work, scale) = ((0, 0, 1920, 1040), 1.0);
    let work_w = f64::from(work.2 - work.0) / scale;
    let work_h = f64::from(work.3 - work.1) / scale;
    let (w, h) = fit_size(dims, (work_w * WORK_SHARE, work_h * WORK_SHARE));
    let step = (index as f64) * CASCADE;
    let x = f64::from(work.0) / scale + work_w * 0.1 + step;
    let y = f64::from(work.1) / scale + work_h * 0.1 + step;

    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let url = format!("float.html?path={}", url_encode(&path.to_string_lossy()));
    let state = app.state::<AppState>();
    WebviewWindowBuilder::new(app, format!("float-{n}"), WebviewUrl::App(url.into()))
        .title("File Chameleon")
        .inner_size(f64::from(w), f64::from(h))
        .position(x, y)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .transparent(false)
        .resizable(true)
        .maximizable(false)
        .minimizable(false)
        .data_directory(state.paths.webview_dir())
        .additional_browser_args(&crate::windows::browser_args())
        .build()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_size_cases() {
        // Wide image: width bound.
        assert_eq!(fit_size((4000, 2000), (768.0, 416.0)), (768, 384));
        // Tall image: height bound.
        assert_eq!(fit_size((1000, 4000), (768.0, 416.0)), (104, 416));
        // Smaller than the box: shown 1:1, not enlarged.
        assert_eq!(fit_size((400, 300), (768.0, 416.0)), (400, 300));
        // Tiny image: brought up to 160 on its long side.
        assert_eq!(fit_size((16, 16), (768.0, 416.0)), (160, 160));
        // A tiny box still wins over the minimum; zero sizes do not divide by zero.
        assert_eq!(fit_size((16, 16), (100.0, 100.0)), (100, 100));
        assert_eq!(fit_size((0, 0), (100.0, 100.0)).0, 100);
    }

    #[test]
    fn float_cap_is_total() {
        assert_eq!(slots_left(0), 6);
        assert_eq!(slots_left(4), 2);
        assert_eq!(slots_left(6), 0);
        assert_eq!(slots_left(9), 0);
    }

    #[test]
    fn url_encode_cases() {
        assert_eq!(url_encode("abc-_.~123"), "abc-_.~123");
        assert_eq!(url_encode(r"C:\a b\ç.png"), "C%3A%5Ca%20b%5C%C3%A7.png");
        assert_eq!(url_encode("a&b=c?d#e"), "a%26b%3Dc%3Fd%23e");
    }
}

//! The clothesline window (`line.html`, label `line`) and the file actions of the wheel.
//! OWNER: clothesline Task 2.
//!
//! Created on first use, then only hidden and shown again. Show modes:
//! - `hotkey`: activates the window; the foreground window before it is remembered so
//!   `line_paste` can paste there.
//! - `peek` (after a copy) and `reveal` (pointer dwelling at the screen edge): never take focus
//!   (`SW_SHOWNOACTIVATE` + `WS_EX_NOACTIVATE`) and never appear over a full-screen window. A
//!   click inside the page calls `line_activate`.
//!
//! The window is shown and hidden with raw `ShowWindow` calls only (tao is never told, see
//! `win::show_window`), so visibility is always asked from Windows, not from Tauri.
//!
//! Creating the webview must not happen on the main thread inside a WebView2 callback (see the
//! `ipc` module docs): callers are the hotkey action thread, the dwell thread, a peek thread and
//! async commands.

use crate::events::{self, LineShow, NavPayload};
use crate::ipc::{CmdError, CmdResult};
use crate::state::AppState;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

pub const LINE: &str = "line";

mod actions;
mod geometry;
mod paste;

pub use actions::*;
pub use geometry::*;
use paste::*;

/// How a show mode treats focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShowFlags {
    /// Only the hotkey activates; peek, reveal and anything unknown never steal focus.
    pub activate: bool,
}

pub fn show_flags(mode: &str) -> ShowFlags {
    ShowFlags {
        activate: matches!(mode, "hotkey" | "search"),
    }
}

// ---------------------------------------------------------------- window

/// Makes "look up the window, else create it" atomic: the hotkey, a peek and a reveal can arrive
/// together on the first use.
static CREATE: Mutex<()> = Mutex::new(());

/// The page of the window. The first show's mode is not passed in the URL: the page pulls it with
/// `line_state` once its listeners are attached.
const LINE_PAGE: &str = "line.html";

/// What the page asks for with `line_state`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct LineState {
    /// "hotkey" | "peek" | "reveal" | "search"; `None` when the line is not up.
    pub mode: Option<String>,
    pub visible: bool,
}

/// The mode the line shows in when `new` is requested while `current` is already pending: an
/// activating mode (hotkey, search) is never downgraded by a peek or reveal; otherwise the
/// latest wins.
pub fn merge_mode<'a>(current: Option<&'a str>, new: &'a str) -> &'a str {
    match (current, new) {
        (Some(c @ ("hotkey" | "search")), "peek" | "reveal") => c,
        _ => new,
    }
}

/// Latest requested mode since the last hide.
static MODE: Mutex<Option<String>> = Mutex::new(None);

/// Serializes [`show_at`]: a peek racing the hotkey must not re-set `WS_EX_NOACTIVATE` on a line
/// that was just activated.
static SHOW: Mutex<()> = Mutex::new(());

/// Number of ropes the window has room for: every enabled one (empty ropes show too, the page
/// lets the user fold them). At least 1 (the empty message).
pub fn rope_count_of(enabled: &[bool]) -> usize {
    enabled.iter().filter(|on| **on).count().max(1)
}

/// Hide on focus loss? Only a line that was activated (hotkey, or a click on a peek).
pub fn should_hide_on_blur(active: bool, focused: bool) -> bool {
    active && !focused
}

/// The line was activated (the hotkey or a click) and has not lost focus since.
static ACTIVE: AtomicBool = AtomicBool::new(false);

fn create(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let state = app.state::<AppState>();
    let (ropes, folded) = rope_count(app);
    let work = (0, 0, 1920, 1040);
    let s = state.settings();
    let (_, _, w, h) = line_rect_folded(work, 1.0, ropes, folded, &s.line_size, &s.line_edge);
    WebviewWindowBuilder::new(app, LINE, WebviewUrl::App(LINE_PAGE.into()))
        .title("File Chameleon Line")
        .inner_size(f64::from(w), f64::from(h))
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .shadow(false)
        .focused(false)
        .visible(false)
        .data_directory(state.paths.webview_dir())
        .additional_browser_args(&crate::windows::browser_args())
        .build()
}

/// The window and whether this call created it. A destroyed window may keep its label for a
/// moment, so a failed build is retried for up to 500 ms (reusing the old one if it is back).
fn get_or_create(app: &AppHandle) -> Option<(WebviewWindow, bool)> {
    let _guard = CREATE.lock().unwrap_or_else(|e| e.into_inner());
    let deadline = std::time::Instant::now() + Duration::from_millis(500);
    loop {
        if let Some(w) = app.get_webview_window(LINE) {
            return Some((w, false));
        }
        match create(app) {
            Ok(w) => return Some((w, true)),
            Err(e) if std::time::Instant::now() >= deadline => {
                eprintln!("line window: {e}");
                return None;
            }
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

/// How many ropes the window needs right now and how many of them are folded.
fn rope_count(app: &AppHandle) -> (usize, usize) {
    let s = app.state::<AppState>().settings();
    rope_count_with(app, &s.line_ropes, &s.line_folded)
}

/// Like [`rope_count`] for settings that are not saved yet: (enabled ropes, how many of them are
/// folded). Every enabled rope gets its band, empty or not.
fn rope_count_with(
    _app: &AppHandle,
    r: &fc_core::settings::LineRopes,
    folded: &[String],
) -> (usize, usize) {
    let keys = [
        ("images", r.images),
        ("text", r.text),
        ("links", r.links),
        ("files", r.files),
        ("shortcuts", r.shortcuts),
    ];
    let on: Vec<&str> = keys.iter().filter(|(_, on)| *on).map(|(k, _)| *k).collect();
    let folded = on.iter().filter(|k| folded.iter().any(|f| f == *k)).count();
    (rope_count_of(&keys.map(|(_, on)| on)), folded.min(on.len()))
}

/// `line_state`: what the page needs to catch up on a show it missed.
pub fn state(app: &AppHandle) -> LineState {
    #[cfg(windows)]
    let visible = app
        .get_webview_window(LINE)
        .and_then(|w| hwnd_of(&w))
        .is_some_and(crate::platform::win::is_visible);
    #[cfg(not(windows))]
    let visible = {
        let _ = app;
        false
    };
    let mode = MODE.lock().unwrap_or_else(|e| e.into_inner()).clone();
    LineState {
        visible,
        mode: if visible { mode } else { None },
    }
}

#[cfg(windows)]
fn hwnd_of(w: &WebviewWindow) -> Option<isize> {
    w.hwnd().ok().map(|h| h.0 as isize)
}

/// Size and place the line at the top of the work area of the monitor under `point`.
#[cfg(windows)]
fn place(app: &AppHandle, w: &WebviewWindow, point: (i32, i32)) {
    let Some(work) = crate::platform::win::work_area_at(point.0, point.1) else {
        return;
    };
    let scale = app
        .monitor_from_point(f64::from(point.0), f64::from(point.1))
        .ok()
        .flatten()
        .map(|m| m.scale_factor())
        .unwrap_or(1.0);
    let (ropes, folded) = rope_count(app);
    let s = app.state::<AppState>().settings();
    let (x, y, width, height) =
        line_rect_folded(work, scale, ropes, folded, &s.line_size, &s.line_edge);
    // Position first: moving to a monitor with another DPI may resize the window.
    let _ = w.set_position(PhysicalPosition::new(x, y));
    let _ = w.set_size(PhysicalSize::new(width as u32, height as u32));
}

/// Re-fit the (placed) window to the ropes, size and edge now, on the monitor it is on. Used for
/// a peek that arrives while the line is already up and when the size or edge changes.
#[cfg(windows)]
fn resize(app: &AppHandle, w: &WebviewWindow, s: &fc_core::settings::Settings) {
    let (Ok(pos), Ok(cur)) = (w.outer_position(), w.outer_size()) else {
        return;
    };
    let scale = w.scale_factor().unwrap_or(1.0);
    let Some(work) = crate::platform::win::work_area_at(pos.x, pos.y) else {
        return;
    };
    let (ropes, folded) = rope_count_with(app, &s.line_ropes, &s.line_folded);
    let (x, y, width, height) =
        line_rect_folded(work, scale, ropes, folded, &s.line_size, &s.line_edge);
    if (x, y) != (pos.x, pos.y) {
        let _ = w.set_position(PhysicalPosition::new(x, y));
    }
    if (width, height) != (cur.width as i32, cur.height as i32) {
        let _ = w.set_size(PhysicalSize::new(width as u32, height as u32));
    }
}

/// The size or rope settings changed (`s` = the NEW, not yet saved settings): re-fit the window
/// when it is up.
pub fn apply_size(app: &AppHandle, s: &fc_core::settings::Settings) {
    #[cfg(windows)]
    if let Some(w) = app.get_webview_window(LINE)
        && hwnd_of(&w).is_some_and(crate::platform::win::is_visible)
    {
        resize(app, &w, s);
    }
    #[cfg(not(windows))]
    let _ = (app, s);
}

/// A rope appeared or emptied: re-fit the window (if it is up) with the saved settings.
pub fn refit(app: &AppHandle) {
    apply_size(app, &app.state::<AppState>().settings());
}

/// Remember the foreground window so `line_paste` can paste into it. `force` (the hotkey) also
/// records "none" (our own window, the taskbar); otherwise only a real foreign window counts.
#[cfg(windows)]
fn remember_foreground(app: &AppHandle, force: bool) {
    use crate::platform::win;
    let fg = win::foreground_window();
    let usable = fg != 0 && !win::is_own_window(fg) && !is_taskbar_class(&win::window_class(fg));
    if usable || force {
        app.state::<AppState>()
            .line_prev
            .store(if usable { fg } else { 0 }, Ordering::SeqCst);
    }
}

/// Show the line in `mode` ("hotkey" | "peek" | "reveal" | "search"), creating it on first use.
pub fn show(app: &AppHandle, mode: &str) {
    show_at(app, mode, None);
}

/// Like [`show`]; `at` is the physical point whose monitor the line appears on (reveal: where
/// the top edge was hit). Without it a peek uses the monitor of the foreground window and the
/// hotkey the one under the cursor.
pub fn show_at(app: &AppHandle, mode: &str, at: Option<(i32, i32)>) {
    #[cfg(not(windows))]
    let _ = (app, mode, at);
    #[cfg(windows)]
    {
        use crate::platform::win;
        if !app.state::<AppState>().settings().line_enabled {
            return;
        }
        let flags = show_flags(mode);
        // The guard is for the uninvited modes: the hotkey always works.
        if !flags.activate && win::fullscreen_foreground() {
            return;
        }
        let _serial = SHOW.lock().unwrap_or_else(|e| e.into_inner());
        if flags.activate {
            remember_foreground(app, true);
        }
        let Some((w, _created)) = get_or_create(app) else {
            return;
        };
        let Some(h) = hwnd_of(&w) else { return };
        let merged = {
            let mut m = MODE.lock().unwrap_or_else(|e| e.into_inner());
            let merged = merge_mode(m.as_deref(), mode).to_string();
            *m = Some(merged.clone());
            merged
        };
        if !flags.activate && win::is_visible(h) {
            // Already up (maybe in use): do not touch its focus style or place, but the rope
            // count may have changed, so resize; tell the page so it keeps itself open.
            resize(app, &w, &app.state::<AppState>().settings());
            let _ = app.emit_to(LINE, events::LINE_SHOW, LineShow { mode: merged });
            return;
        }
        let point = at
            .or_else(|| (mode == "peek").then(win::foreground_point).flatten())
            .unwrap_or_else(win::cursor_pos);
        // The window is re-sized on every show: the rope count can have changed.
        place(app, &w, point);
        win::show_window(h, flags.activate);
        if flags.activate {
            win::set_foreground(h);
            let _ = w.set_focus();
            ACTIVE.store(true, Ordering::SeqCst);
        }
        // After the window is up, also for a page that has only just been built: a page that
        // attaches its listener later pulls the same mode with `line_state`.
        let _ = app.emit_to(LINE, events::LINE_SHOW, LineShow { mode: merged });
    }
}

/// Hide the line (the window and its page stay alive).
pub fn hide(app: &AppHandle) {
    #[cfg(windows)]
    {
        edge_leave();
        ACTIVE.store(false, Ordering::SeqCst);
        *MODE.lock().unwrap_or_else(|e| e.into_inner()) = None;
        let Some(h) = app.get_webview_window(LINE).and_then(|w| hwnd_of(&w)) else {
            return;
        };
        if crate::platform::win::is_visible(h) {
            crate::platform::win::hide(h);
            let _ = app.emit_to(LINE, events::LINE_HIDE, ());
        }
    }
    #[cfg(not(windows))]
    let _ = app;
}

/// Close the line's window for good (the clothesline was switched off).
pub fn destroy(app: &AppHandle) {
    hide(app);
    if let Some(w) = app.get_webview_window(LINE) {
        let _ = w.destroy();
    }
}

/// The search hotkey: show the line in `search` mode (activated, the page focuses its search
/// field). When it is already in front: in `search` mode it hides again, in any other mode it
/// switches to `search` instead of closing.
pub fn toggle_search(app: &AppHandle) {
    #[cfg(windows)]
    {
        let in_front = app
            .get_webview_window(LINE)
            .and_then(|w| hwnd_of(&w))
            .is_some_and(|h| {
                crate::platform::win::is_visible(h)
                    && h == crate::platform::win::foreground_window()
            });
        if in_front {
            let current = MODE.lock().unwrap_or_else(|e| e.into_inner()).clone();
            if current.as_deref() == Some("search") {
                hide(app);
            } else {
                *MODE.lock().unwrap_or_else(|e| e.into_inner()) = Some("search".into());
                let _ = app.emit_to(
                    LINE,
                    events::LINE_SHOW,
                    LineShow {
                        mode: "search".into(),
                    },
                );
            }
            return;
        }
    }
    show(app, "search");
}

/// The line hotkey: hide the line when it is in front, show and activate it otherwise (also when
/// it is only peeking).
pub fn toggle_hotkey(app: &AppHandle) {
    #[cfg(windows)]
    {
        let in_front = app
            .get_webview_window(LINE)
            .and_then(|w| hwnd_of(&w))
            .is_some_and(|h| {
                crate::platform::win::is_visible(h)
                    && h == crate::platform::win::foreground_window()
            });
        if in_front {
            hide(app);
            return;
        }
    }
    show(app, "hotkey");
}

/// The line window lost focus: close it if it was active.
pub fn on_blur(app: &AppHandle) {
    if should_hide_on_blur(ACTIVE.load(Ordering::SeqCst), false) {
        hide(app);
    }
}

/// A click inside a peeking line: take focus from now on. The window we came from is remembered
/// first (a foreign foreground window only), so paste still goes there.
pub fn activate(app: &AppHandle) {
    #[cfg(windows)]
    {
        use crate::platform::win;
        let Some(w) = app.get_webview_window(LINE) else {
            return;
        };
        let Some(h) = hwnd_of(&w) else { return };
        remember_foreground(app, false);
        win::set_no_activate(h, false);
        win::set_foreground(h);
        let _ = w.set_focus();
        ACTIVE.store(true, Ordering::SeqCst);
    }
    #[cfg(not(windows))]
    let _ = app;
}

/// A copy (or an edited image) arrived: let the line peek in, if the user wants that.
/// Runs on its own thread: the first call builds the webview.
pub fn peek(app: &AppHandle) {
    let s = app.state::<AppState>().settings();
    if !(s.line_enabled && s.line_peek_on_copy) {
        return;
    }
    peek_forced(app);
}

/// Like [`peek`] but ignores `line_peek_on_copy`: feedback for an action the user asked for
/// (hang, capture). Still needs the line enabled and respects the full-screen guard (in `show`).
fn peek_forced(app: &AppHandle) {
    if !app.state::<AppState>().settings().line_enabled {
        return;
    }
    #[cfg(windows)]
    {
        // Copying inside one of our own windows (the library, the editor): nothing to peek at.
        let fg = crate::platform::win::foreground_window();
        if fg != 0 && crate::platform::win::is_own_window(fg) {
            return;
        }
    }
    let app = app.clone();
    let _ = std::thread::Builder::new()
        .name("fc-line-peek".into())
        .spawn(move || show(&app, "peek"));
}

// ---------------------------------------------------------------- edge reveal

/// Bumped on every enter, leave and hide: a dwell timer only fires if nothing happened since.
static DWELL_GEN: AtomicU64 = AtomicU64::new(0);

/// The pointer entered the reveal zone at `(x, y)`: arm the dwell timer.
pub fn edge_enter(app: &AppHandle, x: i32, y: i32) {
    let s = app.state::<AppState>().settings();
    if !(s.line_enabled && s.line_edge_reveal) {
        return;
    }
    let wait = dwell(s.line_reveal_dwell_ms);
    let armed = DWELL_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    let _ = std::thread::Builder::new()
        .name("fc-line-dwell".into())
        .spawn(move || {
            std::thread::sleep(wait);
            if DWELL_GEN.load(Ordering::SeqCst) != armed {
                return;
            }
            #[cfg(windows)]
            {
                // Dragging something (a window to the screen edge, a selection): not a reveal.
                if !crate::platform::win::keys_down(&[0x01]).is_empty() {
                    return;
                }
                if !crate::platform::hook_pointer_in_zone() {
                    return;
                }
            }
            show_at(&app, "reveal", Some((x, y)));
        });
}

/// The pointer left the zone: cancel a pending reveal.
pub fn edge_leave() {
    DWELL_GEN.fetch_add(1, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_rect_spans_work_area() {
        let m = "medium";
        assert_eq!(
            line_rect((0, 0, 1920, 1040), 1.0, 3, m, "top"),
            (0, 0, 1920, 570)
        );
        assert_eq!(
            line_rect((0, 0, 2880, 1560), 1.5, 2, m, "top"),
            (0, 0, 2880, 570)
        );
        // Second monitor below a top taskbar; no ropes still gives one rope's height.
        assert_eq!(
            line_rect((1920, 40, 3840, 1080), 1.0, 0, m, "top"),
            (1920, 40, 1920, 190)
        );
        // Never taller than the work area: the size steps down, then only whole ropes fit.
        assert_eq!(line_rect((0, 0, 800, 300), 1.0, 3, m, "top").3, 156);
    }

    #[test]
    fn line_rect_bottom_edge() {
        // Above the taskbar: the work area ends at 1040, the window sits on its bottom.
        assert_eq!(
            line_rect((0, 0, 1920, 1040), 1.0, 3, "medium", "bottom"),
            (0, 470, 1920, 570)
        );
        // A clamped window holds whole (small) ropes only and sits on the bottom.
        assert_eq!(
            line_rect((0, 40, 800, 340), 1.0, 3, "medium", "bottom"),
            (0, 184, 800, 156)
        );
        // An unknown edge is the top.
        assert_eq!(line_rect((0, 0, 1920, 1040), 1.0, 3, "medium", "left").1, 0);
    }

    #[test]
    fn fit_height_clamps_4_large() {
        assert_eq!(window_height(4, "large"), 936.0);
        assert_eq!(fit_height(4, "large", 1.0, Some(900)), 900);
        assert_eq!(fit_height(4, "large", 1.0, Some(1040)), 936);
        // 4 large ropes on a 900 px work area: the line steps down to medium (760) so all fit.
        assert_eq!(line_rect((0, 0, 1600, 900), 1.0, 4, "large", "top").3, 760);
        // Not even small fits: whole small ropes only.
        assert_eq!(line_rect((0, 0, 1600, 400), 1.0, 4, "large", "top").3, 312);
    }

    #[test]
    fn max_ropes_fit_whole_ropes() {
        assert_eq!(max_ropes(688.0, "medium"), 3);
        assert_eq!(max_ropes(760.0, "medium"), 4);
        assert_eq!(max_ropes(100.0, "large"), 1);
        assert_eq!(max_ropes(900.0, "large"), 3);
        // 4 medium ropes on a 688 px work area: small (4 x 156 = 624) holds them all.
        let (_, _, _, h) = line_rect((0, 0, 1920, 688), 1.0, 4, "medium", "top");
        assert_eq!(h, 624);
        // High DPI: 1032 physical px at 1.5 = 688 logical.
        let (_, _, _, h) = line_rect((0, 0, 1920, 1032), 1.5, 4, "medium", "top");
        assert_eq!(h, 936);
    }

    #[test]
    fn size_steps_down_until_every_rope_fits() {
        assert_eq!(fit_size(5, 0, "large", 1040.0), "medium");
        assert_eq!(fit_size(4, 1, "large", 1040.0), "large");
        assert_eq!(fit_size(5, 0, "medium", 1040.0), "medium");
        assert_eq!(fit_size(5, 0, "medium", 800.0), "small");
        assert_eq!(fit_size(9, 0, "small", 100.0), "small");
        assert_eq!(fit_size(2, 0, "weird", 1040.0), "medium");
    }

    #[test]
    fn folded_ropes_take_a_thin_band() {
        assert_eq!(strip_height(3, 0, "medium"), window_height(3, "medium"));
        assert_eq!(strip_height(2, 3, "medium"), 2.0 * 190.0 + 3.0 * 30.0);
        assert_eq!(strip_height(0, 5, "medium"), 150.0);
        let (_, _, _, h) = line_rect_folded((0, 0, 1920, 1040), 1.0, 5, 3, "medium", "top");
        assert_eq!(h, 470);
        let (_, y, _, h) = line_rect_folded((0, 0, 1920, 1040), 2.0, 5, 3, "medium", "bottom");
        assert_eq!((y, h), (100, 940));
        // capped by the work area
        let (_, _, _, h) = line_rect_folded((0, 0, 1920, 300), 1.0, 5, 1, "large", "top");
        assert_eq!(h, 300);
    }

    #[test]
    fn dwell_comes_from_the_setting() {
        assert_eq!(dwell(350), Duration::from_millis(350));
        assert_eq!(dwell(150), Duration::from_millis(150));
    }

    #[test]
    fn fit_height_follows_the_passed_size() {
        // The height comes from the size argument, not from any stored setting.
        assert_eq!(fit_height(2, "small", 1.0, None), 312);
        assert_eq!(fit_height(2, "large", 1.0, None), 468);
        assert_eq!(fit_height(2, "large", 1.5, Some(600)), 600);
        assert_eq!(fit_height(0, "medium", 1.0, None), 190);
    }

    #[test]
    fn band_table_matches() {
        assert_eq!(band_height("small"), 144.0);
        assert_eq!(band_height("medium"), 178.0);
        assert_eq!(band_height("large"), 222.0);
        assert_eq!(band_height("weird"), 178.0);
        assert_eq!(window_height(3, "medium"), 570.0);
        assert_eq!(window_height(1, "small"), 156.0);
        assert_eq!(window_height(0, "large"), 234.0);
    }

    #[test]
    fn fullscreen_guard_cases() {
        let mon = (0, 0, 1920, 1080);
        assert!(fullscreen_guard(mon, mon, "UnityWndClass"));
        // A maximized window stops at the work area.
        assert!(!fullscreen_guard(
            (0, 0, 1920, 1040),
            mon,
            "Chrome_WidgetWin_1"
        ));
        for c in ["Progman", "WorkerW", "Shell_TrayWnd"] {
            assert!(!fullscreen_guard(mon, mon, c), "{c}");
        }
    }

    #[test]
    fn peek_never_activates() {
        assert!(!show_flags("peek").activate);
        assert!(!show_flags("reveal").activate);
        assert!(show_flags("hotkey").activate);
        assert!(show_flags("search").activate);
        // An unknown mode must never steal focus.
        assert!(!show_flags("whatever").activate);
    }

    #[test]
    fn rope_count_from_ropes_and_clips() {
        // Every enabled rope, empty or not; never fewer than one (the empty message).
        assert_eq!(rope_count_of(&[true; 5]), 5);
        assert_eq!(rope_count_of(&[true, false, true, true, false]), 3);
        assert_eq!(rope_count_of(&[false; 5]), 1);
    }

    #[test]
    fn hotkey_mode_is_not_downgraded() {
        assert_eq!(merge_mode(None, "peek"), "peek");
        assert_eq!(merge_mode(Some("peek"), "hotkey"), "hotkey");
        assert_eq!(merge_mode(Some("hotkey"), "peek"), "hotkey");
        assert_eq!(merge_mode(Some("peek"), "reveal"), "reveal");
        assert_eq!(merge_mode(Some("search"), "peek"), "search");
        assert_eq!(merge_mode(Some("hotkey"), "search"), "search");
    }

    #[test]
    fn blur_rules() {
        assert!(should_hide_on_blur(true, false));
        assert!(!should_hide_on_blur(true, true));
        assert!(!should_hide_on_blur(false, false));
    }

    #[test]
    fn terminal_arg_cases() {
        let t = |s: &str| terminal_arg(Path::new(s));
        assert_eq!(t("D:\\My Dir\\"), PathBuf::from("D:\\My Dir"));
        assert_eq!(t("D:\\My Dir"), PathBuf::from("D:\\My Dir"));
        assert_eq!(t("C:\\"), PathBuf::from("C:\\"));
    }

    #[test]
    fn copy_path_text_joins_with_crlf() {
        let a = PathBuf::from(r"C:\x\a.jpg");
        let b = PathBuf::from(r"C:\x\b");
        assert_eq!(copy_path_text(&[a.clone(), b]), "C:\\x\\a.jpg\r\nC:\\x\\b");
        assert_eq!(copy_path_text(&[a]), "C:\\x\\a.jpg");
        assert_eq!(copy_path_text(&[]), "");
    }

    #[test]
    fn terminal_dirs_cases() {
        let base = std::env::temp_dir().join(format!("fc-term-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let mut dirs = Vec::new();
        for i in 0..7 {
            let d = base.join(format!("d{i}"));
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("f.txt"), b"x").unwrap();
            dirs.push(d);
        }
        // A file -> its parent; a folder -> itself; duplicates removed (order kept).
        let got = terminal_dirs(&[
            dirs[0].join("f.txt"),
            dirs[0].clone(),
            dirs[1].clone(),
            dirs[1].join("f.txt"),
        ]);
        assert_eq!(got, vec![dirs[0].clone(), dirs[1].clone()]);
        // Capped at 5.
        let many: Vec<PathBuf> = dirs.iter().map(|d| d.join("f.txt")).collect();
        assert_eq!(terminal_dirs(&many).len(), 5);
        let _ = std::fs::remove_dir_all(&base);
    }

    fn clip(json: serde_json::Value) -> fc_core::clip::ClipItem {
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn drag_plan_cases() {
        let img = clip(serde_json::json!({
            "id": 1, "kind": "image", "image": "C:/c/1.png", "hash": "h", "createdAt": 1
        }));
        assert_eq!(drag_plan(&img), DragPlan::File(PathBuf::from("C:/c/1.png")));
        let edited = clip(serde_json::json!({
            "id": 1, "kind": "image", "image": "C:/c/1.png", "edited": "C:/c/1.edit.png",
            "hash": "h", "createdAt": 1
        }));
        assert_eq!(
            drag_plan(&edited),
            DragPlan::File(PathBuf::from("C:/c/1.edit.png"))
        );
        for kind in ["text", "link"] {
            let t = clip(serde_json::json!({
                "id": 2, "kind": kind, "text": "hello", "hash": "h", "createdAt": 1
            }));
            assert_eq!(drag_plan(&t), DragPlan::Text("hello".into()));
        }
        let empty = clip(serde_json::json!({
            "id": 3, "kind": "text", "text": "", "hash": "h", "createdAt": 1
        }));
        assert_eq!(drag_plan(&empty), DragPlan::Nothing);
        let broken = clip(serde_json::json!({
            "id": 4, "kind": "image", "hash": "h", "createdAt": 1
        }));
        assert_eq!(drag_plan(&broken), DragPlan::Nothing);
    }

    #[test]
    fn only_web_links_open() {
        assert!(is_web_url("https://example.com/a?b=c"));
        assert!(is_web_url("HTTP://example.com"));
        assert!(!is_web_url("file:///C:/Windows/System32/cmd.exe"));
        assert!(!is_web_url("javascript:alert(1)"));
        assert!(!is_web_url("ms-settings:"));
        assert!(!is_web_url(r"C:\Windows\notepad.exe"));
        assert!(!is_web_url("https://a.com b"));
        assert!(!is_web_url(""));
    }
}

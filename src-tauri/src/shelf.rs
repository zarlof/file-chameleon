//! Clipboard shelf window (`shelf.html`, label `shelf`). OWNER: Task 9.
//!
//! Created on first use, then only hidden and shown again (fast reopen, no cost before the first
//! use). It opens at the top centre of the work area under the cursor. Before showing, the
//! foreground window is remembered so `shelf_paste` can paste into it.
//!
//! Like the main window, creating the webview must not happen on the main thread inside a
//! WebView2 callback (see the `ipc` module docs): callers are the hotkey action thread, async
//! commands and the tray (a plain event-loop callback).

use crate::events;
use crate::state::AppState;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

pub const SHELF: &str = "shelf";

/// Logical metrics: maximum width, free space left and right, height of search bar + padding.
const MAX_WIDTH: f64 = 1180.0;
const SIDE_SPACE: f64 = 48.0;
const CHROME_HEIGHT: f64 = 96.0;

/// Logical height of one row for a card size (unknown sizes count as medium).
fn row_height(card: &str) -> f64 {
    match card {
        "small" => 112.0,
        "large" => 196.0,
        _ => 148.0,
    }
}

/// Physical `(x, y, width, height)` of the shelf in the work area `(left, top, right, bottom)`
/// (physical pixels) of a monitor with DPI `scale`: width `min(1180, work − 48)` logical,
/// height `rows × row + 96` logical (at least one row), centred at the top, never larger than
/// the work area.
pub fn shelf_rect(
    work: (i32, i32, i32, i32),
    scale: f64,
    rows: usize,
    card: &str,
) -> (i32, i32, i32, i32) {
    let (left, top, right, bottom) = work;
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let (work_w, work_h) = ((right - left).max(1), (bottom - top).max(1));
    let width = MAX_WIDTH.min(f64::from(work_w) / scale - SIDE_SPACE);
    let width = ((width * scale).round() as i32).clamp(1, work_w);
    let height = rows.max(1) as f64 * row_height(card) + CHROME_HEIGHT;
    let height = ((height * scale).round() as i32).clamp(1, work_h);
    (left + (work_w - width) / 2, top, width, height)
}

const VK_CONTROL: u16 = 0x11;
const VK_V: u16 = 0x56;
/// Left/right Shift, left/right Alt, left/right Win: still held, they turn our Ctrl+V into
/// Ctrl+Shift+V, Ctrl+Alt+V (our own shelf hotkey, Paste Special in Word) or a Win shortcut.
const PASTE_BLOCKERS: [u16; 6] = [0xA0, 0xA1, 0xA4, 0xA5, 0x5B, 0x5C];
/// An unassigned virtual key. Pressed before a lone Alt or Win key-up so the target app does not
/// open its menu bar (or Windows the Start menu).
const VK_MASK: u16 = 0xE8;
/// How long the user gets to let go of Shift/Alt/Win before their key-ups are injected.
const RELEASE_WAIT: std::time::Duration = std::time::Duration::from_millis(300);

fn is_alt_or_win(vk: u16) -> bool {
    matches!(vk, 0xA4 | 0xA5 | 0x5B | 0x5C)
}

/// The masking key press, needed when Alt or Win is among `held`.
fn mask_strokes(held: &[u16]) -> Vec<(u16, bool)> {
    if held.iter().any(|&k| is_alt_or_win(k)) {
        vec![(VK_MASK, false), (VK_MASK, true)]
    } else {
        Vec::new()
    }
}

/// Keystrokes `(virtual key, key up)` for the paste into the previous app: key-ups for the paste
/// blockers in `held` (after a masking key when Alt or Win is among them), then Ctrl+V. When the
/// user still holds Ctrl (Ctrl+Enter in the shelf), Ctrl is left alone and only V is typed.
fn paste_sequence(held: &[u16], ctrl_held: bool) -> Vec<(u16, bool)> {
    let held: Vec<u16> = held
        .iter()
        .copied()
        .filter(|k| PASTE_BLOCKERS.contains(k))
        .collect();
    let mut out = mask_strokes(&held);
    out.extend(held.iter().map(|&k| (k, true)));
    if !ctrl_held {
        out.push((VK_CONTROL, false));
    }
    out.extend([(VK_V, false), (VK_V, true)]);
    if !ctrl_held {
        out.push((VK_CONTROL, true));
    }
    out
}

fn create(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let state = app.state::<AppState>();
    WebviewWindowBuilder::new(app, SHELF, WebviewUrl::App("shelf.html".into()))
        .title("File Chameleon Shelf")
        .inner_size(MAX_WIDTH, 3.0 * row_height("medium") + CHROME_HEIGHT)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .shadow(false)
        .visible(false)
        .data_directory(state.paths.webview_dir())
        .additional_browser_args(&crate::windows::browser_args())
        .build()
}

fn hwnd_of(w: &WebviewWindow) -> Option<isize> {
    #[cfg(windows)]
    return w.hwnd().ok().map(|h| h.0 as isize);
    #[cfg(not(windows))]
    {
        let _ = w;
        None
    }
}

/// Size and place the shelf at the top centre of the work area under the cursor.
fn place(app: &AppHandle, w: &WebviewWindow) {
    #[cfg(windows)]
    {
        let (cx, cy) = crate::platform::win::cursor_pos();
        let Some(work) = crate::platform::win::work_area_at(cx, cy) else {
            return;
        };
        let scale = app
            .monitor_from_point(f64::from(cx), f64::from(cy))
            .ok()
            .flatten()
            .map(|m| m.scale_factor())
            .unwrap_or(1.0);
        let s = app.state::<AppState>().settings();
        let rows = s
            .shelf_rows
            .iter()
            .filter(|r| matches!(r.as_str(), "images" | "texts" | "links"))
            .count();
        let (x, y, width, height) = shelf_rect(work, scale, rows, &s.shelf_card_size);
        // Position first: moving to a monitor with another DPI may resize the window.
        let _ = w.set_position(PhysicalPosition::new(x, y));
        let _ = w.set_size(PhysicalSize::new(width as u32, height as u32));
    }
    #[cfg(not(windows))]
    let _ = (app, w);
}

/// Shell chrome that takes focus when the shelf is opened from the tray (the taskbar, the
/// hidden-icons flyout): pasting "back" into it is meaningless, so it counts as no window.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn is_taskbar_class(class: &str) -> bool {
    matches!(
        class,
        "Shell_TrayWnd"
            | "Shell_SecondaryTrayWnd"
            | "NotifyIconOverflowWindow"
            | "TopLevelWindowForOverflowXamlIsland"
    )
}

/// Show the shelf (creating it on first use) and tell the page (`shelf://open`). On the very
/// first show the page may not be listening yet; it loads its data on mount anyway.
pub fn show_shelf(app: &AppHandle) {
    let existing = app.get_webview_window(SHELF);
    #[cfg(windows)]
    {
        // No foreground window, or one of ours (the shelf, the main window): nothing to paste
        // into later, so `shelf_paste` just copies.
        let fg = crate::platform::win::foreground_window();
        let prev = if fg == 0
            || crate::platform::win::is_own_window(fg)
            || is_taskbar_class(&crate::platform::win::window_class(fg))
        {
            0
        } else {
            fg
        };
        app.state::<AppState>()
            .shelf_prev
            .store(prev, std::sync::atomic::Ordering::SeqCst);
    }
    let w = match existing {
        Some(w) => w,
        None => match create(app) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("shelf window: {e}");
                return;
            }
        },
    };
    place(app, &w);
    let _ = w.show();
    let _ = w.set_focus();
    let _ = app.emit_to(SHELF, events::SHELF_OPEN, ());
}

/// Hide the shelf (the window and its page stay alive). Always through Tauri, never a raw
/// `ShowWindow`: tao keeps its own visibility flag, and a window hidden behind its back is never
/// shown again by `show()`.
pub fn hide(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(SHELF) {
        let _ = w.hide();
    }
}

/// The shelf hotkey: hide the shelf when it is in front, show it otherwise.
pub fn toggle(app: &AppHandle) {
    let in_front = app.get_webview_window(SHELF).is_some_and(|w| {
        #[cfg(windows)]
        let focused = hwnd_of(&w) == Some(crate::platform::win::foreground_window());
        #[cfg(not(windows))]
        let focused = w.is_focused().unwrap_or(false);
        w.is_visible().unwrap_or(false) && focused
    });
    if in_front {
        hide(app);
    } else {
        show_shelf(app);
    }
}

/// After the items were copied: hide the shelf, bring back the app it was opened from and send
/// Ctrl+V. If that app is gone, the items just stay on the clipboard. Our process still owns the
/// last input (the key or click in the shelf), so Windows lets it hand the foreground back.
/// Shift/Alt/Win still held (e.g. Ctrl+Alt kept down after Ctrl+Alt+V, then Enter) get up to
/// 300 ms to be released; then their key-ups are injected, so the target sees a plain Ctrl+V.
pub fn paste_into_previous(app: &AppHandle) {
    hide(app);
    let prev = app
        .state::<AppState>()
        .shelf_prev
        .load(std::sync::atomic::Ordering::SeqCst);
    paste_keys_into(prev);
}

/// Bring `prev` (an HWND, 0 = none) to the front and type Ctrl+V into it. Shared by the shelf and
/// the clothesline; the caller hides its own window first.
pub fn paste_keys_into(prev: isize) {
    #[cfg(not(windows))]
    let _ = prev;
    #[cfg(windows)]
    {
        use crate::platform::win;
        if prev != 0 && win::is_window(prev) && win::set_foreground(prev) {
            std::thread::sleep(std::time::Duration::from_millis(60));
            // Mask right away: Alt or Win released by hand while we wait must not open a menu.
            win::send_keys(&mask_strokes(&win::keys_down(&PASTE_BLOCKERS)));
            let deadline = std::time::Instant::now() + RELEASE_WAIT;
            let mut held = win::keys_down(&PASTE_BLOCKERS);
            while !held.is_empty() && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(15));
                held = win::keys_down(&PASTE_BLOCKERS);
            }
            let ctrl_held = !win::keys_down(&[VK_CONTROL]).is_empty();
            win::send_keys(&paste_sequence(&held, ctrl_held));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{is_taskbar_class, paste_sequence, shelf_rect};

    #[test]
    fn taskbar_windows_are_not_a_paste_target() {
        for c in [
            "Shell_TrayWnd",
            "NotifyIconOverflowWindow",
            "TopLevelWindowForOverflowXamlIsland",
        ] {
            assert!(is_taskbar_class(c), "{c}");
        }
        assert!(!is_taskbar_class("Notepad"));
        assert!(!is_taskbar_class("Chrome_WidgetWin_1"));
    }

    #[test]
    fn paste_releases_held_modifiers_first() {
        const MASK: u16 = 0xE8;
        let ctrl_v = |held_ctrl: bool| {
            if held_ctrl {
                vec![(0x56, false), (0x56, true)]
            } else {
                vec![(0x11, false), (0x56, false), (0x56, true), (0x11, true)]
            }
        };
        // Nothing held: plain Ctrl+V.
        assert_eq!(paste_sequence(&[], false), ctrl_v(false));
        // Ctrl still held (Ctrl+Enter in the shelf): only V, Ctrl stays as the user holds it.
        assert_eq!(paste_sequence(&[], true), ctrl_v(true));
        // Shift held: released first, no mask needed.
        let mut want = vec![(0xA0, true)];
        want.extend(ctrl_v(true));
        assert_eq!(paste_sequence(&[0xA0], true), want);
        // Ctrl+Alt held after Ctrl+Alt+V, Enter: a masking key, then Alt up, then V.
        let mut want = vec![(MASK, false), (MASK, true), (0xA4, true)];
        want.extend(ctrl_v(true));
        assert_eq!(paste_sequence(&[0xA4], true), want);
        // Right Win + right Shift: masked (Start menu), both released.
        let mut want = vec![(MASK, false), (MASK, true), (0xA1, true), (0x5C, true)];
        want.extend(ctrl_v(false));
        assert_eq!(paste_sequence(&[0xA1, 0x5C], false), want);
        // Keys that are not paste blockers are ignored.
        assert_eq!(paste_sequence(&[0x41], false), ctrl_v(false));
    }

    #[test]
    fn shelf_rect_is_top_centre_and_clamped() {
        // 1080p at 100 %: full width cap, three medium rows, centred at the top of the work area.
        assert_eq!(
            shelf_rect((0, 0, 1920, 1040), 1.0, 3, "medium"),
            (370, 0, 1180, 3 * 148 + 96)
        );
        // Card sizes; an unknown size is medium; zero rows still shows one.
        assert_eq!(shelf_rect((0, 0, 1920, 1040), 1.0, 1, "small").3, 112 + 96);
        assert_eq!(
            shelf_rect((0, 0, 1920, 1040), 1.0, 2, "large").3,
            2 * 196 + 96
        );
        assert_eq!(shelf_rect((0, 0, 1920, 1040), 1.0, 1, "huge").3, 148 + 96);
        assert_eq!(shelf_rect((0, 0, 1920, 1040), 1.0, 0, "medium").3, 148 + 96);
        // 150 % on a second monitor to the right, below a top taskbar: logical sizes scale up.
        assert_eq!(
            shelf_rect((1920, 40, 3840, 1080), 1.5, 2, "small"),
            (
                1920 + (1920 - 1770) / 2,
                40,
                1770,
                ((2 * 112 + 96) as f64 * 1.5) as i32
            )
        );
        // Narrow and short work area: 24 px logical space each side, height clamped.
        assert_eq!(
            shelf_rect((0, 0, 1000, 400), 1.0, 3, "large"),
            (24, 0, 952, 400)
        );
        assert_eq!(
            shelf_rect((-1280, 0, 0, 1024), 2.0, 3, "medium"),
            (-1280 + 48, 0, 1280 - 96, 1024)
        );
    }
}

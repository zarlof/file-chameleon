//! Global low-level mouse and keyboard hooks: "Ctrl + drag" asks for the wheel, and while the
//! wheel is open the mouse wheel / Tab flip it and Esc (or, in click mode, a click outside it)
//! closes it. The overlay never takes focus, so these keys can only come from here.
//!
//! The callbacks only do atomic bookkeeping and a channel send (Windows silently removes LL
//! hooks that block ~300 ms) and always call `CallNextHookEx`. While the wheel is not open they
//! are a pure pass-through; input is swallowed only in click mode (Tab, Esc). Filtering (which
//! app the drag started in) happens on the consumer thread.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering::Relaxed};
use std::sync::mpsc::{Sender, channel};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_ESCAPE, VK_SHIFT, VK_TAB,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, GetSystemMetrics, KBDLLHOOKSTRUCT, MSG,
    MSLLHOOKSTRUCT, SM_CXDRAG, SM_CYDRAG, SetWindowsHookExW, TranslateMessage, WH_KEYBOARD_LL,
    WH_MOUSE_LL, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL,
    WM_RBUTTONDOWN,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookEvent {
    /// Ctrl held while dragging past the system drag threshold. `down_*` = where the press began.
    DragStart {
        x: i32,
        y: i32,
        down_x: i32,
        down_y: i32,
        /// Shift was held too (Ctrl+Shift+drag opens the back face).
        shift: bool,
    },
    /// Left button released after a `DragStart`.
    Released,
    /// Wheel open: flip front <-> back (mouse wheel or Tab).
    Flip,
    /// Wheel open: close it (Esc, or a click outside it in click mode).
    Close,
}

/// Flips closer together than this are one (the overlay page uses the same gap).
const FLIP_GAP_MS: u32 = 250;

static ENABLED: AtomicBool = AtomicBool::new(false);
static ANY_APP: AtomicBool = AtomicBool::new(false);
static DOWN: AtomicBool = AtomicBool::new(false);
static TRIGGERED: AtomicBool = AtomicBool::new(false);
static DOWN_X: AtomicI32 = AtomicI32::new(0);
static DOWN_Y: AtomicI32 = AtomicI32::new(0);
static TH_X: AtomicI32 = AtomicI32::new(4);
static TH_Y: AtomicI32 = AtomicI32::new(4);
static SENDER: OnceLock<Sender<HookEvent>> = OnceLock::new();

// Wheel state, written by `windows.rs` through `set_wheel_open`.
static WHEEL_OPEN: AtomicBool = AtomicBool::new(false);
static CLICK_MODE: AtomicBool = AtomicBool::new(false);
static RECT_L: AtomicI32 = AtomicI32::new(0);
static RECT_T: AtomicI32 = AtomicI32::new(0);
static RECT_R: AtomicI32 = AtomicI32::new(0);
static RECT_B: AtomicI32 = AtomicI32::new(0);
static FLIP_SCROLL: AtomicBool = AtomicBool::new(true);
static FLIP_TAB: AtomicBool = AtomicBool::new(true);
/// Message time (ms) of the last flip sent; 0 = none since the wheel opened.
static LAST_FLIP: AtomicU32 = AtomicU32::new(0);
/// Tab is held down (key repeat must not flip again and again).
static TAB_HELD: AtomicBool = AtomicBool::new(false);

pub fn set_config(enabled: bool, any_app: bool) {
    ENABLED.store(enabled, Relaxed);
    ANY_APP.store(any_app, Relaxed);
}

/// Settings: flip with the mouse wheel / with Tab while the wheel is open.
pub fn set_flip(scroll: bool, tab: bool) {
    FLIP_SCROLL.store(scroll, Relaxed);
    FLIP_TAB.store(tab, Relaxed);
}

/// The overlay shows (or hid) the wheel. `rect` = (left, top, right, bottom) of the overlay
/// window in physical screen pixels; in click mode a press outside it closes the wheel.
pub fn set_wheel_open(open: bool, click: bool, rect: (i32, i32, i32, i32)) {
    RECT_L.store(rect.0, Relaxed);
    RECT_T.store(rect.1, Relaxed);
    RECT_R.store(rect.2, Relaxed);
    RECT_B.store(rect.3, Relaxed);
    LAST_FLIP.store(0, Relaxed);
    TAB_HELD.store(false, Relaxed);
    CLICK_MODE.store(click, Relaxed);
    WHEEL_OPEN.store(open, Relaxed);
}

pub fn any_app() -> bool {
    ANY_APP.load(Relaxed)
}

fn send(ev: HookEvent) {
    if let Some(tx) = SENDER.get() {
        let _ = tx.send(ev);
    }
}

/// Send `Flip` unless one was sent less than `FLIP_GAP_MS` ago (`time` = message time in ms).
fn flip(time: u32) {
    let last = LAST_FLIP.load(Relaxed);
    if last == 0 || time.wrapping_sub(last) >= FLIP_GAP_MS {
        LAST_FLIP.store(time.max(1), Relaxed);
        send(HookEvent::Flip);
    }
}

/// What a key press does: (event to send, swallow the key). Only Tab (when flipping with Tab
/// is on) and Esc do anything, and only while the wheel is open. They are swallowed only in
/// click mode: during a drag the source app must still see Esc to cancel the drag.
fn key_action(open: bool, click: bool, vk: u32, flip_tab: bool) -> (Option<HookEvent>, bool) {
    if !open {
        return (None, false);
    }
    match vk {
        v if v == u32::from(VK_TAB.0) && flip_tab => (Some(HookEvent::Flip), click),
        v if v == u32::from(VK_ESCAPE.0) => (Some(HookEvent::Close), click),
        _ => (None, false),
    }
}

/// What a mouse message does while the wheel is open (`None` when closed): flip on the mouse
/// wheel (when enabled), close on a press outside the wheel window in click mode.
fn mouse_action(
    open: bool,
    click: bool,
    msg: u32,
    inside: bool,
    flip_scroll: bool,
) -> Option<HookEvent> {
    if !open {
        return None;
    }
    match msg {
        WM_MOUSEWHEEL if flip_scroll => Some(HookEvent::Flip),
        WM_LBUTTONDOWN | WM_RBUTTONDOWN if click && !inside => Some(HookEvent::Close),
        _ => None,
    }
}

fn inside_rect(x: i32, y: i32) -> bool {
    x >= RECT_L.load(Relaxed)
        && x < RECT_R.load(Relaxed)
        && y >= RECT_T.load(Relaxed)
        && y < RECT_B.load(Relaxed)
}

fn key_held(vk: u16) -> bool {
    unsafe { GetAsyncKeyState(vk as i32) as u16 & 0x8000 != 0 }
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        let (x, y) = (info.pt.x, info.pt.y);
        let msg = wparam.0 as u32;
        if WHEEL_OPEN.load(Relaxed) {
            match mouse_action(
                true,
                CLICK_MODE.load(Relaxed),
                msg,
                inside_rect(x, y),
                FLIP_SCROLL.load(Relaxed),
            ) {
                Some(HookEvent::Flip) => flip(info.time),
                Some(ev) => send(ev),
                None => {}
            }
        }
        if ENABLED.load(Relaxed) {
            match msg {
                WM_LBUTTONDOWN => {
                    DOWN.store(true, Relaxed);
                    TRIGGERED.store(false, Relaxed);
                    DOWN_X.store(x, Relaxed);
                    DOWN_Y.store(y, Relaxed);
                }
                WM_MOUSEMOVE if DOWN.load(Relaxed) && !TRIGGERED.load(Relaxed) => {
                    let (dx, dy) = (
                        (x - DOWN_X.load(Relaxed)).abs(),
                        (y - DOWN_Y.load(Relaxed)).abs(),
                    );
                    // Use a slightly larger threshold than the system's so clicks never trigger.
                    if (dx > TH_X.load(Relaxed) * 2 || dy > TH_Y.load(Relaxed) * 2)
                        && key_held(VK_CONTROL.0)
                    {
                        TRIGGERED.store(true, Relaxed);
                        send(HookEvent::DragStart {
                            x,
                            y,
                            down_x: DOWN_X.load(Relaxed),
                            down_y: DOWN_Y.load(Relaxed),
                            shift: key_held(VK_SHIFT.0),
                        });
                    }
                }
                WM_LBUTTONUP => {
                    if TRIGGERED.swap(false, Relaxed) {
                        send(HookEvent::Released);
                    }
                    DOWN.store(false, Relaxed);
                }
                _ => {}
            }
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

unsafe extern "system" fn key_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let mut swallow = false;
    if code >= 0 && WHEEL_OPEN.load(Relaxed) {
        let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        // WM_SYSKEYDOWN (Alt held: Alt+Tab, Alt+Esc) is never touched.
        match wparam.0 as u32 {
            WM_KEYDOWN => {
                let (ev, sw) = key_action(
                    true,
                    CLICK_MODE.load(Relaxed),
                    info.vkCode,
                    FLIP_TAB.load(Relaxed),
                );
                swallow = sw;
                match ev {
                    // Key repeat while Tab is held does not flip again.
                    Some(HookEvent::Flip) => {
                        if !TAB_HELD.swap(true, Relaxed) {
                            flip(info.time);
                        }
                    }
                    Some(ev) => send(ev),
                    None => {}
                }
            }
            WM_KEYUP if info.vkCode == u32::from(VK_TAB.0) => TAB_HELD.store(false, Relaxed),
            _ => {}
        }
    }
    let next = unsafe { CallNextHookEx(None, code, wparam, lparam) };
    if swallow { LRESULT(1) } else { next }
}

/// Install the hooks once (idempotent). `on_event` runs on a dedicated consumer thread.
pub fn ensure_started(on_event: impl Fn(HookEvent) + Send + 'static) {
    static STARTED: AtomicBool = AtomicBool::new(false);
    if STARTED.swap(true, Relaxed) {
        return;
    }
    let (tx, rx) = channel::<HookEvent>();
    let _ = SENDER.set(tx);
    std::thread::Builder::new()
        .name("fc-input-hooks".into())
        .spawn(|| unsafe {
            TH_X.store(GetSystemMetrics(SM_CXDRAG).max(1), Relaxed);
            TH_Y.store(GetSystemMetrics(SM_CYDRAG).max(1), Relaxed);
            let module = GetModuleHandleW(None).ok().map(|m| m.into());
            let Ok(_mouse) = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), module, 0) else {
                eprintln!("mouse hook: install failed");
                return;
            };
            // Without it the wheel still works; only Tab / Esc do nothing.
            if SetWindowsHookExW(WH_KEYBOARD_LL, Some(key_proc), module, 0).is_err() {
                eprintln!("keyboard hook: install failed");
            }
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        })
        .expect("spawn hook thread");
    std::thread::Builder::new()
        .name("fc-input-events".into())
        .spawn(move || {
            for ev in rx {
                on_event(ev);
            }
        })
        .expect("spawn hook consumer");
}

/// Should a drag that started at this point open the wheel?
/// Default: only Explorer windows and the desktop. Never our own windows, never `excluded`
/// apps (lowercase exe names).
pub fn source_allowed(down_x: i32, down_y: i32, excluded: &[String]) -> bool {
    let at = super::win::process_at(down_x, down_y);
    let own = at
        .as_ref()
        .is_some_and(|(_, pid)| *pid == super::win::current_pid());
    allowed(
        at.as_ref().map(|(n, _)| n.as_str()),
        own,
        any_app(),
        excluded,
    )
}

/// `exe` = lowercase exe name of the window under the press (None = unknown).
fn allowed(exe: Option<&str>, own: bool, any_app: bool, excluded: &[String]) -> bool {
    if own {
        return false;
    }
    match exe {
        Some(name) if excluded.iter().any(|e| e.trim().eq_ignore_ascii_case(name)) => false,
        Some(name) => any_app || name == "explorer.exe",
        None => any_app,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TAB: u32 = VK_TAB.0 as u32;
    const ESC: u32 = VK_ESCAPE.0 as u32;
    const A: u32 = 0x41;

    #[test]
    fn flip_keys_only_when_wheel_open() {
        // Closed: everything passes through untouched, whatever the mode.
        for click in [false, true] {
            for vk in [TAB, ESC, A] {
                assert_eq!(key_action(false, click, vk, true), (None, false));
            }
        }
        // Open during a drag: act, but never swallow (Esc must still cancel the drag).
        assert_eq!(
            key_action(true, false, TAB, true),
            (Some(HookEvent::Flip), false)
        );
        assert_eq!(
            key_action(true, false, ESC, true),
            (Some(HookEvent::Close), false)
        );
        // Open in click mode: swallowed (the app under the wheel must not see them).
        assert_eq!(
            key_action(true, true, TAB, true),
            (Some(HookEvent::Flip), true)
        );
        assert_eq!(
            key_action(true, true, ESC, true),
            (Some(HookEvent::Close), true)
        );
        // Flipping with Tab turned off: Tab is ignored and passes through.
        assert_eq!(key_action(true, false, TAB, false), (None, false));
        assert_eq!(key_action(true, true, TAB, false), (None, false));
        assert_eq!(
            key_action(true, true, ESC, false),
            (Some(HookEvent::Close), true)
        );
        // Other keys are never touched.
        assert_eq!(key_action(true, true, A, true), (None, false));
    }

    #[test]
    fn mouse_flips_and_outside_clicks_only_when_wheel_open() {
        for msg in [WM_MOUSEWHEEL, WM_LBUTTONDOWN, WM_RBUTTONDOWN, WM_MOUSEMOVE] {
            assert_eq!(mouse_action(false, true, msg, false, true), None);
        }
        assert_eq!(
            mouse_action(true, false, WM_MOUSEWHEEL, true, true),
            Some(HookEvent::Flip)
        );
        assert_eq!(mouse_action(true, false, WM_MOUSEWHEEL, true, false), None);
        // A press outside closes only in click mode; inside, the page handles it.
        assert_eq!(
            mouse_action(true, true, WM_LBUTTONDOWN, false, true),
            Some(HookEvent::Close)
        );
        assert_eq!(
            mouse_action(true, true, WM_RBUTTONDOWN, false, true),
            Some(HookEvent::Close)
        );
        assert_eq!(mouse_action(true, true, WM_LBUTTONDOWN, true, true), None);
        assert_eq!(mouse_action(true, false, WM_LBUTTONDOWN, false, true), None);
    }

    #[test]
    fn excluded_apps_never_open_the_wheel() {
        let excluded = vec!["photoshop.exe".to_string(), " Explorer.EXE ".to_string()];
        assert!(!allowed(Some("explorer.exe"), false, false, &excluded));
        assert!(!allowed(Some("photoshop.exe"), false, true, &excluded));
        assert!(allowed(Some("explorer.exe"), false, false, &[]));
        assert!(!allowed(Some("code.exe"), false, false, &excluded));
        assert!(allowed(Some("code.exe"), false, true, &excluded));
        // Our own windows never, unknown windows only with "any app".
        assert!(!allowed(Some("file-chameleon.exe"), true, true, &[]));
        assert!(allowed(None, false, true, &[]));
        assert!(!allowed(None, false, false, &[]));
    }
}

//! Global low-level mouse and keyboard hooks: "Ctrl + drag" asks for the wheel, and while the
//! wheel is open the mouse wheel / Tab flip it and Esc (or, in click mode, a click outside it)
//! closes it. The overlay never takes focus, so these keys can only come from here.
//!
//! The callbacks only do atomic bookkeeping and a channel send (Windows silently removes LL
//! hooks that block ~300 ms) and always call `CallNextHookEx`. While the wheel is not open they
//! are a pure pass-through; input is swallowed only in click mode (plain Tab, Esc). A foreground
//! event hook closes the click-mode wheel when the user switches to another window (Alt+Tab,
//! Win+number). Filtering (which app the drag started in) happens on the consumer thread.

use std::sync::OnceLock;
use std::sync::atomic::{
    AtomicBool, AtomicI32, AtomicIsize, AtomicU32, AtomicUsize, Ordering::Relaxed,
};
use std::sync::mpsc::{Sender, channel};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_ESCAPE, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT, VK_TAB,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, EVENT_SYSTEM_FOREGROUND, GetForegroundWindow, GetMessageW,
    GetSystemMetrics, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, SM_CXDRAG, SM_CYDRAG,
    SetWindowsHookExW, TranslateMessage, WH_KEYBOARD_LL, WH_MOUSE_LL, WINEVENT_OUTOFCONTEXT,
    WINEVENT_SKIPOWNPROCESS, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
    WM_MOUSEWHEEL, WM_RBUTTONDOWN,
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
    /// The pointer reached the clothesline's reveal zone (top edge, middle of a monitor).
    EdgeEnter { x: i32, y: i32 },
    /// The pointer left the reveal zone.
    EdgeLeave,
    /// The monitor layout changed: refresh the cached rectangles (done off the hook thread).
    Displays,
}

/// Height of the reveal zone at the top (or bottom) of a monitor, in physical pixels.
const EDGE_PX: i32 = 2;
/// The zone spans the middle 60 % of the monitor's width (20 % to 80 %).
const EDGE_FROM: i32 = 20;
const EDGE_TO: i32 = 80;
/// Cached monitors the hook looks at (more are ignored).
const MAX_MONITORS: usize = 8;

/// Is the point in the reveal zone of this monitor `(left, top, right, bottom)`: its top two
/// rows, or its bottom two when `bottom`?
fn edge_zone_in(x: i32, y: i32, (l, t, r, b): (i32, i32, i32, i32), bottom: bool) -> bool {
    let w = r - l;
    let rows = if bottom {
        y >= b - EDGE_PX && y < b
    } else {
        y >= t && y < t + EDGE_PX
    };
    rows && x >= l + w * EDGE_FROM / 100 && x < l + w * EDGE_TO / 100
}

/// Is the point in the reveal zone of any of the monitors?
#[cfg(test)]
fn edge_zone(x: i32, y: i32, monitors: &[(i32, i32, i32, i32)], bottom: bool) -> bool {
    monitors.iter().any(|&m| edge_zone_in(x, y, m, bottom))
}

/// One mouse-wheel gesture flips once: after a flip, wheel events are ignored until this long
/// passes without any (the overlay page uses the same rule).
const FLIP_GAP_MS: u32 = 250;

/// Modifiers held with a key.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Mods {
    ctrl: bool,
    alt: bool,
    win: bool,
}

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
/// Message time (ms) of the last mouse-wheel event while open; 0 = none since the wheel opened.
static LAST_WHEEL: AtomicU32 = AtomicU32::new(0);
/// Tab is held down (key repeat must not flip again and again).
static TAB_HELD: AtomicBool = AtomicBool::new(false);
/// Foreground window (HWND as isize) when the wheel opened; another one closes a click wheel.
static OPEN_FG: AtomicIsize = AtomicIsize::new(0);

// Clothesline edge reveal: its own flag, the cached monitor rectangles (atomics, so the hook
// never calls a monitor API) and the layout fingerprint they were read for.
static LINE_EDGE: AtomicBool = AtomicBool::new(false);
/// The reveal zone is at the bottom of the monitors instead of the top.
static LINE_EDGE_BOTTOM: AtomicBool = AtomicBool::new(false);
static IN_ZONE: AtomicBool = AtomicBool::new(false);
static MON_COUNT: AtomicUsize = AtomicUsize::new(0);
static MON_RECTS: [AtomicI32; MAX_MONITORS * 4] = [const { AtomicI32::new(0) }; MAX_MONITORS * 4];
static DISPLAY_SIG: [AtomicI32; 5] = [const { AtomicI32::new(0) }; 5];

/// Turn the edge-zone check on or off (the clothesline's "reveal at the edge"); `bottom` puts
/// the zone at the bottom edge instead of the top.
pub fn set_line_edge(on: bool, bottom: bool) {
    LINE_EDGE.store(on, Relaxed);
    LINE_EDGE_BOTTOM.store(bottom, Relaxed);
    IN_ZONE.store(false, Relaxed);
}

/// Replace the cached monitor rectangles `(left, top, right, bottom)` and the layout fingerprint.
pub fn set_monitors(rects: &[(i32, i32, i32, i32)], signature: [i32; 5]) {
    let n = rects.len().min(MAX_MONITORS);
    for (i, r) in rects.iter().take(n).enumerate() {
        MON_RECTS[i * 4].store(r.0, Relaxed);
        MON_RECTS[i * 4 + 1].store(r.1, Relaxed);
        MON_RECTS[i * 4 + 2].store(r.2, Relaxed);
        MON_RECTS[i * 4 + 3].store(r.3, Relaxed);
    }
    MON_COUNT.store(n, Relaxed);
    for (slot, v) in DISPLAY_SIG.iter().zip(signature) {
        slot.store(v, Relaxed);
    }
}

fn in_edge_zone(x: i32, y: i32) -> bool {
    let bottom = LINE_EDGE_BOTTOM.load(Relaxed);
    (0..MON_COUNT.load(Relaxed).min(MAX_MONITORS)).any(|i| {
        edge_zone_in(
            x,
            y,
            (
                MON_RECTS[i * 4].load(Relaxed),
                MON_RECTS[i * 4 + 1].load(Relaxed),
                MON_RECTS[i * 4 + 2].load(Relaxed),
                MON_RECTS[i * 4 + 3].load(Relaxed),
            ),
            bottom,
        )
    })
}

/// Is the pointer in the reveal zone right now (as of the last mouse move)?
pub fn pointer_in_zone() -> bool {
    LINE_EDGE.load(Relaxed) && IN_ZONE.load(Relaxed)
}

/// One mouse move while the edge reveal is on: atomics and a send only.
fn edge_move(x: i32, y: i32) {
    let sig = super::win::display_signature();
    if sig
        .iter()
        .zip(&DISPLAY_SIG)
        .any(|(now, cached)| *now != cached.load(Relaxed))
    {
        // Store it so the event is sent once; the consumer thread reads the monitors.
        for (slot, v) in DISPLAY_SIG.iter().zip(sig) {
            slot.store(v, Relaxed);
        }
        send(HookEvent::Displays);
    }
    let inside = in_edge_zone(x, y);
    if inside != IN_ZONE.swap(inside, Relaxed) {
        send(if inside {
            HookEvent::EdgeEnter { x, y }
        } else {
            HookEvent::EdgeLeave
        });
    }
}

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
    LAST_WHEEL.store(0, Relaxed);
    TAB_HELD.store(false, Relaxed);
    let fg = if open {
        unsafe { GetForegroundWindow() }.0 as isize
    } else {
        0
    };
    OPEN_FG.store(fg, Relaxed);
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

/// Leading edge of a mouse-wheel gesture: a wheel event at `now` flips when the previous one
/// (`last`, 0 = none) is at least `FLIP_GAP_MS` old. Every event refreshes `last`, so a long
/// scroll flips once. Times are message times in ms (wrapping).
fn gesture_flip(last: u32, now: u32) -> bool {
    last == 0 || now.wrapping_sub(last) >= FLIP_GAP_MS
}

/// Does a new foreground window close the wheel? Only a click-mode wheel, and only when the
/// window differs from the one that was in front when it opened (0 = none: ignored).
fn foreground_closes(open: bool, click: bool, new_fg: isize, open_fg: isize) -> bool {
    open && click && new_fg != 0 && new_fg != open_fg
}

/// What a key press does: (event to send, swallow the key). Only Tab (when flipping with Tab
/// is on) and Esc do anything, and only while the wheel is open. They are swallowed only in
/// click mode: during a drag the source app must still see Esc to cancel the drag. With Alt or
/// Win held (Win+Tab, Alt+Esc) nothing happens; with Ctrl only during a drag, where Ctrl is part
/// of the Ctrl+drag gesture and nothing is swallowed (click mode leaves Ctrl+Esc, Ctrl+Shift+Esc
/// and Ctrl+Tab to Windows and the apps).
fn key_action(
    open: bool,
    click: bool,
    vk: u32,
    flip_tab: bool,
    mods: Mods,
) -> (Option<HookEvent>, bool) {
    if !open || mods.alt || mods.win || (mods.ctrl && click) {
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
                Some(HookEvent::Flip) => {
                    let now = info.time.max(1);
                    if gesture_flip(LAST_WHEEL.swap(now, Relaxed), now) {
                        send(HookEvent::Flip);
                    }
                }
                Some(ev) => send(ev),
                None => {}
            }
        }
        if msg == WM_MOUSEMOVE && LINE_EDGE.load(Relaxed) {
            edge_move(x, y);
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
                let mods = Mods {
                    ctrl: key_held(VK_CONTROL.0),
                    alt: key_held(VK_MENU.0),
                    win: key_held(VK_LWIN.0) || key_held(VK_RWIN.0),
                };
                let (ev, sw) = key_action(
                    true,
                    CLICK_MODE.load(Relaxed),
                    info.vkCode,
                    FLIP_TAB.load(Relaxed),
                    mods,
                );
                swallow = sw;
                match ev {
                    // One flip per Tab press: key repeat while it is held does not flip again.
                    Some(HookEvent::Flip) => {
                        if !TAB_HELD.swap(true, Relaxed) {
                            send(HookEvent::Flip);
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

/// `EVENT_SYSTEM_FOREGROUND` (other processes only): the user switched to another window.
unsafe extern "system" fn foreground_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    _object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    if foreground_closes(
        WHEEL_OPEN.load(Relaxed),
        CLICK_MODE.load(Relaxed),
        hwnd.0 as isize,
        OPEN_FG.load(Relaxed),
    ) {
        send(HookEvent::Close);
    }
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
            // Out of context: delivered through this thread's message loop, only on a switch.
            let fg = SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                Some(foreground_proc),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            );
            if fg.is_invalid() {
                eprintln!("foreground hook: install failed");
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
    const NONE: Mods = Mods {
        ctrl: false,
        alt: false,
        win: false,
    };
    const CTRL: Mods = Mods { ctrl: true, ..NONE };
    const ALT: Mods = Mods { alt: true, ..NONE };
    const WIN: Mods = Mods { win: true, ..NONE };

    #[test]
    fn flip_keys_only_when_wheel_open() {
        // Closed: everything passes through untouched, whatever the mode or modifiers.
        for click in [false, true] {
            for vk in [TAB, ESC, A] {
                for mods in [NONE, CTRL, ALT, WIN] {
                    assert_eq!(key_action(false, click, vk, true, mods), (None, false));
                }
            }
        }
        // Open during a drag: act, but never swallow (Esc must still cancel the drag).
        assert_eq!(
            key_action(true, false, TAB, true, NONE),
            (Some(HookEvent::Flip), false)
        );
        assert_eq!(
            key_action(true, false, ESC, true, NONE),
            (Some(HookEvent::Close), false)
        );
        // Open in click mode: swallowed (the app under the wheel must not see them).
        assert_eq!(
            key_action(true, true, TAB, true, NONE),
            (Some(HookEvent::Flip), true)
        );
        assert_eq!(
            key_action(true, true, ESC, true, NONE),
            (Some(HookEvent::Close), true)
        );
        // Flipping with Tab turned off: Tab is ignored and passes through.
        assert_eq!(key_action(true, false, TAB, false, NONE), (None, false));
        assert_eq!(key_action(true, true, TAB, false, NONE), (None, false));
        assert_eq!(
            key_action(true, true, ESC, false, NONE),
            (Some(HookEvent::Close), true)
        );
        // Other keys are never touched.
        assert_eq!(key_action(true, true, A, true, NONE), (None, false));
    }

    #[test]
    fn shortcuts_with_modifiers_pass_through() {
        // Win+Tab, Alt+Esc, Win+Esc: never acted on or swallowed, in either mode.
        for click in [false, true] {
            for vk in [TAB, ESC] {
                for mods in [ALT, WIN, Mods { ctrl: true, ..WIN }] {
                    assert_eq!(key_action(true, click, vk, true, mods), (None, false));
                }
            }
        }
        // Click mode: Ctrl+Esc (Start), Ctrl+Shift+Esc (Task Manager), Ctrl+Tab pass through.
        assert_eq!(key_action(true, true, ESC, true, CTRL), (None, false));
        assert_eq!(key_action(true, true, TAB, true, CTRL), (None, false));
        // During a Ctrl+drag, Ctrl is held anyway: Tab flips and Esc closes (not swallowed).
        assert_eq!(
            key_action(true, false, TAB, true, CTRL),
            (Some(HookEvent::Flip), false)
        );
        assert_eq!(
            key_action(true, false, ESC, true, CTRL),
            (Some(HookEvent::Close), false)
        );
    }

    /// How many flips a run of wheel events at these message times gives.
    fn flips(times: &[u32]) -> usize {
        let mut last = 0;
        times
            .iter()
            .filter(|&&now| {
                let go = gesture_flip(last, now);
                last = now;
                go
            })
            .count()
    }

    #[test]
    fn one_flip_per_wheel_gesture() {
        assert_eq!(flips(&[1000]), 1);
        // A long scroll (events closer than 250 ms) is one gesture, however long it lasts.
        assert_eq!(flips(&[1000, 1100, 1200, 1300, 1400, 1500, 1600]), 1);
        // A quiet gap of 250 ms or more starts a new gesture.
        assert_eq!(flips(&[1000, 1100, 1350, 1400]), 2);
        assert_eq!(flips(&[1000, 1249]), 1);
        assert_eq!(flips(&[1000, 1250]), 2);
        // Message time wraps around (about every 49.7 days).
        assert_eq!(flips(&[u32::MAX - 100, 50]), 1);
        assert_eq!(flips(&[u32::MAX - 100, 200]), 2);
    }

    #[test]
    fn switching_windows_closes_only_the_click_wheel() {
        // Another window comes to the front (Alt+Tab, Win+1): the click wheel closes.
        assert!(foreground_closes(true, true, 200, 100));
        // The window it opened over (again), no window, a drag wheel or a closed wheel: no.
        assert!(!foreground_closes(true, true, 100, 100));
        assert!(!foreground_closes(true, true, 0, 100));
        assert!(!foreground_closes(true, false, 200, 100));
        assert!(!foreground_closes(false, true, 200, 100));
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
    fn edge_zone_bottom() {
        let m = [(0, 0, 1920, 1080)];
        assert!(edge_zone(960, 1079, &m, true));
        assert!(edge_zone(960, 1078, &m, true));
        assert!(!edge_zone(960, 1077, &m, true));
        assert!(!edge_zone(960, 0, &m, true));
        assert!(!edge_zone(960, 1079, &m, false));
        assert!(!edge_zone(100, 1079, &m, true));
        // A monitor arranged lower: its own bottom row.
        assert!(edge_zone(2880, 1279, &[(1920, 200, 3840, 1280)], true));
    }

    #[test]
    fn edge_zone_cases() {
        let m = [(0, 0, 1920, 1080)];
        assert!(edge_zone(960, 0, &m, false));
        assert!(edge_zone(960, 1, &m, false));
        assert!(!edge_zone(960, 3, &m, false));
        // Outside the middle 60 %.
        assert!(!edge_zone(100, 0, &m, false));
        assert!(!edge_zone(1800, 0, &m, false));
        // A second monitor to the right.
        let two = [(0, 0, 1920, 1080), (1920, 0, 3840, 1080)];
        assert!(edge_zone(2880, 0, &two, false));
        assert!(!edge_zone(1930, 0, &two, false));
        // A monitor whose top is not 0 (arranged lower).
        assert!(edge_zone(2880, 200, &[(1920, 200, 3840, 1280)], false));
        assert!(!edge_zone(2880, 0, &[(1920, 200, 3840, 1280)], false));
        assert!(!edge_zone(960, 0, &[], false));
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

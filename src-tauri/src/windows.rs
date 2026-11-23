//! Window management: the main window (created on demand, destroyed on close) and the
//! single always-alive overlay window used for the drag wheel and toasts.

use crate::events::{self, NavPayload, ToastPayload, WheelPayload, WheelPointer, WheelThumb};
use crate::state::AppState;
use fc_core::jobs::{JobSnapshot, JobState};
use serde::Serialize;
use serde_json::json;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{
    AppHandle, DragDropEvent, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    Window, WindowEvent,
};

pub const MAIN: &str = "main";
pub const OVERLAY: &str = "overlay";

/// Logical size of the wheel window at wheel size 100 % (`ringRadii(1).size` in
/// `src/lib/wheel/geometry.ts`: the wheel, its group fans and a shadow margin). The window is
/// `WHEEL_SIZE × wheelScale / 100` and the page draws the wheel centred in it.
pub const WHEEL_SIZE: f64 = 540.0;
/// Paths from several `--wheel` invocations (one per selected file) arriving within this time
/// open one wheel.
const WHEEL_ARGS_MS: u64 = 350;
/// Files asked for within this time after a click wheel opened join it (late `--wheel`
/// invocations of the same Explorer selection) instead of replacing it.
const WHEEL_JOIN: Duration = Duration::from_millis(1000);
/// Logical size of the toast window (card + shadow margin).
pub const TOAST_W: f64 = 404.0;
pub const TOAST_H: f64 = 150.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Hidden,
    Wheel,
    Toast,
}

/// A toast requested before the overlay page could show it: (kind, data, timeout).
type QueuedToast = (String, serde_json::Value, Option<u64>);

struct OverlayCtl {
    mode: Mode,
    /// Bumped on every show; delayed hides only fire if the generation is unchanged.
    generation: u64,
    /// Files entered the wheel during this generation.
    entered: bool,
    /// The overlay page has registered its listeners (`overlay_ready`).
    ready: bool,
    /// Latest toast sent before `ready`; shown once the page is ready.
    queued_toast: Option<QueuedToast>,
    pending_nav: Option<NavPayload>,
    /// The wheel is in click mode (opened for files, not by a drag).
    click: bool,
    /// The drag that opened the wheel started with Shift.
    shift: bool,
    /// Bumped on every wheel show; late slots / thumbnails of an older wheel are dropped.
    wheel_id: u64,
    /// Paths whose slots this wheel already got (a drag re-entering sends nothing new).
    slots_for: Option<Vec<String>>,
    /// Click-mode wheel requested before the page was ready; opened by `overlay_ready`.
    pending_wheel: Option<Vec<String>>,
    /// When the current click wheel opened (late files within `WHEEL_JOIN` join it).
    click_opened: Option<Instant>,
    /// The wheel window's rectangle (physical pixels: left, top, right, bottom).
    rect: (i32, i32, i32, i32),
}

static CTL: Mutex<OverlayCtl> = Mutex::new(OverlayCtl {
    mode: Mode::Hidden,
    generation: 0,
    entered: false,
    ready: false,
    queued_toast: None,
    pending_nav: None,
    click: false,
    shift: false,
    wheel_id: 0,
    slots_for: None,
    pending_wheel: None,
    click_opened: None,
    rect: (0, 0, 0, 0),
});

/// Append `more` to `into`, keeping the order and dropping paths already there.
fn merge_paths(into: &mut Vec<String>, more: Vec<String>) {
    for p in more {
        if !into.contains(&p) {
            into.push(p);
        }
    }
}

fn ctl() -> std::sync::MutexGuard<'static, OverlayCtl> {
    CTL.lock().unwrap_or_else(|e| e.into_inner())
}

// ------------------------------------------------------------------ main window

/// WebView2 flags shared by every webview. All webviews share one browser process, so the flags
/// must be identical for each of them. They are Tauri's defaults plus software rendering:
/// the UI is light enough for CPU raster, and dropping the GPU process roughly halves idle
/// memory (measured on the release build in the tray: ~160 MB -> ~97 MB in total).
/// `FC_WEBVIEW_ARGS` overrides them, which is useful when measuring memory or debugging rendering.
const BROWSER_ARGS: &str =
    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --disable-gpu --in-process-gpu";

pub(crate) fn browser_args() -> String {
    std::env::var("FC_WEBVIEW_ARGS").unwrap_or_else(|_| BROWSER_ARGS.to_string())
}

/// Show (or create) the main window. Never call this from a sync command or another WebView2
/// callback on the main thread: building the webview there deadlocks (see `ipc` module docs).
/// Tray, single-instance and setup callers are plain event-loop callbacks and are fine.
pub fn show_main(app: &AppHandle, nav: Option<NavPayload>) {
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        if let Some(nav) = nav {
            let _ = w.emit(events::NAV_GOTO, nav);
        }
        return;
    }
    ctl().pending_nav = nav;
    let state = app.state::<AppState>();
    let built = WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App("index.html".into()))
        .title("File Chameleon")
        .inner_size(1000.0, 680.0)
        .min_inner_size(780.0, 540.0)
        .center()
        .decorations(false)
        .transparent(true)
        .shadow(true)
        .visible(false)
        .data_directory(state.paths.webview_dir())
        .additional_browser_args(&browser_args())
        .effects(
            tauri::window::EffectsBuilder::new()
                .effect(tauri::window::Effect::Mica)
                .build(),
        )
        .build();
    match built {
        // The page calls `show()` after its first paint; this is the fallback if it never does.
        Ok(w) => {
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(1500));
                if !w.is_visible().unwrap_or(true) {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            });
        }
        Err(e) => eprintln!("main window: {e}"),
    }
}

/// Navigation requested before the main window existed (read once by the page on mount).
pub fn take_pending_nav() -> Option<NavPayload> {
    ctl().pending_nav.take()
}

pub(crate) fn main_is_active(app: &AppHandle) -> bool {
    app.get_webview_window(MAIN)
        .map(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false))
        .unwrap_or(false)
}

// ------------------------------------------------------------------ overlay window

pub fn create_overlay(app: &AppHandle) -> tauri::Result<()> {
    let state = app.state::<AppState>();
    let w = WebviewWindowBuilder::new(app, OVERLAY, WebviewUrl::App("overlay.html".into()))
        .title("File Chameleon Overlay")
        .inner_size(WHEEL_SIZE, WHEEL_SIZE)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .visible(false)
        .focused(false)
        .focusable(false)
        .data_directory(state.paths.webview_dir())
        .additional_browser_args(&browser_args())
        .build()?;
    #[cfg(windows)]
    if let Ok(hwnd) = w.hwnd() {
        crate::platform::win::make_overlay_style(hwnd.0 as isize);
    }
    let _ = w;
    Ok(())
}

/// The overlay page is listening: open a click-mode wheel or show a toast that arrived before
/// it was (the wheel wins, as it would have then).
pub fn overlay_ready(app: &AppHandle) {
    let (queued, wheel) = {
        let mut c = ctl();
        c.ready = true;
        (c.queued_toast.take(), c.pending_wheel.take())
    };
    if let Some(paths) = wheel {
        // Builds the slots (file checks): not in this WebView2 callback.
        let app = app.clone();
        std::thread::spawn(move || open_wheel_click(&app, paths));
    } else if let Some((kind, data, timeout_ms)) = queued {
        show_toast(app, &kind, data, timeout_ms);
    }
}

fn overlay(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(OVERLAY)
}

/// Run `f` on the main thread (inline when already there). Overlay show/hide uses raw Win32
/// calls and the overlay state machine, so it always runs there, whoever asks.
fn on_main(app: &AppHandle, f: impl FnOnce(&AppHandle) + Send + 'static) {
    let a = app.clone();
    let _ = app.run_on_main_thread(move || f(&a));
}

/// Hide the overlay (wheel or toast). Callable from any thread.
pub fn hide_overlay(app: &AppHandle) {
    on_main(app, hide_overlay_now);
}

#[track_caller]
fn hide_overlay_now(app: &AppHandle) {
    let at = std::panic::Location::caller();
    crate::diag::log(|| format!("hide_overlay_now from {at}"));
    {
        let mut c = ctl();
        c.mode = Mode::Hidden;
        c.generation += 1;
        c.entered = false;
        c.click = false;
        c.slots_for = None;
        c.click_opened = None;
    }
    crate::platform::set_wheel_open(false, false, (0, 0, 0, 0));
    if let Some(w) = overlay(app) {
        let _ = w.emit(events::OVERLAY_HIDE, ());
        #[cfg(windows)]
        if let Ok(hwnd) = w.hwnd() {
            crate::platform::win::hide(hwnd.0 as isize);
            return;
        }
        let _ = w.hide();
    }
}

/// Hide after `ms` unless something re-showed the overlay meanwhile.
fn hide_overlay_later(app: &AppHandle, ms: u64, only_if_not_entered: bool) {
    let generation = ctl().generation;
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(ms));
        let go = {
            let c = ctl();
            c.generation == generation
                && c.mode != Mode::Hidden
                && !(only_if_not_entered && c.entered)
        };
        if go {
            hide_overlay(&app);
        }
    });
}

/// The monitor scale factor at the physical point (x, y) and the physical rect (left, top,
/// right, bottom) of the wheel window centred there, kept inside the work area.
fn wheel_rect(app: &AppHandle, x: i32, y: i32) -> (f64, (i32, i32, i32, i32)) {
    let scale = app
        .monitor_from_point(x as f64, y as f64)
        .ok()
        .flatten()
        .map(|m| m.scale_factor())
        .unwrap_or(1.0);
    let wheel = crate::ipc::presets::wheel_scale(&app.state::<AppState>().settings());
    let size = (WHEEL_SIZE * wheel * scale).round() as i32;
    let (left, top) = (x - size / 2, y - size / 2);
    #[cfg(windows)]
    let (left, top) = match crate::platform::win::work_area_at(x, y) {
        Some((wl, wt, wr, wb)) => (
            left.clamp(wl, (wr - size).max(wl)),
            top.clamp(wt, (wb - size).max(wt)),
        ),
        None => (left, top),
    };
    (scale, (left, top, left + size, top + size))
}

/// Show the overlay as the wheel at `rect` (the mode is already set): reset the page, show the
/// window without activating it, and let the hooks act on input.
fn show_wheel_window(w: &WebviewWindow, scale: f64, rect: (i32, i32, i32, i32), click: bool) {
    ctl().rect = rect;
    crate::diag::log(|| format!("show_wheel_window rect={rect:?} scale={scale} click={click}"));
    let _ = w.emit(events::OVERLAY_WHEEL, WheelPayload { scale, click });
    #[cfg(windows)]
    if let Ok(hwnd) = w.hwnd() {
        let h = hwnd.0 as isize;
        if !click {
            crate::platform::win::share_drop_target(h);
        }
        let (l, t, r, b) = rect;
        crate::platform::win::show_no_activate(h, l, t, r - l, b - t);
    }
    crate::platform::set_wheel_open(true, click, rect);
}

/// Show the wheel centred on the physical screen point (x, y). Called from the mouse hook.
/// The window shows nothing until files enter it (`overlay://wheel-slots`), so other
/// Ctrl+drags (text, a marquee selection) leave no trace. `shift`: Ctrl+Shift+drag.
pub fn show_wheel_at(app: &AppHandle, x: i32, y: i32, shift: bool) {
    let Some(w) = overlay(app) else { return };
    if !ctl().ready {
        return; // the page is still loading and could not draw or hit-test the wheel
    }
    let (scale, rect) = wheel_rect(app, x, y);
    crate::diag::log(|| format!("show_wheel_at ({x},{y}) shift={shift}"));
    {
        let mut c = ctl();
        c.mode = Mode::Wheel;
        c.generation += 1;
        c.entered = false;
        c.click = false;
        c.shift = shift;
        c.wheel_id += 1;
        c.slots_for = None;
    }
    show_wheel_window(&w, scale, rect, false);
    // No files dragged in shortly -> it was not a file drag (e.g. text selection): go away.
    hide_overlay_later(app, 700, true);
}

fn cursor() -> (i32, i32) {
    #[cfg(windows)]
    return crate::platform::win::cursor_pos();
    #[cfg(not(windows))]
    (0, 0)
}

/// Open the wheel in click mode at the cursor for `paths` (Explorer verb, "open the wheel").
/// Callable from any thread. The window takes clicks but never focus: Esc, Tab, the mouse wheel
/// and clicks outside it come from the global hooks. No auto-hide: Esc, a click outside or a
/// choice closes it. Before the page is ready, `overlay_ready` opens it.
pub fn open_wheel_click(app: &AppHandle, paths: Vec<String>) {
    if paths.is_empty() {
        return;
    }
    crate::platform::ensure_hooks(app);
    on_main(app, move |app| {
        let Some(w) = overlay(app) else { return };
        let joined = {
            let mut c = ctl();
            if !c.ready {
                // Every batch that arrives before the page is ready opens with the first.
                merge_paths(c.pending_wheel.get_or_insert_with(Vec::new), paths);
                return;
            }
            let joins = c.mode == Mode::Wheel
                && c.click
                && c.click_opened.is_some_and(|t| t.elapsed() < WHEEL_JOIN);
            if joins {
                let mut all = c.slots_for.clone().unwrap_or_default();
                merge_paths(&mut all, paths);
                if c.slots_for.as_ref() == Some(&all) {
                    return; // nothing new
                }
                // A new id drops the slots still being built for the smaller set.
                c.wheel_id += 1;
                c.slots_for = Some(all.clone());
                Ok((c.wheel_id, all))
            } else {
                Err(paths)
            }
        };
        let paths = match joined {
            // The wheel stays where it is; only its slots change.
            Ok((wheel_id, all)) => {
                send_slots(app, wheel_id, all, false, true);
                return;
            }
            Err(paths) => paths,
        };
        let (x, y) = cursor();
        let (scale, rect) = wheel_rect(app, x, y);
        crate::diag::log(|| format!("open_wheel_click cursor=({x},{y})"));
        let wheel_id = {
            let mut c = ctl();
            c.mode = Mode::Wheel;
            c.generation += 1;
            c.entered = true;
            c.click = true;
            c.shift = false;
            c.wheel_id += 1;
            c.slots_for = Some(paths.clone());
            c.click_opened = Some(Instant::now());
            c.wheel_id
        };
        show_wheel_window(&w, scale, rect, true);
        send_slots(app, wheel_id, paths, false, true);
    });
}

static WHEEL_ARGS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Paths of an Explorer "Convert with File Chameleon" invocation (`--wheel`). Explorer starts one
/// process per selected file, so the paths arriving within `WHEEL_ARGS_MS` open one wheel.
pub fn queue_wheel_paths(app: &AppHandle, paths: Vec<String>) {
    if paths.is_empty() {
        return;
    }
    let first = {
        let mut q = WHEEL_ARGS.lock().unwrap_or_else(|e| e.into_inner());
        let first = q.is_empty();
        merge_paths(&mut q, paths);
        first
    };
    if !first {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(WHEEL_ARGS_MS));
        let paths = std::mem::take(&mut *WHEEL_ARGS.lock().unwrap_or_else(|e| e.into_inner()));
        open_wheel_click(&app, paths);
    });
}

/// Hook: mouse wheel or Tab while the wheel is open.
/// Where a flip came from: the keyboard/mouse hook, or the line page passing on a Tab.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FlipFrom {
    Hook,
    Line,
}

/// A flip from the hook and one from the line within this time are the same Tab press.
const SAME_PRESS: Duration = Duration::from_millis(250);

/// When each source last flipped (hook, line).
static LAST_FLIPS: Mutex<(Option<Instant>, Option<Instant>)> = Mutex::new((None, None));

/// Should this flip happen? During a Ctrl+drag from the line, Tab reaches both the hook (which
/// never swallows keys during a drag) and the focused line page, which passes it on: one press
/// would flip twice and leave the wheel as it was. A flip right after one from the other source
/// is that press again, and uses up the other one.
fn flip_counts(
    last: &mut (Option<Instant>, Option<Instant>),
    from: FlipFrom,
    now: Instant,
) -> bool {
    let (mine, other) = match from {
        FlipFrom::Hook => (&mut last.0, &mut last.1),
        FlipFrom::Line => (&mut last.1, &mut last.0),
    };
    if other.is_some_and(|t| now.saturating_duration_since(t) < SAME_PRESS) {
        *other = None;
        return false;
    }
    *mine = Some(now);
    true
}

/// Flip the wheel, unless the other source just flipped it for the same press.
pub fn wheel_flip_from(app: &AppHandle, from: FlipFrom) {
    let counts = flip_counts(
        &mut LAST_FLIPS.lock().unwrap_or_else(|e| e.into_inner()),
        from,
        Instant::now(),
    );
    crate::diag::log(|| format!("flip from {from:?} counts={counts}"));
    if counts {
        wheel_flip(app);
    }
}

pub fn wheel_flip(app: &AppHandle) {
    // No window work, so no trip to the main thread: during a Ctrl+drag that starts on the line
    // the main thread sits in the drag loop until the drop.
    let open = ctl().mode == Mode::Wheel;
    if open && let Some(w) = overlay(app) {
        let _ = w.emit(events::OVERLAY_WHEEL_FLIP, ());
    }
}

/// Hook: Esc, or a press outside the click-mode wheel.
pub fn wheel_close(app: &AppHandle) {
    on_main(app, |app| {
        let open = ctl().mode == Mode::Wheel;
        if open {
            hide_overlay_now(app);
        }
    });
}

/// Build the slots for `paths` off the main thread (file checks for every preset) and send
/// them, then the first file's thumbnail when it is an image, to wheel `wheel_id`.
fn send_slots(app: &AppHandle, wheel_id: u64, paths: Vec<String>, shift: bool, click: bool) {
    // Files that reach the wheel (Ctrl+drag, the Explorer verb) join the files rope.
    crate::files::record(
        app,
        paths.iter().map(std::path::PathBuf::from).collect(),
        fc_core::files::FileSource::Added,
    );
    let app = app.clone();
    std::thread::spawn(move || {
        let first = paths.first().cloned();
        let slots = crate::ipc::presets::wheel_slots_for(&app, paths, shift, click);
        emit_to_wheel(&app, wheel_id, events::OVERLAY_WHEEL_SLOTS, slots);
        if let Some(path) = first
            && let Some(data_url) = crate::ipc::images::wheel_thumbnail(&path)
        {
            let thumb = WheelThumb { path, data_url };
            emit_to_wheel(&app, wheel_id, events::OVERLAY_WHEEL_THUMB, thumb);
        }
    });
}

/// Emit to the overlay, only if wheel `wheel_id` is still the one shown. Straight from the
/// calling thread (emitting needs no window work): during a Ctrl+drag that starts on the line the
/// main thread sits in the drag loop, and the slots must reach the wheel before the drop.
fn emit_to_wheel<S: Serialize + Clone + Send + 'static>(
    app: &AppHandle,
    wheel_id: u64,
    event: &'static str,
    payload: S,
) {
    let current = {
        let c = ctl();
        c.mode == Mode::Wheel && c.wheel_id == wheel_id
    };
    if current && let Some(w) = overlay(app) {
        let _ = w.emit(event, payload);
    }
}

/// Is the wheel showing?
pub fn wheel_is_open() -> bool {
    ctl().mode == Mode::Wheel
}

/// The overlay page's point for the physical screen point `(x, y)`, in a window at `left, top`
/// whose device pixel ratio is `dpr`.
fn page_point((x, y): (i32, i32), (left, top): (i32, i32), dpr: f64) -> WheelPointer {
    let dpr = if dpr.is_finite() && dpr > 0.0 {
        dpr
    } else {
        1.0
    };
    WheelPointer {
        x: f64::from(x - left) / dpr,
        y: f64::from(y - top) / dpr,
    }
}

/// A Ctrl+drag that started on the line (no OLE drag: we own both ends): the pointer moved, or
/// was released (`dropped`). Tells the wheel's page where the cursor is, so it highlights the
/// slice under it or runs it. A release also cleans up like the end of a File Explorer drag.
pub fn wheel_pointer(app: &AppHandle, dropped: bool) {
    let (rect, open) = {
        let c = ctl();
        (c.rect, c.mode == Mode::Wheel && !c.click)
    };
    if !open {
        return;
    }
    let Some(w) = overlay(app) else { return };
    let dpr = w.scale_factor().unwrap_or(1.0);
    let p = page_point(cursor(), (rect.0, rect.1), dpr);
    let event = if dropped {
        events::OVERLAY_WHEEL_DROP
    } else {
        events::OVERLAY_WHEEL_POINTER
    };
    let _ = w.emit(event, p);
    if dropped {
        crate::diag::log(|| format!("wheel drop at page ({:.0},{:.0})", p.x, p.y));
        on_drag_released(app);
    }
}

/// Files reached the drag wheel: build and send its slots (once per set of files). Called for
/// the overlay's drag-enter event, and up front by a Ctrl+drag that starts on the line, where the
/// files are known before the drag (the enter event only arrives through the main thread).
pub fn files_entered(app: &AppHandle, paths: Vec<String>) {
    let send = {
        let mut c = ctl();
        // No files: not a file drag. The wheel stays invisible and goes away.
        // Click mode ignores drags (the page does too).
        if c.mode != Mode::Wheel || c.click || paths.is_empty() {
            return;
        }
        c.entered = true;
        c.generation += 1; // cancels a pending "leave" hide
        // Re-entering with the same files keeps the face and hover state.
        (c.slots_for.as_ref() != Some(&paths)).then(|| {
            c.slots_for = Some(paths.clone());
            (c.wheel_id, c.shift)
        })
    };
    if let Some((wheel_id, shift)) = send {
        send_slots(app, wheel_id, paths, shift, false);
    }
}

/// Mouse button released somewhere: if nothing entered the wheel, hide it.
pub fn on_drag_released(app: &AppHandle) {
    let (mode, entered, click) = {
        let c = ctl();
        (c.mode, c.entered, c.click)
    };
    if mode == Mode::Wheel && !click {
        // A drop on the wheel is handled by the page; give it a moment, then clean up anyway.
        hide_overlay_later(app, if entered { 1500 } else { 120 }, false);
    }
}

/// Show a toast at the bottom right of the work area under the cursor. Callable from any
/// thread (it runs on the main thread). Before the overlay page is ready the latest toast is
/// queued and shown by `overlay_ready`, never as an empty window.
pub fn show_toast(app: &AppHandle, kind: &str, data: serde_json::Value, timeout_ms: Option<u64>) {
    let kind = kind.to_string();
    on_main(app, move |app| show_toast_now(app, &kind, data, timeout_ms));
}

fn show_toast_now(app: &AppHandle, kind: &str, data: serde_json::Value, timeout_ms: Option<u64>) {
    let Some(w) = overlay(app) else { return };
    {
        let mut c = ctl();
        if !c.ready {
            c.queued_toast = Some((kind.to_string(), data, timeout_ms));
            return;
        }
        if c.mode == Mode::Wheel {
            return; // the wheel has priority; results are still listed in the app
        }
    }
    let state = app.state::<AppState>();
    let payload = ToastPayload {
        id: state.next_toast_id(),
        kind: kind.into(),
        data,
        progress: None,
        timeout_ms,
    };
    {
        let mut c = ctl();
        c.mode = Mode::Toast;
        c.generation += 1;
    }
    let _ = w.emit(events::OVERLAY_TOAST, payload);
    #[cfg(windows)]
    if let Ok(hwnd) = w.hwnd() {
        let (cx, cy) = crate::platform::win::cursor_pos();
        if let Some((_, _, wr, wb)) = crate::platform::win::work_area_at(cx, cy) {
            let scale = app
                .monitor_from_point(cx as f64, cy as f64)
                .ok()
                .flatten()
                .map(|m| m.scale_factor())
                .unwrap_or(1.0);
            let (tw, th) = (
                (TOAST_W * scale).round() as i32,
                (TOAST_H * scale).round() as i32,
            );
            // Bottom right (the card keeps its margin inside the window), clear of the caption
            // buttons of maximized windows.
            crate::platform::win::show_no_activate(hwnd.0 as isize, wr - tw, wb - th, tw, th);
        }
    }
    if let Some(ms) = timeout_ms {
        hide_overlay_later(app, ms, false);
    }
}

// ------------------------------------------------------------------ toasts for app events

static FINISHED: Mutex<Vec<JobSnapshot>> = Mutex::new(Vec::new());

/// Collect finished jobs for 400 ms, then show one toast (single result or a batch summary).
/// Skipped while the main window is focused (the Convert view shows results itself).
pub fn toast_job_finished(app: &AppHandle, snap: &JobSnapshot) {
    if snap.state == JobState::Cancelled || main_is_active(app) {
        return;
    }
    let first = {
        let mut f = FINISHED.lock().unwrap_or_else(|e| e.into_inner());
        f.push(snap.clone());
        f.len() == 1
    };
    if !first {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(400));
        let batch: Vec<JobSnapshot> =
            std::mem::take(&mut *FINISHED.lock().unwrap_or_else(|e| e.into_inner()));
        let app2 = app.clone();
        let _ = app.run_on_main_thread(move || {
            let name = |s: &JobSnapshot| {
                s.inputs.first().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
            };
            if batch.len() == 1 {
                let s = &batch[0];
                if s.state == JobState::Done {
                    show_toast(&app2, "jobDone", json!({ "jobId": s.id, "outputs": s.outputs, "inputName": name(s) }), Some(6000));
                } else {
                    show_toast(
                        &app2,
                        "jobFailed",
                        json!({ "jobId": s.id, "inputName": name(s), "errorCode": s.error_code, "error": s.error }),
                        Some(9000),
                    );
                }
            } else {
                let done = batch.iter().filter(|s| s.state == JobState::Done).count();
                let first_out = batch.iter().find_map(|s| s.outputs.first().cloned());
                show_toast(
                    &app2,
                    "jobsBatch",
                    json!({ "done": done, "failed": batch.len() - done, "total": batch.len(), "firstOutput": first_out }),
                    Some(7000),
                );
            }
        });
    });
}

/// Toast button handler, called from an async command (never on the main thread). The toast is
/// hidden first, so an action may show a follow-up toast. Unknown actions just dismiss.
pub fn toast_action(
    app: &AppHandle,
    state: &AppState,
    kind: &str,
    action: &str,
    data: &serde_json::Value,
) {
    use tauri_plugin_opener::OpenerExt;
    hide_overlay(app);
    let s = |k: &str| data.get(k).and_then(|v| v.as_str()).map(String::from);
    let first_output = data
        .get("outputs")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|v| v.as_str())
        .map(String::from)
        .or_else(|| s("firstOutput"))
        .or_else(|| s("output"));
    match (kind, action) {
        (_, "reveal") => {
            if let Some(p) = first_output {
                let _ = app.opener().reveal_item_in_dir(p);
            }
        }
        (_, "open") => {
            if let Some(p) = first_output {
                let _ = app.opener().open_path(p, None::<&str>);
            }
        }
        (_, "details") => show_main(
            app,
            Some(NavPayload {
                view: "recent".into(),
                paths: vec![],
                url: None,
                clip: None,
            }),
        ),
        ("updateAvailable", "install") => crate::update::install_from_toast(app),
        ("clipLink", "download") => show_main(
            app,
            Some(NavPayload {
                view: "download".into(),
                paths: vec![],
                url: s("url"),
                clip: None,
            }),
        ),
        ("clipImage", "convert") => {
            if let Some(thumb) = data.get("image").and_then(|v| v.as_str()) {
                show_main(
                    app,
                    Some(NavPayload {
                        view: "convert".into(),
                        paths: vec![thumb.into()],
                        url: None,
                        clip: None,
                    }),
                );
            }
        }
        ("clipImage", "savePng") => {
            if let Some(id) = data.get("clipId").and_then(|v| v.as_u64()) {
                let app = app.clone();
                // Decoding and encoding a large image takes a while: not on this thread.
                tauri::async_runtime::spawn_blocking(move || {
                    let state = app.state::<AppState>();
                    match crate::ipc::clipboard::save_image(&state, id, "png") {
                        Ok(out) => {
                            show_toast(&app, "imageSaved", json!({ "output": out }), Some(6000))
                        }
                        Err(e) => eprintln!("save clipboard image: {}", e.message),
                    }
                });
            }
        }
        ("toolNeeded", "install") => {
            if let Some(tool) = s("tool") {
                install_then_convert(app, state, tool, data);
            }
        }
        _ => {}
    }
}

/// `toolNeeded` → "Download": install in the background, then run the preset `target` on the
/// dropped files.
fn install_then_convert(app: &AppHandle, state: &AppState, tool: String, data: &serde_json::Value) {
    let name = state
        .engines()
        .manifest()
        .get(&tool)
        .map(|e| e.name.clone())
        .unwrap_or_else(|| tool.clone());
    let paths: Vec<std::path::PathBuf> = data
        .get("paths")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|p| p.as_str())
                .map(Into::into)
                .collect()
        })
        .unwrap_or_default();
    let target = data
        .get("target")
        .and_then(|v| v.as_str())
        .map(String::from);
    show_toast(
        app,
        "toolInstalling",
        json!({ "tool": tool, "name": name }),
        Some(5000),
    );
    let app = app.clone();
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        // Installed meanwhile (e.g. from Settings): just convert.
        let installed = if state.engines().is_available(&tool) {
            Ok(())
        } else {
            crate::ipc::install_now(&app, &tool)
        };
        if let Err(e) = installed {
            show_toast(
                &app,
                "toolFailed",
                json!({ "tool": tool, "name": name, "errorCode": e.code(), "error": e.to_string() }),
                Some(9000),
            );
            return;
        }
        let (Some(preset), false) = (target, paths.is_empty()) else {
            return;
        };
        // Results come back as the usual job toasts.
        if let Err(e) = crate::ipc::presets::run_preset(&app, paths, &preset, true, None) {
            eprintln!("run preset {preset} after install: {}", e.message);
        }
    });
}

// ------------------------------------------------------------------ window events

pub fn on_window_event(window: &Window, event: &WindowEvent) {
    let app = window.app_handle();
    if window.label() == MAIN {
        // "Close button: quit" (Settings > General): closing the window ends the app, through
        // the tray's quit (running jobs are cancelled and their partial files removed).
        if let WindowEvent::CloseRequested { .. } = event
            && !app.state::<AppState>().settings().close_to_tray
        {
            crate::tray::quit(app);
        }
        return;
    }
    if window.label() == crate::line::LINE {
        if let WindowEvent::Focused(false) = event {
            crate::line::on_blur(app);
        }
        return;
    }
    if window.label() != OVERLAY {
        return;
    }
    if let WindowEvent::DragDrop(dd) = event {
        match dd {
            DragDropEvent::Enter { paths, .. } => {
                crate::diag::log(|| format!("overlay drag enter: {} file(s)", paths.len()));
                let paths: Vec<String> = paths
                    .iter()
                    .map(|p| p.to_string_lossy().into_owned())
                    .collect();
                files_entered(app, paths);
            }
            DragDropEvent::Leave => {
                crate::diag::log(|| "overlay drag leave".into());
                let drag = {
                    let c = ctl();
                    c.mode == Mode::Wheel && !c.click
                };
                if drag {
                    hide_overlay_later(app, 150, false);
                }
            }
            // The page hit-tests the drop and calls `wheel_drop` or `overlay_hide`.
            DragDropEvent::Drop { .. } if !ctl().click => {
                crate::diag::log(|| "overlay drop".into());
                hide_overlay_later(app, 2000, false);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FlipFrom, flip_counts, merge_paths, page_point};
    use std::time::{Duration, Instant};

    #[test]
    fn one_tab_seen_by_hook_and_line_flips_once() {
        let t = Instant::now();
        let mut last = (None, None);
        // Hook first, then the line passing on the same press: one flip.
        assert!(flip_counts(&mut last, FlipFrom::Hook, t));
        assert!(!flip_counts(
            &mut last,
            FlipFrom::Line,
            t + Duration::from_millis(40)
        ));
        // The next press, again seen by both (line first this time): one more flip.
        let t2 = t + Duration::from_millis(600);
        assert!(flip_counts(&mut last, FlipFrom::Line, t2));
        assert!(!flip_counts(
            &mut last,
            FlipFrom::Hook,
            t2 + Duration::from_millis(30)
        ));
        // Only the hook sees the presses (no line page): every press flips, even quick ones.
        let t3 = t2 + Duration::from_millis(600);
        assert!(flip_counts(&mut last, FlipFrom::Hook, t3));
        assert!(flip_counts(
            &mut last,
            FlipFrom::Hook,
            t3 + Duration::from_millis(100)
        ));
        // A late line copy (after the window) is a press of its own.
        assert!(flip_counts(
            &mut last,
            FlipFrom::Line,
            t3 + Duration::from_millis(500)
        ));
    }

    #[test]
    fn page_points_are_logical_and_window_relative() {
        let p = page_point((1100, 300), (1000, 100), 1.25);
        assert_eq!((p.x, p.y), (80.0, 160.0));
        let p = page_point((5, 5), (0, 0), 0.0);
        assert_eq!((p.x, p.y), (5.0, 5.0));
    }

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn later_wheel_batches_join_the_earlier_ones() {
        // Explorer starts one process per selected file; batches arrive one after another.
        let mut all = strings(&[r"C:\a.png", r"C:\b.png"]);
        merge_paths(&mut all, strings(&[r"C:\c.png"]));
        merge_paths(&mut all, strings(&[r"C:\b.png", r"C:\d.png", r"C:\d.png"]));
        assert_eq!(
            all,
            strings(&[r"C:\a.png", r"C:\b.png", r"C:\c.png", r"C:\d.png"])
        );
        // Nothing new: unchanged.
        merge_paths(&mut all, strings(&[r"C:\a.png"]));
        merge_paths(&mut all, Vec::new());
        assert_eq!(all.len(), 4);
        let mut empty = Vec::new();
        merge_paths(&mut empty, strings(&["x", "x", "y"]));
        assert_eq!(empty, strings(&["x", "y"]));
    }
}

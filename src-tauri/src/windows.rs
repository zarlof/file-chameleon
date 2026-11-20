//! Window management: the main window (created on demand, destroyed on close) and the
//! single always-alive overlay window used for the drag wheel and toasts.

use crate::events::{self, NavPayload, ToastPayload, WheelPayload, WheelTargets};
use crate::state::AppState;
use fc_core::jobs::{JobSnapshot, JobState};
use serde_json::json;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{
    AppHandle, DragDropEvent, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    Window, WindowEvent,
};

pub const MAIN: &str = "main";
pub const OVERLAY: &str = "overlay";

/// Logical size of the wheel window (the wheel itself is drawn inside with a shadow margin).
pub const WHEEL_SIZE: f64 = 440.0;
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
}

static CTL: Mutex<OverlayCtl> = Mutex::new(OverlayCtl {
    mode: Mode::Hidden,
    generation: 0,
    entered: false,
    ready: false,
    queued_toast: None,
    pending_nav: None,
});

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

fn browser_args() -> String {
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

/// The overlay page is listening: show a toast that arrived before it was.
pub fn overlay_ready(app: &AppHandle) {
    let queued = {
        let mut c = ctl();
        c.ready = true;
        c.queued_toast.take()
    };
    if let Some((kind, data, timeout_ms)) = queued {
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

fn hide_overlay_now(app: &AppHandle) {
    {
        let mut c = ctl();
        c.mode = Mode::Hidden;
        c.generation += 1;
        c.entered = false;
    }
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

/// Show the wheel centred on the physical screen point (x, y). Called from the mouse hook.
/// The window shows nothing until files enter it (`overlay://wheel-targets`), so other
/// Ctrl+drags (text, a marquee selection) leave no trace.
pub fn show_wheel_at(app: &AppHandle, x: i32, y: i32) {
    let Some(w) = overlay(app) else { return };
    if !ctl().ready {
        return; // the page is still loading and could not draw or hit-test the wheel
    }
    let scale = app
        .monitor_from_point(x as f64, y as f64)
        .ok()
        .flatten()
        .map(|m| m.scale_factor())
        .unwrap_or(1.0);
    let size = (WHEEL_SIZE * scale).round() as i32;
    {
        let mut c = ctl();
        c.mode = Mode::Wheel;
        c.generation += 1;
        c.entered = false;
    }
    let _ = w.emit(events::OVERLAY_WHEEL, WheelPayload { scale });
    #[cfg(windows)]
    if let Ok(hwnd) = w.hwnd() {
        crate::platform::win::share_drop_target(hwnd.0 as isize);
        let (mut left, mut top) = (x - size / 2, y - size / 2);
        if let Some((wl, wt, wr, wb)) = crate::platform::win::work_area_at(x, y) {
            left = left.clamp(wl, (wr - size).max(wl));
            top = top.clamp(wt, (wb - size).max(wt));
        }
        crate::platform::win::show_no_activate(hwnd.0 as isize, left, top, size, size);
    }
    // No files dragged in shortly -> it was not a file drag (e.g. text selection): go away.
    hide_overlay_later(app, 700, true);
}

/// Mouse button released somewhere: if nothing entered the wheel, hide it.
pub fn on_drag_released(app: &AppHandle) {
    let (mode, entered) = {
        let c = ctl();
        (c.mode, c.entered)
    };
    if mode == Mode::Wheel {
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
            }),
        ),
        ("clipLink", "download") => show_main(
            app,
            Some(NavPayload {
                view: "download".into(),
                paths: vec![],
                url: s("url"),
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

/// `toolNeeded` → "Download": install in the background, then convert the dropped files.
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
        let (Some(to), false) = (target, paths.is_empty()) else {
            return;
        };
        let req = fc_core::jobs::JobRequest {
            inputs: paths,
            op: fc_core::jobs::Operation::Convert { to },
            options: Default::default(),
        };
        // Results come back as the usual job toasts.
        let _ = crate::ipc::submit(&state, req);
    });
}

// ------------------------------------------------------------------ window events

pub fn on_window_event(window: &Window, event: &WindowEvent) {
    if window.label() != OVERLAY {
        return;
    }
    let app = window.app_handle();
    if let WindowEvent::DragDrop(dd) = event {
        match dd {
            DragDropEvent::Enter { paths, .. } => {
                // No files: not a file drag. The wheel stays invisible and goes away.
                if ctl().mode != Mode::Wheel || paths.is_empty() {
                    return;
                }
                {
                    let mut c = ctl();
                    c.entered = true;
                    c.generation += 1; // cancels a pending "leave" hide
                }
                let paths: Vec<String> = paths
                    .iter()
                    .map(|p| p.to_string_lossy().into_owned())
                    .collect();
                let (inputs, targets) = crate::ipc::targets_for_paths(&paths);
                let first_kind = inputs.first().and_then(|i| i.kind);
                let state = app.state::<AppState>();
                let recent = state
                    .settings()
                    .recent_targets
                    .into_iter()
                    .filter(|r| targets.iter().any(|t| t.id == r))
                    .collect();
                // Only these get the download glyph (managed or PATH copies count as present).
                let mut missing_tools: Vec<String> = Vec::new();
                for tool in targets.iter().flat_map(|t| t.needs_tools.iter()) {
                    if !missing_tools.iter().any(|m| m.as_str() == *tool)
                        && !state.engines().is_available(tool)
                    {
                        missing_tools.push(tool.to_string());
                    }
                }
                let _ = window.emit(
                    events::OVERLAY_WHEEL_TARGETS,
                    WheelTargets {
                        paths,
                        first_kind,
                        targets,
                        recent,
                        missing_tools,
                    },
                );
            }
            DragDropEvent::Leave => {
                if ctl().mode == Mode::Wheel {
                    hide_overlay_later(app, 150, false);
                }
            }
            DragDropEvent::Drop { .. } => {
                // The page hit-tests the drop and calls `wheel_drop` or `overlay_hide`.
                hide_overlay_later(app, 2000, false);
            }
            _ => {}
        }
    }
}

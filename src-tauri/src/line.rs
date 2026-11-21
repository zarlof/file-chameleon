//! The clothesline window (`line.html`, label `line`) and the file actions of the wheel.
//! OWNER: clothesline Task 2.
//!
//! Created on first use, then only hidden and shown again, like the shelf. Three show modes:
//! - `hotkey`: activates the window; the foreground window before it is remembered so
//!   `line_paste` can paste there.
//! - `peek` (after a copy) and `reveal` (pointer dwelling at the top edge): never take focus
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

/// Logical metrics: free space left and right, height of one rope, height of the padding.
/// These must equal what the page draws (`src/line/Rope.svelte` BAND = 178 px plus its 4 px
/// bottom margin = 182 px per rope; `.strip` in `src/line/Line.svelte` has 4 px padding above and
/// below = 8 px), so nothing transparent hangs under the last rope.
const SIDE_SPACE: f64 = 24.0;
const ROPE_HEIGHT: f64 = 182.0;
const CHROME_HEIGHT: f64 = 8.0;
/// The pointer must stay in the top-edge zone this long before the line reveals itself.
const DWELL: Duration = Duration::from_millis(350);
/// How many folders "open a terminal here" opens at most.
const MAX_TERMINALS: usize = 5;

/// Physical `(x, y, width, height)` of the line in the work area `(left, top, right, bottom)`
/// (physical pixels) of a monitor with DPI `scale`: 24 px logical free on each side, height
/// `ropes × 182 + 8` logical (at least one rope), at the top, never taller than the work area.
pub fn line_rect(work: (i32, i32, i32, i32), scale: f64, ropes: usize) -> (i32, i32, i32, i32) {
    let (left, top, right, bottom) = work;
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let (work_w, work_h) = ((right - left).max(1), (bottom - top).max(1));
    let side = (SIDE_SPACE * scale).round() as i32;
    let width = (work_w - 2 * side).clamp(1, work_w);
    let height = ((ropes.max(1) as f64 * ROPE_HEIGHT + CHROME_HEIGHT) * scale).round() as i32;
    (left + side, top, width, height.clamp(1, work_h))
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

/// How a show mode treats focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShowFlags {
    /// Only the hotkey activates; peek, reveal and anything unknown never steal focus.
    pub activate: bool,
}

pub fn show_flags(mode: &str) -> ShowFlags {
    ShowFlags {
        activate: mode == "hotkey",
    }
}

/// The paths as clipboard text, one per line (CRLF).
pub fn copy_path_text(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("\r\n")
}

/// The folders to open terminals in: a file -> its folder, a folder -> itself. Duplicates are
/// dropped (first occurrence wins) and at most [`MAX_TERMINALS`] are kept.
pub fn terminal_dirs(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for p in paths {
        let dir = if p.is_dir() {
            Some(p.as_path())
        } else {
            p.parent()
        };
        let Some(dir) = dir.filter(|d| !d.as_os_str().is_empty()) else {
            continue;
        };
        if !out.iter().any(|d| d == dir) {
            out.push(dir.to_path_buf());
        }
        if out.len() == MAX_TERMINALS {
            break;
        }
    }
    out
}

/// The folder argument for `wt -d`: a trailing backslash is dropped (it would escape the closing
/// quote of `"D:\My Dir\"`), except for a drive root such as `C:\`.
pub fn terminal_arg(dir: &Path) -> PathBuf {
    let s = dir.to_string_lossy();
    let trimmed = s.trim_end_matches('\\');
    let is_root = trimmed.len() == 2 && trimmed.ends_with(':');
    if trimmed.len() < s.len() && !is_root && !trimmed.is_empty() {
        PathBuf::from(trimmed)
    } else {
        dir.to_path_buf()
    }
}

/// What dragging a clip out of the line carries.
#[derive(Debug, PartialEq)]
pub enum DragPlan {
    /// An image: its file (the edited copy when there is one).
    File(PathBuf),
    /// Text or a link.
    Text(String),
    Nothing,
}

pub fn drag_plan(item: &fc_core::clip::ClipItem) -> DragPlan {
    use fc_core::clip::ClipKind;
    if item.kind == ClipKind::Image {
        return item
            .edited
            .clone()
            .or_else(|| item.image.clone())
            .map_or(DragPlan::Nothing, DragPlan::File);
    }
    match item.text.as_deref() {
        Some(t) if !t.is_empty() => DragPlan::Text(t.to_string()),
        _ => DragPlan::Nothing,
    }
}

/// Only web links are opened.
pub fn is_web_url(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    (lower.starts_with("http://") || lower.starts_with("https://"))
        && !url.chars().any(|c| c.is_control() || c.is_whitespace())
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
    /// "hotkey" | "peek" | "reveal"; `None` when the line is not up.
    pub mode: Option<String>,
    pub visible: bool,
}

/// The mode the line shows in when `new` is requested while `current` is already pending: the
/// hotkey (activating) is never downgraded by a peek or reveal; otherwise the latest wins.
pub fn merge_mode<'a>(current: Option<&'a str>, new: &'a str) -> &'a str {
    match current {
        Some("hotkey") => "hotkey",
        _ => new,
    }
}

/// Latest requested mode since the last hide.
static MODE: Mutex<Option<String>> = Mutex::new(None);

/// Serializes [`show_at`]: a peek racing the hotkey must not re-set `WS_EX_NOACTIVATE` on a line
/// that was just activated.
static SHOW: Mutex<()> = Mutex::new(());

/// Number of ropes the window has room for: those enabled in the settings AND holding at least
/// one live clip (`present`, in the order images, text, links). At least 1 (the empty message).
pub fn rope_count_of(enabled: [bool; 3], present: [bool; 3]) -> usize {
    enabled
        .iter()
        .zip(present)
        .filter(|(on, has)| **on && *has)
        .count()
        .max(1)
}

/// Hide on focus loss? Only a line that was activated (hotkey, or a click on a peek).
pub fn should_hide_on_blur(active: bool, focused: bool) -> bool {
    active && !focused
}

/// The line was activated (the hotkey or a click) and has not lost focus since.
static ACTIVE: AtomicBool = AtomicBool::new(false);

fn create(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let state = app.state::<AppState>();
    let ropes = rope_count(app);
    let work = (0, 0, 1920, 1040);
    let (_, _, w, h) = line_rect(work, 1.0, ropes);
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

/// How many ropes the window needs right now (see [`rope_count_of`]).
fn rope_count(app: &AppHandle) -> usize {
    use fc_core::clip::{ClipQuery, Shelf};
    let state = app.state::<AppState>();
    let r = state.settings().line_ropes;
    let enabled = [r.images, r.text, r.links];
    if !enabled.iter().any(|on| *on) {
        return 1;
    }
    let store = state.clips().lock().unwrap_or_else(|e| e.into_inner());
    let has = |on: bool, shelf: Shelf| {
        on && !store
            .query(&ClipQuery {
                shelf: Some(shelf),
                limit: Some(1),
                ..Default::default()
            })
            .is_empty()
    };
    let present = [
        has(enabled[0], Shelf::Images),
        has(enabled[1], Shelf::Texts),
        has(enabled[2], Shelf::Links),
    ];
    rope_count_of(enabled, present)
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
    let ropes = rope_count(app);
    let (x, y, width, height) = line_rect(work, scale, ropes);
    // Position first: moving to a monitor with another DPI may resize the window.
    let _ = w.set_position(PhysicalPosition::new(x, y));
    let _ = w.set_size(PhysicalSize::new(width as u32, height as u32));
}

/// Remember the foreground window so `line_paste` can paste into it. `force` (the hotkey) also
/// records "none" (our own window, the taskbar); otherwise only a real foreign window counts.
#[cfg(windows)]
fn remember_foreground(app: &AppHandle, force: bool) {
    use crate::platform::win;
    let fg = win::foreground_window();
    let usable = fg != 0
        && !win::is_own_window(fg)
        && !crate::shelf::is_taskbar_class(&win::window_class(fg));
    if usable || force {
        app.state::<AppState>()
            .line_prev
            .store(if usable { fg } else { 0 }, Ordering::SeqCst);
    }
}

/// Show the line in `mode` ("hotkey" | "peek" | "reveal"), creating it on first use.
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
            // Already up (maybe in use): do not touch its focus style, just tell the page so it
            // keeps itself open.
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
    let armed = DWELL_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    let _ = std::thread::Builder::new()
        .name("fc-line-dwell".into())
        .spawn(move || {
            std::thread::sleep(DWELL);
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

// ---------------------------------------------------------------- commands' work

/// `line_paste`: copy the items, hide the line and paste into the window it came from.
pub fn paste(app: &AppHandle, ids: &[u64]) -> CmdResult<()> {
    crate::ipc::clipboard::copy_items(&app.state::<AppState>(), ids)?;
    hide(app);
    let prev = app.state::<AppState>().line_prev.load(Ordering::SeqCst);
    crate::shelf::paste_keys_into(prev);
    Ok(())
}

/// `line_open_item`: the clipboard library, with that clip selected.
pub fn open_item(app: &AppHandle, id: u64) {
    hide(app);
    crate::windows::show_main(
        app,
        Some(NavPayload {
            view: "clipboard".into(),
            paths: vec![],
            url: None,
            clip: Some(id),
        }),
    );
}

/// `line_drag_out`: start an OLE drag of clip `id` (copy only) and wait for the drop to end. The
/// drag runs on the main (UI) thread, which blocks inside `SHDoDragDrop` until then.
pub async fn drag_out(app: &AppHandle, id: u64) -> CmdResult<()> {
    let item = crate::ipc::clipboard::find(&app.state::<AppState>(), id)?;
    let plan = drag_plan(&item);
    if plan == DragPlan::Nothing {
        return Err(CmdError::new("notFound", "nothing to drag"));
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        Err(CmdError::new("unsupported", "drag out needs Windows"))
    }
    #[cfg(windows)]
    {
        use crate::platform::win;
        let owner = app
            .get_webview_window(LINE)
            .and_then(|w| hwnd_of(&w))
            .ok_or_else(|| CmdError::new("failed", "line window is not open"))?;
        let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
        app.run_on_main_thread(move || {
            let result = match plan {
                DragPlan::File(p) => win::file_data_object(&[p]),
                DragPlan::Text(t) => win::text_data_object(&t),
                DragPlan::Nothing => Err("nothing to drag".into()),
            }
            .and_then(|data| win::do_drag(owner, &data));
            let _ = tx.send(result);
        })
        .map_err(|e| CmdError::new("failed", e.to_string()))?;
        crate::ipc::blocking(move || rx.recv())
            .await?
            .map_err(|e| CmdError::new("failed", e.to_string()))?
            .map_err(|e| CmdError::new("drag", e))
    }
}

/// `open_url`: web links only.
pub fn open_url(url: &str) -> CmdResult<()> {
    if !is_web_url(url) {
        return Err(CmdError::new("invalid", "only http and https links open"));
    }
    #[cfg(windows)]
    if crate::platform::win::shell_open(url.trim()) {
        return Ok(());
    }
    Err(CmdError::new("io", "could not open the link"))
}

// ---------------------------------------------------------------- file actions (wheel)

/// The `copyPath` preset: the paths, one per line, on the clipboard. Our own write is not
/// recorded as a new clip.
pub fn copy_paths(app: &AppHandle, paths: &[PathBuf]) -> CmdResult<()> {
    let text = copy_path_text(paths);
    match crate::platform::clipboard::write_text(&text) {
        Ok(seq) => {
            app.state::<AppState>()
                .clip_self_write
                .store(seq, Ordering::SeqCst);
            Ok(())
        }
        Err(e) => Err(CmdError::new("clipboard", e)),
    }
}

/// The `openTerminal` preset: Windows Terminal in each folder (at most five), PowerShell when
/// `wt.exe` is not there.
pub fn open_terminal(paths: &[PathBuf]) -> CmdResult<()> {
    let dirs = terminal_dirs(paths);
    if dirs.is_empty() {
        return Err(CmdError::new("unsupported", "no folder to open"));
    }
    let mut failed = None;
    for dir in &dirs {
        if let Err(e) = spawn_terminal(dir) {
            failed = Some(e);
        }
    }
    failed.map_or(Ok(()), |e| Err(CmdError::new("io", e.to_string())))
}

#[cfg(windows)]
fn spawn_terminal(dir: &Path) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
    // wt.exe makes its own window: no console flash for the launcher.
    let wt = Command::new("wt.exe")
        .arg("-d")
        .arg(terminal_arg(dir))
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
    match wt {
        Ok(_) => Ok(()),
        Err(_) => Command::new("powershell.exe")
            .arg("-NoExit")
            .current_dir(dir)
            .creation_flags(CREATE_NEW_CONSOLE)
            .spawn()
            .map(|_| ()),
    }
}

#[cfg(not(windows))]
fn spawn_terminal(_dir: &Path) -> std::io::Result<()> {
    Err(std::io::Error::other("terminals need Windows"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_rect_spans_work_area() {
        assert_eq!(line_rect((0, 0, 1920, 1040), 1.0, 3), (24, 0, 1872, 554));
        assert_eq!(line_rect((0, 0, 2880, 1560), 1.5, 2), (36, 0, 2808, 558));
        // Second monitor below a top taskbar; no ropes still gives one rope's height.
        assert_eq!(
            line_rect((1920, 40, 3840, 1080), 1.0, 0),
            (1944, 40, 1872, 190)
        );
        // Never taller than the work area.
        assert_eq!(line_rect((0, 0, 800, 300), 1.0, 3).3, 300);
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
        // An unknown mode must never steal focus.
        assert!(!show_flags("whatever").activate);
    }

    #[test]
    fn page_heights_match_the_page() {
        // Rope.svelte: BAND 178 + 4 px margin; Line.svelte `.strip`: 4 px padding top and bottom.
        assert_eq!(ROPE_HEIGHT, 178.0 + 4.0);
        assert_eq!(CHROME_HEIGHT, 4.0 + 4.0);
        assert_eq!(line_rect((0, 0, 1920, 1040), 1.0, 1).3, 190);
    }

    #[test]
    fn rope_count_from_ropes_and_clips() {
        // Enabled and non-empty only; never fewer than one (the empty message).
        assert_eq!(rope_count_of([true; 3], [true; 3]), 3);
        assert_eq!(rope_count_of([true; 3], [true, false, true]), 2);
        assert_eq!(rope_count_of([true, false, true], [true, true, false]), 1);
        assert_eq!(rope_count_of([true; 3], [false; 3]), 1);
        assert_eq!(rope_count_of([false; 3], [true; 3]), 1);
    }

    #[test]
    fn hotkey_mode_is_not_downgraded() {
        assert_eq!(merge_mode(None, "peek"), "peek");
        assert_eq!(merge_mode(Some("peek"), "hotkey"), "hotkey");
        assert_eq!(merge_mode(Some("hotkey"), "peek"), "hotkey");
        assert_eq!(merge_mode(Some("peek"), "reveal"), "reveal");
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

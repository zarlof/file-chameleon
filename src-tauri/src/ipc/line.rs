//! Commands of the clothesline and the float windows. The work is in `line` and `float`.

use super::CmdResult;
use tauri::AppHandle;

/// Show the line. The page only ever asks for "hotkey" (activating); the other modes come from
/// the clipboard watcher and the mouse hook.
#[tauri::command(async)]
pub fn line_show(app: AppHandle, mode: String) {
    crate::line::show(&app, &mode);
}

#[tauri::command]
pub fn line_hide(app: AppHandle) {
    crate::line::hide(&app);
}

/// A click inside a peeking line: take focus.
#[tauri::command]
pub fn line_activate(app: AppHandle) {
    crate::line::activate(&app);
}

/// Start an OLE drag of clip `id`; returns when the drop ends.
#[tauri::command]
pub async fn line_drag_out(app: AppHandle, id: u64) -> CmdResult<()> {
    crate::line::drag_out(&app, id).await
}

/// Double-click: open the clip in the library. Async: it may create the main window.
#[tauri::command(async)]
pub fn line_open_item(app: AppHandle, id: u64) {
    crate::line::open_item(&app, id);
}

/// Copy the clips, hide the line and paste into the app it was opened from.
#[tauri::command]
pub async fn line_paste(app: AppHandle, ids: Vec<u64>) -> CmdResult<()> {
    super::blocking(move || crate::line::paste(&app, &ids)).await?
}

/// Open a web link in the default browser (http and https only).
#[tauri::command(async)]
pub fn open_url(url: String) -> CmdResult<()> {
    crate::line::open_url(&url)
}

/// Pin images on screen in small always-on-top windows.
#[tauri::command(async)]
pub fn float_open(app: AppHandle, paths: Vec<String>) {
    crate::float::open(&app, paths.into_iter().map(Into::into).collect());
}

/// The latest requested show mode and whether the line is up: the page pulls it once its
/// listeners are attached, so a show that happened while the page was loading is not missed.
#[tauri::command]
pub fn line_state(app: AppHandle) -> crate::line::LineState {
    crate::line::state(&app)
}

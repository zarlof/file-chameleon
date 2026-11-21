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

/// The files rope: its files (newest first, thumbnails made lazily). Async: stats every file
/// and decodes the thumbnails that are not cached yet.
#[tauri::command]
pub async fn line_files(app: AppHandle) -> CmdResult<Vec<crate::files::LineFile>> {
    super::blocking(move || crate::files::list(&app)).await
}

/// Put the files on the clipboard as a file list (paste in Explorer or a chat app).
#[tauri::command]
pub async fn line_file_copy(app: AppHandle, paths: Vec<String>) -> CmdResult<()> {
    super::blocking(move || crate::line::copy_files(&app, &paths)).await?
}

/// Put the files on the clipboard, hide the line and paste into the app it was opened from.
#[tauri::command]
pub async fn line_file_paste(app: AppHandle, paths: Vec<String>) -> CmdResult<()> {
    super::blocking(move || crate::line::paste_files(&app, &paths)).await?
}

/// Put text on the clipboard without it becoming a clip (used by "Copy path").
#[tauri::command(async)]
pub fn line_copy_text(app: AppHandle, text: String) -> CmdResult<()> {
    crate::line::copy_text(&app, &text)
}

/// Open a file with its default app.
#[tauri::command(async)]
pub fn line_file_open(app: AppHandle, path: String) -> CmdResult<()> {
    crate::line::open_file(&app, &path)
}

/// Show a file in Explorer.
#[tauri::command(async)]
pub fn line_file_reveal(app: AppHandle, path: String) -> CmdResult<()> {
    crate::line::reveal_file(&app, &path)
}

/// Take files off the rope (the files themselves stay).
#[tauri::command]
pub async fn line_file_remove(app: AppHandle, paths: Vec<String>) -> CmdResult<()> {
    super::blocking(move || {
        let paths: Vec<std::path::PathBuf> = paths.into_iter().map(Into::into).collect();
        crate::files::remove(&app, &paths).map_err(super::CmdError::from)
    })
    .await?
}

/// Start an OLE drag of a file of the rope; returns when the drop ends.
#[tauri::command]
pub async fn line_file_drag_out(app: AppHandle, path: String) -> CmdResult<()> {
    crate::line::drag_out_file(&app, path.into()).await
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

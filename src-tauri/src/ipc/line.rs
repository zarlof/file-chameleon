//! Commands of the clothesline and the float windows. The work is in `line` and `float`.

use super::CmdResult;
use crate::state::AppState;
use tauri::{AppHandle, Manager};

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

/// A click on a clip card: open it (image viewer, library, browser).
#[tauri::command(async)]
pub fn line_open_clip(app: AppHandle, id: u64) -> CmdResult<()> {
    crate::line::open_clip(&app, id)
}

/// Files the wheel can convert for clips (Ctrl+click on cards): image clips give their picture,
/// text and link clips a `.txt` in the clipboard store.
#[tauri::command(async)]
pub fn line_clip_files(app: AppHandle, ids: Vec<u64>) -> CmdResult<Vec<String>> {
    crate::line::clip_files(&app, &ids)
}

/// Ctrl+click on cards: hide the line, then open the click-mode wheel for `paths`.
#[tauri::command(async)]
pub fn line_wheel(app: AppHandle, paths: Vec<String>) {
    crate::line::open_wheel(&app, paths);
}

/// Ctrl+drag on cards: the drag wheel under the cursor with the cards' files dragged onto it.
/// Returns when the drop ends.
#[tauri::command]
pub async fn line_wheel_drag(
    app: AppHandle,
    ids: Vec<u64>,
    paths: Vec<String>,
    shift: bool,
) -> CmdResult<()> {
    crate::line::wheel_drag(&app, &ids, &paths, shift).await
}

/// The Windows share sheet for clips and files of the files rope.
#[tauri::command(async)]
pub fn line_share(app: AppHandle, ids: Vec<u64>, paths: Vec<String>) -> CmdResult<()> {
    crate::line::share(&app, &ids, &paths)
}

/// Pin or unpin files of the files rope (pinned ones stay first).
#[tauri::command]
pub async fn line_file_pin(app: AppHandle, paths: Vec<String>, pinned: bool) -> CmdResult<()> {
    super::blocking(move || crate::files::pin(&app, &paths, pinned)).await?
}

/// Place a clip where it was dropped on its rope (`rank`: higher = further left; `None` = back
/// to its time order).
#[tauri::command(async)]
pub fn line_clip_rank(app: AppHandle, id: u64, rank: Option<f64>) -> CmdResult<()> {
    let state = app.state::<AppState>();
    state
        .clips()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .set_rank(id, rank)?;
    super::clipboard::changed(&app);
    Ok(())
}

/// Place a file of the files rope where it was dropped.
#[tauri::command]
pub async fn line_file_rank(app: AppHandle, path: String, rank: Option<f64>) -> CmdResult<()> {
    super::blocking(move || crate::files::set_rank(&app, &path, rank)).await?
}

/// Give cards their own colour (`#rrggbb`; `None` = the line's card colour): clips by id,
/// files by path.
#[tauri::command]
pub async fn line_card_color(
    app: AppHandle,
    ids: Vec<u64>,
    paths: Vec<String>,
    color: Option<String>,
) -> CmdResult<()> {
    super::blocking(move || -> CmdResult<()> {
        if !ids.is_empty() {
            let state = app.state::<AppState>();
            state
                .clips()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .set_color(&ids, color.as_deref())?;
            super::clipboard::changed(&app);
        }
        if !paths.is_empty() {
            crate::files::set_color(&app, &paths, color.as_deref())?;
        }
        Ok(())
    })
    .await?
}

/// Drop the own colour of every card (clips and files): they all take the line's card colour.
#[tauri::command]
pub async fn line_clear_card_colors(app: AppHandle) -> CmdResult<()> {
    super::blocking(move || -> CmdResult<()> {
        let state = app.state::<AppState>();
        state
            .clips()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear_colors()?;
        super::clipboard::changed(&app);
        crate::files::clear_colors(&app)
    })
    .await?
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

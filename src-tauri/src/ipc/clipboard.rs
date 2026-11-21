//! Clipboard, shelf and hotkey commands (v2). OWNER: Task 9. Mirrored by `src/lib/ipc.ts`.
//!
//! Single-item changes emit `clip://updated` (the item), bulk changes `clip://changed`, so the
//! main window and the shelf stay in sync whichever one made the change.

use super::{CmdError, CmdResult};
use crate::events;
use crate::state::AppState;
use fc_core::clip::history;
use fc_core::clip::{
    ClipItem, ClipKind, ClipQuery, ClipStats, ClipStore, Field, FolderInfo, TagInfo,
};
use std::path::{Path, PathBuf};
use std::sync::MutexGuard;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, State};

fn store(state: &AppState) -> MutexGuard<'_, ClipStore> {
    state.clips().lock().unwrap_or_else(|e| e.into_inner())
}

fn not_found() -> CmdError {
    CmdError::new("notFound", "clipboard item not found")
}

pub(crate) fn find(state: &AppState, id: u64) -> CmdResult<ClipItem> {
    store(state).get(id).cloned().ok_or_else(not_found)
}

fn changed(app: &AppHandle) {
    let _ = app.emit(events::CLIP_CHANGED, ());
}

fn updated(app: &AppHandle, item: &ClipItem) {
    let _ = app.emit(events::CLIP_UPDATED, item);
}

// Async (see the `ipc` module docs): the store reads and writes files, and opening it for the
// first time prunes old items.

// ---------------------------------------------------------------- items

#[tauri::command(async)]
pub fn clip_list(state: State<'_, AppState>, query: Option<ClipQuery>) -> Vec<ClipItem> {
    store(&state).query(&query.unwrap_or_default())
}

#[tauri::command(async)]
pub fn clip_get(state: State<'_, AppState>, id: u64) -> CmdResult<ClipItem> {
    find(&state, id)
}

/// Empty value clears the title/note; `text` edits text items only.
#[tauri::command(async)]
pub fn clip_set_field(
    app: AppHandle,
    state: State<'_, AppState>,
    id: u64,
    field: Field,
    value: String,
) -> CmdResult<ClipItem> {
    let item = store(&state).set_field(id, field, &value)?;
    updated(&app, &item);
    Ok(item)
}

#[tauri::command(async)]
pub fn clip_pin(
    app: AppHandle,
    state: State<'_, AppState>,
    ids: Vec<u64>,
    pinned: bool,
) -> CmdResult<()> {
    store(&state).set_pinned(&ids, pinned)?;
    changed(&app);
    Ok(())
}

#[tauri::command(async)]
pub fn clip_trash(app: AppHandle, state: State<'_, AppState>, ids: Vec<u64>) -> CmdResult<()> {
    store(&state).trash(&ids, fc_core::now_ms())?;
    changed(&app);
    Ok(())
}

#[tauri::command(async)]
pub fn clip_restore(app: AppHandle, state: State<'_, AppState>, ids: Vec<u64>) -> CmdResult<()> {
    store(&state).restore(&ids)?;
    changed(&app);
    Ok(())
}

#[tauri::command(async)]
pub fn clip_delete_forever(
    app: AppHandle,
    state: State<'_, AppState>,
    ids: Vec<u64>,
) -> CmdResult<()> {
    store(&state).delete_forever(&ids)?;
    changed(&app);
    Ok(())
}

#[tauri::command(async)]
pub fn clip_empty_trash(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    store(&state).empty_trash()?;
    changed(&app);
    Ok(())
}

/// Trash every unsaved live item; returns how many.
#[tauri::command(async)]
pub fn clip_clear_unsaved(app: AppHandle, state: State<'_, AppState>) -> CmdResult<usize> {
    let n = store(&state).clear_unsaved(fc_core::now_ms())?;
    changed(&app);
    Ok(n)
}

#[tauri::command(async)]
pub fn clip_stats(state: State<'_, AppState>) -> ClipStats {
    store(&state).stats()
}

// ---------------------------------------------------------------- folders and tags

#[tauri::command(async)]
pub fn clip_folders(state: State<'_, AppState>) -> Vec<FolderInfo> {
    store(&state).folders()
}

#[tauri::command(async)]
pub fn clip_folder_create(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    parent: Option<u64>,
) -> CmdResult<FolderInfo> {
    let f = store(&state).folder_create(&name, parent)?;
    changed(&app);
    Ok(FolderInfo {
        id: f.id,
        name: f.name,
        parent: f.parent,
        count: 0,
    })
}

#[tauri::command(async)]
pub fn clip_folder_rename(
    app: AppHandle,
    state: State<'_, AppState>,
    id: u64,
    name: String,
) -> CmdResult<()> {
    store(&state).folder_rename(id, &name)?;
    changed(&app);
    Ok(())
}

/// Items in it become unfiled; sub-folders move up.
#[tauri::command(async)]
pub fn clip_folder_delete(app: AppHandle, state: State<'_, AppState>, id: u64) -> CmdResult<()> {
    store(&state).folder_delete(id)?;
    changed(&app);
    Ok(())
}

#[tauri::command(async)]
pub fn clip_move(
    app: AppHandle,
    state: State<'_, AppState>,
    ids: Vec<u64>,
    folder: Option<u64>,
) -> CmdResult<()> {
    store(&state).move_to(&ids, folder)?;
    changed(&app);
    Ok(())
}

#[tauri::command(async)]
pub fn clip_tags(state: State<'_, AppState>) -> Vec<TagInfo> {
    store(&state).tags()
}

#[tauri::command(async)]
pub fn clip_tag(
    app: AppHandle,
    state: State<'_, AppState>,
    ids: Vec<u64>,
    add: Vec<String>,
    remove: Vec<String>,
) -> CmdResult<()> {
    store(&state).tag(&ids, &add, &remove)?;
    changed(&app);
    Ok(())
}

// ---------------------------------------------------------------- copy

/// What `clip_copy` puts on the clipboard for a selection.
#[derive(Debug, PartialEq)]
pub enum CopyPlan {
    Text(String),
    Image(PathBuf),
    /// `CF_HDROP` file list.
    Files(Vec<PathBuf>),
    Nothing,
}

/// The file that represents an image item: the edited copy when there is one.
fn image_file(item: &ClipItem) -> Option<PathBuf> {
    (item.kind == ClipKind::Image)
        .then(|| item.edited.clone().or_else(|| item.image.clone()))
        .flatten()
}

/// One text -> that text; one image -> that image; several, all text-like -> joined with `\n`
/// (in the given order); otherwise -> the image files (edited copies where present) as a file
/// list. Items without content are skipped.
pub fn copy_plan(items: &[ClipItem]) -> CopyPlan {
    let text_like = |i: &ClipItem| i.kind != ClipKind::Image;
    if !items.is_empty() && items.iter().all(text_like) {
        let texts: Vec<&str> = items.iter().filter_map(|i| i.text.as_deref()).collect();
        return if texts.is_empty() {
            CopyPlan::Nothing
        } else {
            CopyPlan::Text(texts.join("\n"))
        };
    }
    let files: Vec<PathBuf> = items.iter().filter_map(image_file).collect();
    match (items.len(), files.len()) {
        (_, 0) => CopyPlan::Nothing,
        (1, _) => CopyPlan::Image(files.into_iter().next().unwrap_or_default()),
        _ => CopyPlan::Files(files),
    }
}

/// Put the items on the clipboard as [`copy_plan`] says; our write is not recorded again.
pub fn copy_items(state: &AppState, ids: &[u64]) -> CmdResult<()> {
    let items: Vec<ClipItem> = {
        let s = store(state);
        ids.iter().filter_map(|id| s.get(*id).cloned()).collect()
    };
    let written = match copy_plan(&items) {
        CopyPlan::Nothing => return Err(not_found()),
        CopyPlan::Text(t) => crate::platform::clipboard::write_text(&t),
        CopyPlan::Image(p) => std::fs::read(&p)
            .map_err(|e| e.to_string())
            .and_then(|png| crate::platform::clipboard::write_image(&png)),
        CopyPlan::Files(paths) => crate::platform::clipboard::write_files(&paths),
    };
    mark_self_write(state, written)
}

fn mark_self_write(state: &AppState, written: Result<u32, String>) -> CmdResult<()> {
    match written {
        Ok(seq) => {
            state.clip_self_write.store(seq, Ordering::SeqCst);
            Ok(())
        }
        Err(e) => Err(CmdError::new("clipboard", e)),
    }
}

/// One text/image -> that; several texts -> joined; images -> a file list (`CF_HDROP`).
/// Async: an image is decoded and converted to a DIB (up to 25 MP).
#[tauri::command]
pub async fn clip_copy(app: AppHandle, ids: Vec<u64>) -> CmdResult<()> {
    super::blocking(move || copy_items(&app.state::<AppState>(), &ids)).await?
}

/// Copy the OCR text of an image item.
#[tauri::command(async)]
pub fn clip_copy_ocr(state: State<'_, AppState>, id: u64) -> CmdResult<()> {
    let item = find(&state, id)?;
    let text = item
        .ocr
        .filter(|t| !t.trim().is_empty())
        .ok_or_else(|| CmdError::new("notFound", "no recognized text"))?;
    mark_self_write(&state, crate::platform::clipboard::write_text(&text))
}

/// Write the items as files into `dir`; returns how many files.
#[tauri::command]
pub async fn clip_export(app: AppHandle, ids: Vec<u64>, dir: String) -> CmdResult<usize> {
    super::blocking(move || {
        // Copies of the items: the files are written without holding the store.
        let state = app.state::<AppState>();
        let items: Vec<ClipItem> = {
            let s = store(&state);
            ids.iter().filter_map(|id| s.get(*id).cloned()).collect()
        };
        Ok(history::export_items(&items, Path::new(&dir))?)
    })
    .await?
}

// ---------------------------------------------------------------- images

/// Save an image item as `format` ("png" | "jpg" | "webp") into the Downloads folder; returns the path.
/// Async: decodes and re-encodes the image.
#[tauri::command]
pub async fn clip_save_image(app: AppHandle, id: u64, format: String) -> CmdResult<String> {
    super::blocking(move || {
        save_image(&app.state::<AppState>(), id, &format).map(|p| p.to_string_lossy().into_owned())
    })
    .await?
}

/// Write image item `id` (its edited copy when there is one) as `format` into the Downloads
/// folder as `Clipboard <date time>.<ext>`. Shared by `clip_save_image` and the clipImage
/// toast's "Save as PNG".
pub fn save_image(state: &AppState, id: u64, format: &str) -> CmdResult<PathBuf> {
    let ext = match format {
        "png" => "png",
        "jpg" | "jpeg" => "jpg",
        "webp" => "webp",
        other => {
            return Err(CmdError::new(
                "unsupported",
                format!("image output format: {other}"),
            ));
        }
    };
    let item = find(state, id)?;
    let src = image_file(&item).ok_or_else(|| CmdError::new("notFound", "not an image item"))?;
    let dir = state.default_download_dir();
    std::fs::create_dir_all(&dir).map_err(fc_core::Error::from)?;
    let out = unique_output(&dir, ext);
    history::export_png(&src, ext, &out)?;
    Ok(out)
}

/// Store an edited copy (PNG data URL) of an image item; the original is kept.
/// Async: decodes the PNG and writes it with a thumbnail.
#[tauri::command]
pub async fn clip_save_edited(app: AppHandle, id: u64, data_url: String) -> CmdResult<ClipItem> {
    super::blocking(move || {
        let item = save_edited_data_url(app.state::<AppState>().clips(), id, &data_url)?;
        updated(&app, &item);
        // An edited image peeks in on the clothesline like a new copy.
        crate::line::peek(&app);
        Ok(item)
    })
    .await?
}

/// Decode a PNG data URL (or bare base64) and store it as the edited copy of image item `id`.
/// The base64 is decoded before the store is locked.
fn save_edited_data_url(
    store: &std::sync::Mutex<ClipStore>,
    id: u64,
    data_url: &str,
) -> CmdResult<ClipItem> {
    let png = fc_core::b64::decode(data_url)?;
    let mut store = store.lock().unwrap_or_else(|e| e.into_inner());
    Ok(store.save_edited(id, &png)?)
}

#[tauri::command(async)]
pub fn clip_revert_edit(
    app: AppHandle,
    state: State<'_, AppState>,
    id: u64,
) -> CmdResult<ClipItem> {
    let item = store(&state).revert_edit(id)?;
    updated(&app, &item);
    Ok(item)
}

/// `Clipboard <yyyy-mm-dd HH.MM.SS>.<ext>` in `dir`, numbered if it already exists.
fn unique_output(dir: &Path, ext: &str) -> PathBuf {
    // A dummy input name (different extension, so the stem keeps the dots of the time).
    let stem = format!("Clipboard {}", local_stamp());
    let input = PathBuf::from(format!("{stem}.clip"));
    fc_core::jobs::naming::output_path(&input, ext, Some(dir), None)
}

/// Local time as `yyyy-mm-dd HH.MM.SS` (Windows locale APIs; falls back to the Unix time).
pub(crate) fn local_stamp() -> String {
    #[cfg(windows)]
    {
        use windows::Win32::Globalization::{
            ENUM_DATE_FORMATS_FLAGS, GetDateFormatEx, GetTimeFormatEx, TIME_FORMAT_FLAGS,
        };
        use windows::core::{PCWSTR, w};
        let mut date = [0u16; 32];
        let mut time = [0u16; 32];
        let (nd, nt) = unsafe {
            (
                GetDateFormatEx(
                    w!("en-US"),
                    ENUM_DATE_FORMATS_FLAGS(0),
                    None,
                    w!("yyyy-MM-dd"),
                    Some(&mut date),
                    PCWSTR::null(),
                ),
                GetTimeFormatEx(
                    w!("en-US"),
                    TIME_FORMAT_FLAGS(0),
                    None,
                    w!("HH.mm.ss"),
                    Some(&mut time),
                ),
            )
        };
        if nd > 1 && nt > 1 {
            let d = String::from_utf16_lossy(&date[..(nd - 1) as usize]);
            let t = String::from_utf16_lossy(&time[..(nt - 1) as usize]);
            return format!("{d} {t}");
        }
    }
    (fc_core::now_ms() / 1000).to_string()
}

// ---------------------------------------------------------------- pause

/// Event (bool payload) emitted whenever the pause state changes, from any source.
pub const CLIP_PAUSED: &str = "clip://paused";

/// Change the pause state, keep it in the settings (it survives a restart) and tell the
/// webviews. Shared by the command and the tray.
pub fn set_paused(app: &AppHandle, paused: bool) {
    let state = app.state::<AppState>();
    state.clip_paused.store(paused, Ordering::SeqCst);
    state.update_settings(|s| s.clipboard_paused = paused);
    let _ = app.emit(CLIP_PAUSED, paused);
}

/// Async: writes the settings file.
#[tauri::command(async)]
pub fn clip_set_paused(app: AppHandle, paused: bool) {
    set_paused(&app, paused);
    // Keep the tray's check mark in sync.
    crate::tray::refresh(&app);
}

#[tauri::command]
pub fn clip_get_paused(state: State<'_, AppState>) -> bool {
    state.clip_paused.load(Ordering::SeqCst)
}

// ---------------------------------------------------------------- shelf and hotkeys

/// Async: the first call creates the shelf webview.
#[tauri::command(async)]
pub fn show_shelf(app: AppHandle) {
    crate::shelf::show_shelf(&app);
}

#[tauri::command(async)]
pub fn shelf_hide(app: AppHandle) {
    crate::shelf::hide(&app);
}

/// Copy the items, hide the shelf and paste (Ctrl+V) into the app that was active before; when
/// that app is gone the items just stay on the clipboard.
#[tauri::command]
pub async fn shelf_paste(app: AppHandle, ids: Vec<u64>) -> CmdResult<()> {
    super::blocking(move || {
        copy_items(&app.state::<AppState>(), &ids)?;
        crate::shelf::paste_into_previous(&app);
        Ok(())
    })
    .await?
}

/// Async: waits (at most 0.5 s) for a re-registration that is still running.
#[tauri::command(async)]
pub fn hotkey_status() -> Vec<crate::platform::hotkeys::HotkeyStatus> {
    crate::platform::hotkeys::status()
}

#[cfg(test)]
mod helper_tests {
    use super::{CopyPlan, copy_plan};
    use fc_core::clip::ClipItem;
    use serde_json::json;
    use std::path::PathBuf;

    fn text(id: u64, kind: &str, t: &str) -> ClipItem {
        serde_json::from_value(json!({
            "id": id, "kind": kind, "text": t, "hash": "h", "createdAt": 1
        }))
        .unwrap()
    }

    fn image(id: u64, edited: Option<&str>) -> ClipItem {
        serde_json::from_value(json!({
            "id": id, "kind": "image", "image": format!("C:/clip/img/{id}.png"),
            "edited": edited, "hash": "h", "createdAt": 1
        }))
        .unwrap()
    }

    #[test]
    fn copy_plan_cases() {
        assert_eq!(copy_plan(&[]), CopyPlan::Nothing);
        assert_eq!(
            copy_plan(&[text(1, "text", "hello")]),
            CopyPlan::Text("hello".into())
        );
        assert_eq!(
            copy_plan(&[text(1, "link", "https://a.com")]),
            CopyPlan::Text("https://a.com".into())
        );
        // One image: the edited copy wins when there is one.
        assert_eq!(
            copy_plan(&[image(3, None)]),
            CopyPlan::Image("C:/clip/img/3.png".into())
        );
        assert_eq!(
            copy_plan(&[image(3, Some("C:/clip/img/3.edit.png"))]),
            CopyPlan::Image("C:/clip/img/3.edit.png".into())
        );
        // Several text-like items: joined with newlines, in the given order.
        assert_eq!(
            copy_plan(&[
                text(2, "text", "b"),
                text(1, "mediaLink", "https://youtu.be/x"),
                text(5, "text", "a"),
            ]),
            CopyPlan::Text("b\nhttps://youtu.be/x\na".into())
        );
        // Images (alone or mixed with text): a file list of the images, each one's edited copy
        // when there is one.
        assert_eq!(
            copy_plan(&[
                image(1, None),
                text(2, "text", "x"),
                image(4, Some("C:/clip/img/4.edit.png")),
            ]),
            CopyPlan::Files(vec![
                PathBuf::from("C:/clip/img/1.png"),
                PathBuf::from("C:/clip/img/4.edit.png"),
            ])
        );
        // An image without a file is nothing to copy.
        let mut broken = image(9, None);
        broken.image = None;
        assert_eq!(copy_plan(&[broken]), CopyPlan::Nothing);
    }

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut out = Vec::new();
        image::RgbaImage::from_pixel(w, h, image::Rgba([1, 2, 3, 255]))
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    #[test]
    fn save_edited_decodes_a_png_data_url() {
        let dir = std::env::temp_dir().join(format!(
            "fc-edit-test-{}-{}",
            std::process::id(),
            fc_core::now_ms()
        ));
        let mut store = fc_core::clip::ClipStore::open(&dir).unwrap();
        let item = store.add_image_png(&png(8, 6), 8, 6, 1).unwrap().unwrap();
        let text = store.add_text("not an image", 2).unwrap().unwrap();
        let url = format!("data:image/png;base64,{}", fc_core::b64::encode(&png(4, 3)));
        let store = std::sync::Mutex::new(store);
        let saved = super::save_edited_data_url(&store, item.id, &url).unwrap();
        let edited = saved.edited.clone().unwrap();
        assert_eq!(std::fs::read(&edited).unwrap(), png(4, 3));
        assert_eq!(saved.image, item.image);
        // Garbage, a non-PNG payload and a text item are errors, and nothing changes.
        assert!(super::save_edited_data_url(&store, item.id, "data:image/png;base64,@@").is_err());
        let not_png = format!("data:image/png;base64,{}", fc_core::b64::encode(b"hello"));
        assert!(super::save_edited_data_url(&store, item.id, &not_png).is_err());
        assert!(super::save_edited_data_url(&store, text.id, &url).is_err());
        assert_eq!(std::fs::read(&edited).unwrap(), png(4, 3));
        drop(store);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::local_stamp;

    #[test]
    fn local_stamp_has_expected_shape() {
        let s = local_stamp();
        let b = s.as_bytes();
        assert_eq!(s.len(), 19, "{s}");
        assert_eq!(
            (b[4], b[7], b[10], b[13], b[16]),
            (b'-', b'-', b' ', b'.', b'.')
        );
    }
}

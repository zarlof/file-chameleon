//! Clipboard commands. OWNER: Task G. Replace the stub bodies; keep names and arguments.

use super::{CmdError, CmdResult};
use crate::state::AppState;
use fc_core::clip::history;
use fc_core::clip::{ClipItem, ClipKind, ClipStore};
use std::path::{Path, PathBuf};
use std::sync::MutexGuard;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, State};

fn store(state: &AppState) -> MutexGuard<'_, ClipStore> {
    state.clips().lock().unwrap_or_else(|e| e.into_inner())
}

fn find(state: &AppState, id: u64) -> CmdResult<ClipItem> {
    store(state)
        .items()
        .iter()
        .find(|i| i.id == id)
        .cloned()
        .ok_or_else(|| CmdError::new("notFound", "clipboard item not found"))
}

#[tauri::command]
pub fn clip_list(state: State<'_, AppState>, query: Option<String>) -> Vec<ClipItem> {
    store(&state).search(query.as_deref().unwrap_or(""))
}

#[tauri::command]
pub fn clip_pin(state: State<'_, AppState>, id: u64, pinned: bool) -> CmdResult<bool> {
    Ok(store(&state).set_pinned(id, pinned)?)
}

#[tauri::command]
pub fn clip_delete(state: State<'_, AppState>, id: u64) -> CmdResult<bool> {
    Ok(store(&state).remove(id)?)
}

#[tauri::command]
pub fn clip_clear(state: State<'_, AppState>) -> CmdResult<()> {
    Ok(store(&state).clear_unpinned()?)
}

/// Put an item back on the clipboard (not re-recorded as a new item).
#[tauri::command]
pub fn clip_copy(app: AppHandle, state: State<'_, AppState>, id: u64) -> CmdResult<()> {
    let _ = app;
    let item = find(&state, id)?;
    // Mark first: the listener skips the change our own write causes.
    state.clip_self_write.store(true, Ordering::SeqCst);
    let written = match item.kind {
        ClipKind::Image => item
            .image
            .as_deref()
            .ok_or_else(|| "image file missing".to_string())
            .and_then(|p| std::fs::read(p).map_err(|e| e.to_string()))
            .and_then(|png| crate::platform::clipboard::write_image(&png)),
        _ => match item.text.as_deref() {
            Some(t) => crate::platform::clipboard::write_text(t),
            None => Err("item has no text".to_string()),
        },
    };
    if let Err(e) = written {
        state.clip_self_write.store(false, Ordering::SeqCst);
        return Err(CmdError::new("clipboard", e));
    }
    Ok(())
}

/// Save an image item as `format` ("png" | "jpg" | "webp") into the Downloads folder; returns the path.
#[tauri::command]
pub fn clip_save_image(state: State<'_, AppState>, id: u64, format: String) -> CmdResult<String> {
    let ext = match format.as_str() {
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
    let item = find(&state, id)?;
    let src = item
        .image
        .filter(|_| item.kind == ClipKind::Image)
        .ok_or_else(|| CmdError::new("notFound", "not an image item"))?;
    let dir = state.default_download_dir();
    std::fs::create_dir_all(&dir).map_err(fc_core::Error::from)?;
    let out = unique_output(&dir, ext);
    history::export_png(&src, ext, &out)?;
    Ok(out.to_string_lossy().into_owned())
}

/// `Clipboard <yyyy-mm-dd HH.MM.SS>.<ext>` in `dir`, numbered if it already exists.
fn unique_output(dir: &Path, ext: &str) -> PathBuf {
    // A dummy input name (different extension, so the stem keeps the dots of the time).
    let stem = format!("Clipboard {}", local_stamp());
    let input = PathBuf::from(format!("{stem}.clip"));
    fc_core::jobs::naming::output_path(&input, ext, Some(dir), None)
}

/// Local time as `yyyy-mm-dd HH.MM.SS` (Windows locale APIs; falls back to the Unix time).
fn local_stamp() -> String {
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

#[tauri::command]
pub fn clip_set_paused(state: State<'_, AppState>, paused: bool) {
    state.clip_paused.store(paused, Ordering::SeqCst);
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

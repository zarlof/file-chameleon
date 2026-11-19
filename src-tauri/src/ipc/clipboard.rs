//! Clipboard commands. OWNER: Task G. Replace the stub bodies; keep names and arguments.

use super::{CmdError, CmdResult};
use crate::state::AppState;
use fc_core::clip::ClipItem;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, State};

#[tauri::command]
pub fn clip_list(state: State<'_, AppState>, query: Option<String>) -> Vec<ClipItem> {
    let _ = (state, query);
    vec![]
}

#[tauri::command]
pub fn clip_pin(state: State<'_, AppState>, id: u64, pinned: bool) -> CmdResult<bool> {
    let _ = (state, id, pinned);
    Err(CmdError::new("unsupported", "not implemented yet"))
}

#[tauri::command]
pub fn clip_delete(state: State<'_, AppState>, id: u64) -> CmdResult<bool> {
    let _ = (state, id);
    Err(CmdError::new("unsupported", "not implemented yet"))
}

#[tauri::command]
pub fn clip_clear(state: State<'_, AppState>) -> CmdResult<()> {
    let _ = state;
    Err(CmdError::new("unsupported", "not implemented yet"))
}

/// Put an item back on the clipboard (not re-recorded as a new item).
#[tauri::command]
pub fn clip_copy(app: AppHandle, state: State<'_, AppState>, id: u64) -> CmdResult<()> {
    let _ = (app, state, id);
    Err(CmdError::new("unsupported", "not implemented yet"))
}

/// Save an image item as `format` ("png" | "jpg" | "webp") into the Downloads folder; returns the path.
#[tauri::command]
pub fn clip_save_image(state: State<'_, AppState>, id: u64, format: String) -> CmdResult<String> {
    let _ = (state, id, format);
    Err(CmdError::new("unsupported", "not implemented yet"))
}

#[tauri::command]
pub fn clip_set_paused(state: State<'_, AppState>, paused: bool) {
    state.clip_paused.store(paused, Ordering::SeqCst);
}

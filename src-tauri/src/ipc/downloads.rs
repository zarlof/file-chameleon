//! Download commands and the in-memory download list. OWNER: Task F.
//! Replace the stub bodies; keep the command names and argument names.

use super::{CmdError, CmdResult};
use crate::state::AppState;
use fc_core::download::{DownloadRequest, DownloadSnapshot, MediaInfo};
use tauri::{AppHandle, State};

/// Active + recent downloads. Task F decides the internals.
#[derive(Default)]
pub struct Downloads {}

#[tauri::command]
pub async fn probe_url(app: AppHandle, url: String) -> CmdResult<MediaInfo> {
    let _ = (app, url);
    Err(CmdError::new("unsupported", "not implemented yet"))
}

#[tauri::command]
pub fn start_download(
    app: AppHandle,
    state: State<'_, AppState>,
    req: DownloadRequest,
) -> CmdResult<u64> {
    let _ = (app, state, req);
    Err(CmdError::new("unsupported", "not implemented yet"))
}

#[tauri::command]
pub fn cancel_download(state: State<'_, AppState>, id: u64) -> bool {
    let _ = (state, id);
    false
}

#[tauri::command]
pub fn list_downloads(state: State<'_, AppState>) -> Vec<DownloadSnapshot> {
    let _ = state;
    vec![]
}

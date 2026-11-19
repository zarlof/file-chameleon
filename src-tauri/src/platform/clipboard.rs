//! Windows clipboard listener for the smart clipboard. OWNER: Task G.
//! `start` spawns the listener thread; `set_enabled` turns capture on/off at runtime.

use tauri::AppHandle;

pub fn start(app: &AppHandle) {
    let _ = app;
}

pub fn set_enabled(app: &AppHandle, enabled: bool) {
    let _ = (app, enabled);
}

//! Platform integration: mouse hook (wheel), clipboard listener, autostart, OS language.

pub mod clipboard;
#[cfg(windows)]
mod hook;
#[cfg(windows)]
pub mod win;

use crate::state::AppState;
use fc_core::settings::Settings;
use tauri::{AppHandle, Manager};

pub fn start(app: &AppHandle) {
    let s = app.state::<AppState>().settings();
    configure_wheel(app, &s);
    if s.clipboard_enabled {
        clipboard::start(app);
    }
}

fn configure_wheel(app: &AppHandle, s: &Settings) {
    #[cfg(windows)]
    {
        hook::set_config(s.wheel_enabled, s.wheel_any_app);
        if s.wheel_enabled {
            let app = app.clone();
            hook::ensure_started(move |ev| match ev {
                hook::HookEvent::DragStart {
                    x,
                    y,
                    down_x,
                    down_y,
                } => {
                    if hook::source_allowed(down_x, down_y) {
                        let a = app.clone();
                        let _ =
                            app.run_on_main_thread(move || crate::windows::show_wheel_at(&a, x, y));
                    }
                }
                hook::HookEvent::Released => {
                    let a = app.clone();
                    let _ = app.run_on_main_thread(move || crate::windows::on_drag_released(&a));
                }
            });
        }
    }
    #[cfg(not(windows))]
    let _ = (app, s);
}

/// Apply settings that have side effects outside the webview. Returns the settings to store:
/// `new`, except that a failed autostart change keeps the old value.
pub fn apply_settings(app: &AppHandle, old: &Settings, mut new: Settings) -> Settings {
    if old.launch_at_login != new.launch_at_login {
        use tauri_plugin_autostart::ManagerExt;
        let al = app.autolaunch();
        let result = if new.launch_at_login {
            al.enable()
        } else {
            al.disable()
        };
        if let Err(e) = result {
            eprintln!("autostart: {e}");
            new.launch_at_login = old.launch_at_login;
        }
    }
    if old.wheel_enabled != new.wheel_enabled || old.wheel_any_app != new.wheel_any_app {
        configure_wheel(app, &new);
    }
    if old.clipboard_enabled != new.clipboard_enabled {
        if new.clipboard_enabled {
            clipboard::start(app);
        }
        clipboard::set_enabled(app, new.clipboard_enabled);
    }
    if old.clipboard_retention_images != new.clipboard_retention_images
        || old.clipboard_max_items != new.clipboard_max_items
    {
        // Apply the new limits now, not on the next copy.
        let state = app.state::<AppState>();
        let mut store = state.clips().lock().unwrap_or_else(|e| e.into_inner());
        let _ = store.prune(
            fc_core::now_ms(),
            new.clipboard_retention_images,
            new.clipboard_max_items as usize,
        );
    }
    if old.language != new.language {
        crate::tray::refresh(app);
    }
    new
}

pub fn system_language() -> String {
    #[cfg(windows)]
    return win::system_language();
    #[cfg(not(windows))]
    "en".into()
}

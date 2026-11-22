//! Platform integration: mouse hook (wheel), clipboard listener, global hotkeys, OCR,
//! autostart, OS language.

pub mod clipboard;
pub mod contextmenu;
#[cfg(windows)]
mod hook;
#[cfg(windows)]
pub mod win;
// Clipboard v2 (Task 9): global hotkeys and Windows OCR.
pub mod hotkeys;
pub mod ocr;
pub mod share;

use crate::state::AppState;
use fc_core::settings::Settings;
use tauri::{AppHandle, Manager};

pub fn start(app: &AppHandle) {
    let s = app.state::<AppState>().settings();
    configure_hooks(app, &s);
    if s.clipboard_enabled {
        clipboard::start(app);
    }
    // Rewrite the Explorer verb: the exe may have moved since it was written.
    if s.context_menu
        && let Err(e) = contextmenu::set_enabled(true, &ui_language(&s.language))
    {
        eprintln!("context menu: {e}");
    }
    // Clipboard v2 (Task 9): hotkeys, OCR, watched image folders, retention.
    hotkeys::configure(app, &s);
    ocr::set_enabled(s.clipboard_ocr);
    clipboard::start_maintenance(app);
    // Watched folders (clipboard image folders, Task 10 watch folders): the first scan reads
    // every file's metadata, and an unreachable network path can stall for a long time, so it
    // never runs on the main thread.
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("fc-folders-init".into())
        .spawn(move || {
            clipboard::configure_folders(&app, &s);
            crate::watchfolders::configure(&app, &s);
        });
    if let Err(e) = spawned {
        eprintln!("folder watchers: {e}");
    }
}

fn configure_hooks(app: &AppHandle, s: &Settings) {
    #[cfg(windows)]
    {
        hook::set_config(s.wheel_enabled, s.wheel_any_app);
        hook::set_flip(s.wheel_flip_scroll, s.wheel_flip_tab);
        let edge = s.line_enabled && s.line_edge_reveal;
        if edge {
            hook::set_monitors(&win::monitors(), win::display_signature());
        }
        hook::set_line_edge(edge, s.line_edge == "bottom");
        if s.wheel_enabled || edge {
            ensure_hooks(app);
        }
    }
    #[cfg(not(windows))]
    let _ = (app, s);
}

// ---- wheel v2 (Task 6)

/// Start the global input hooks (idempotent). Ctrl+drag needs them, and so does the click-mode
/// wheel even when the Ctrl+drag trigger is off (Esc, Tab, clicks outside it).
pub fn ensure_hooks(app: &AppHandle) {
    #[cfg(windows)]
    {
        let app = app.clone();
        hook::ensure_started(move |ev| match ev {
            hook::HookEvent::DragStart {
                x,
                y,
                down_x,
                down_y,
                shift,
            } => {
                let excluded = app.state::<AppState>().settings().wheel_excluded_apps;
                if hook::source_allowed(down_x, down_y, &excluded) {
                    let a = app.clone();
                    let _ = app
                        .run_on_main_thread(move || crate::windows::show_wheel_at(&a, x, y, shift));
                }
            }
            hook::HookEvent::Released => {
                let a = app.clone();
                let _ = app.run_on_main_thread(move || crate::windows::on_drag_released(&a));
            }
            hook::HookEvent::Flip => crate::windows::wheel_flip(&app),
            hook::HookEvent::Close => crate::windows::wheel_close(&app),
            hook::HookEvent::EdgeEnter { x, y } => crate::line::edge_enter(&app, x, y),
            hook::HookEvent::EdgeLeave => crate::line::edge_leave(),
            hook::HookEvent::Displays => {
                hook::set_monitors(&win::monitors(), win::display_signature());
            }
        });
    }
    #[cfg(not(windows))]
    let _ = app;
}

/// Tell the hooks whether the wheel is open (they only act on input then) and where the
/// overlay window is: (left, top, right, bottom) in physical pixels.
pub fn set_wheel_open(open: bool, click: bool, rect: (i32, i32, i32, i32)) {
    #[cfg(windows)]
    hook::set_wheel_open(open, click, rect);
    #[cfg(not(windows))]
    let _ = (open, click, rect);
}

/// The UI language for a `language` setting: "tr" or "en" ("system" asks Windows).
pub fn ui_language(setting: &str) -> String {
    match setting {
        "tr" | "en" => setting.to_string(),
        _ => system_language(),
    }
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
    if old.wheel_enabled != new.wheel_enabled
        || old.wheel_any_app != new.wheel_any_app
        || old.wheel_flip_scroll != new.wheel_flip_scroll
        || old.wheel_flip_tab != new.wheel_flip_tab
        || old.line_enabled != new.line_enabled
        || old.line_edge_reveal != new.line_edge_reveal
        || old.line_edge != new.line_edge
    {
        configure_hooks(app, &new);
    }
    // Explorer verb: written when turned on, removed when off, relabelled on a language change.
    // After a failure the setting says what the registry has: a failed write removed the key
    // (off), a failed removal left it there (on).
    let verb_changed =
        old.context_menu != new.context_menu || (new.context_menu && old.language != new.language);
    if verb_changed
        && let Err(e) = contextmenu::set_enabled(new.context_menu, &ui_language(&new.language))
    {
        eprintln!("context menu: {e}");
        new.context_menu = !new.context_menu;
    }
    if old.clipboard_enabled != new.clipboard_enabled {
        if new.clipboard_enabled {
            clipboard::start(app);
        }
        clipboard::set_enabled(app, new.clipboard_enabled);
    }
    if old.clipboard_retention_text != new.clipboard_retention_text
        || old.clipboard_retention_links != new.clipboard_retention_links
        || old.clipboard_retention_images != new.clipboard_retention_images
        || old.clipboard_max_items != new.clipboard_max_items
    {
        // Apply the new limits now, not on the next copy.
        clipboard::prune(app, &new);
    }
    // Clipboard v2 (Task 9).
    if old.hotkey_library != new.hotkey_library
        || old.hotkey_download_link != new.hotkey_download_link
        || old.hotkey_line != new.hotkey_line
        || old.hotkey_line_search != new.hotkey_line_search
        || old.hotkey_capture != new.hotkey_capture
        || old.hotkey_copy_latest_image != new.hotkey_copy_latest_image
        || old.line_enabled != new.line_enabled
        || old.clipboard_enabled != new.clipboard_enabled
    {
        hotkeys::configure(app, &new);
    }
    // Clothesline: turning it off closes and frees its window.
    if old.line_enabled && !new.line_enabled {
        crate::line::destroy(app);
    }
    // The window is resized when the size setting changes while it is up.
    if old.line_size != new.line_size
        || old.line_ropes != new.line_ropes
        || old.line_edge != new.line_edge
        || old.line_files_sources != new.line_files_sources
    {
        crate::line::apply_size(app, &new);
    }
    // Link titles just turned on: catch up on thumbnails of existing YouTube links.
    if !old.clipboard_link_titles && new.clipboard_link_titles && new.clipboard_enabled {
        let (app, new) = (app.clone(), new.clone());
        let _ = std::thread::Builder::new()
            .name("fc-thumb-backfill".into())
            .spawn(move || clipboard::backfill_thumbs(&app, &new));
    }
    if old.clipboard_ocr != new.clipboard_ocr {
        ocr::set_enabled(new.clipboard_ocr);
        if new.clipboard_ocr {
            ocr::backfill(app);
        }
    }
    if old.clipboard_watch_folders != new.clipboard_watch_folders
        || old.clipboard_enabled != new.clipboard_enabled
    {
        clipboard::configure_folders(app, &new);
    }
    // Watch folders (Task 10).
    if old.watch_folders != new.watch_folders {
        crate::watchfolders::configure(app, &new);
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

/// Is the pointer in the clothesline's edge reveal zone (as of the last mouse move)?
pub fn hook_pointer_in_zone() -> bool {
    #[cfg(windows)]
    return hook::pointer_in_zone();
    #[cfg(not(windows))]
    false
}

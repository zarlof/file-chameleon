//! System tray icon and menu (the app's home while no window is open).

use crate::state::AppState;
use std::sync::atomic::Ordering;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

const TRAY_ID: &str = "main-tray";

struct Labels {
    open: &'static str,
    clipboard: &'static str,
    pause: &'static str,
    quit: &'static str,
}

fn labels(app: &AppHandle) -> Labels {
    let lang = app.state::<AppState>().settings().language;
    let lang = if lang == "system" {
        crate::platform::system_language()
    } else {
        lang
    };
    if lang == "tr" {
        Labels {
            open: "File Chameleon'ı Aç",
            clipboard: "Pano",
            pause: "Panoyu Duraklat",
            quit: "Çıkış",
        }
    } else {
        Labels {
            open: "Open File Chameleon",
            clipboard: "Clipboard",
            pause: "Pause Clipboard",
            quit: "Quit",
        }
    }
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let l = labels(app);
    let paused = app.state::<AppState>().clip_paused.load(Ordering::SeqCst);
    Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "open", l.open, true, None::<&str>)?,
            &MenuItem::with_id(app, "clipboard", l.clipboard, true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &CheckMenuItem::with_id(app, "pause", l.pause, true, paused, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", l.quit, true, None::<&str>)?,
        ],
    )
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(app.default_window_icon().cloned().expect("bundle icon"))
        .tooltip("File Chameleon")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => crate::windows::show_main(app, None),
            "clipboard" => crate::windows::show_main(
                app,
                Some(crate::events::NavPayload {
                    view: "clipboard".into(),
                    paths: vec![],
                    url: None,
                }),
            ),
            "pause" => {
                let state = app.state::<AppState>();
                let now = !state.clip_paused.load(Ordering::SeqCst);
                state.clip_paused.store(now, Ordering::SeqCst);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                crate::windows::show_main(tray.app_handle(), None);
            }
        })
        .build(app)?;
    Ok(())
}

/// Rebuild the menu (language or pause state changed).
pub fn refresh(app: &AppHandle) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id(TRAY_ID), build_menu(app)) {
        let _ = tray.set_menu(Some(menu));
    }
}

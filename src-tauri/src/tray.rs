//! System tray icon and menu (the app's home while no window is open).

use crate::state::AppState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

const TRAY_ID: &str = "main-tray";

struct Labels {
    open: &'static str,
    line: &'static str,
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
            line: "İpi göster",
            pause: "Panoyu Duraklat",
            quit: "Çıkış",
        }
    } else {
        Labels {
            open: "Open File Chameleon",
            line: "Show the line",
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
            &MenuItem::with_id(app, "line", l.line, true, None::<&str>)?,
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
            // The clothesline, as the hotkey shows it. Off the event loop: the first show builds
            // the webview.
            "line" => {
                let app = app.clone();
                if !app.state::<AppState>().settings().line_enabled {
                    // Off: nothing would appear, so open the settings where it is switched on.
                    crate::windows::show_main(
                        &app,
                        Some(crate::events::NavPayload {
                            view: "settings".into(),
                            paths: vec![],
                            url: None,
                            clip: None,
                        }),
                    );
                } else {
                    std::thread::spawn(move || crate::line::show(&app, "hotkey"));
                }
            }
            "pause" => {
                let state = app.state::<AppState>();
                let now = !state.clip_paused.load(Ordering::SeqCst);
                crate::ipc::clipboard::set_paused(app, now);
            }
            "quit" => quit(app),
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

/// Quit: cancel running conversions and downloads, give them up to 2 s to remove their partial
/// files (`*.fcpart.*`, staging folders), then exit. Runs off the main thread so the wait
/// never freezes the UI.
pub fn quit(app: &AppHandle) {
    static QUITTING: AtomicBool = AtomicBool::new(false);
    if QUITTING.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let jobs = state.jobs_if_started();
        let jobs_busy =
            || jobs.is_some_and(|q| q.snapshot().iter().any(|s| !s.state.is_finished()));
        if let Some(q) = jobs {
            for s in q.snapshot().iter().filter(|s| !s.state.is_finished()) {
                q.cancel(s.id);
            }
        }
        state.downloads.cancel_all();
        let deadline = Instant::now() + Duration::from_secs(2);
        while (jobs_busy() || state.downloads.any_active()) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        app.exit(0);
    });
}

/// Rebuild the menu (language or pause state changed).
pub fn refresh(app: &AppHandle) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id(TRAY_ID), build_menu(app)) {
        let _ = tray.set_menu(Some(menu));
    }
}

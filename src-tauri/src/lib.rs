//! File Chameleon desktop shell (Tauri 2). Long-lived work (tray, mouse hook, clipboard
//! listener, job queue) lives in Rust; webviews only exist while visible (main) or are
//! kept tiny (overlay), to keep memory low.

mod events;
mod ipc;
mod platform;
mod state;
mod tray;
mod windows;

use tauri::Manager;

pub fn run() {
    let background = std::env::args().any(|a| a == "--background");

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            windows::show_main(app, None);
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--background"]),
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            let handle = app.handle().clone();
            let state = state::AppState::init(&handle)?;
            app.manage(state);
            tray::create(&handle)?;
            windows::create_overlay(&handle)?;
            if !background {
                windows::show_main(&handle, None);
            }
            platform::start(&handle);
            Ok(())
        })
        .on_window_event(windows::on_window_event)
        .invoke_handler(tauri::generate_handler![
            ipc::app_info,
            ipc::get_settings,
            ipc::set_settings,
            ipc::list_targets,
            ipc::submit_job,
            ipc::cancel_job,
            ipc::list_jobs,
            ipc::clear_jobs,
            ipc::tools_status,
            ipc::install_tool,
            ipc::uninstall_tool,
            ipc::downloads::probe_url,
            ipc::downloads::start_download,
            ipc::downloads::cancel_download,
            ipc::downloads::list_downloads,
            ipc::clipboard::clip_list,
            ipc::clipboard::clip_pin,
            ipc::clipboard::clip_delete,
            ipc::clipboard::clip_clear,
            ipc::clipboard::clip_copy,
            ipc::clipboard::clip_save_image,
            ipc::clipboard::clip_set_paused,
            ipc::reveal_path,
            ipc::open_path,
            ipc::show_main,
            ipc::take_pending_nav,
            ipc::overlay_ready,
            ipc::overlay_hide,
            ipc::overlay_toast_action,
            ipc::wheel_drop,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build File Chameleon")
        .run(|_app, event| {
            // Closing the main window must not quit: the app keeps living in the tray.
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event
                && code.is_none()
            {
                api.prevent_exit();
            }
        });
}

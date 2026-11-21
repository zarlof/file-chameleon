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
    // Lossy: `std::env::args` panics on a file name that is not valid Unicode, and the Explorer
    // verb passes file names.
    let args: Vec<String> = std::env::args_os()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    // Explorer's "Convert with File Chameleon" verb: open the wheel, not the window.
    let wheel_args = platform::contextmenu::wheel_paths(&args);
    let background = args.iter().any(|a| a == "--background") || !wheel_args.is_empty();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            let wheel = platform::contextmenu::wheel_paths(&args);
            if !wheel.is_empty() {
                windows::queue_wheel_paths(app, wheel);
            } else if !args.iter().any(|a| a == "--background") {
                // A second autostart launch (--background) must not pop the window up.
                windows::show_main(app, None);
            }
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
            // Opens once the overlay page is ready (with paths of further invocations).
            windows::queue_wheel_paths(&handle, wheel_args);
            // Apply clipboard retention shortly after start (opening the store prunes it), so
            // expired items are removed even if nothing is copied, and remove download leftovers
            // of earlier runs (crash, failed cleanup). Delayed to keep startup light.
            let h = handle.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(10));
                let state = h.state::<state::AppState>();
                if state.settings().clipboard_enabled {
                    let _ = state.clips();
                }
                // This run's downloads are at most ~10 s old here.
                ipc::downloads::sweep_staging(
                    &state.paths.staging_dir(),
                    std::time::Duration::from_secs(5 * 60),
                );
            });
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
            ipc::install_and_submit,
            ipc::update_ytdlp,
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
            ipc::clipboard::clip_get_paused,
            ipc::reveal_path,
            ipc::open_path,
            ipc::show_main,
            ipc::take_pending_nav,
            ipc::overlay_ready,
            ipc::overlay_hide,
            ipc::overlay_toast_action,
            ipc::wheel_drop,
            // presets & wheel v2, image editor (Task 6)
            ipc::run_preset,
            ipc::wheel_open,
            ipc::presets::get_wheel_config,
            ipc::presets::set_wheel_config,
            ipc::presets::reset_wheel_profile,
            ipc::presets::reset_wheel_config,
            ipc::presets::presets_for_extensions,
            ipc::images::read_image_preview,
            ipc::images::save_annotated,
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

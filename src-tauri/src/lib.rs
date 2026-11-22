//! File Chameleon desktop shell (Tauri 2). Long-lived work (tray, mouse hook, clipboard
//! listener, job queue) lives in Rust; webviews only exist while visible (main) or are
//! kept tiny (overlay), to keep memory low.

mod events;
mod files;
mod float;
mod hang;
mod ipc;
mod line;
mod platform;
mod state;
mod tray;
mod watchfolders;
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
    // Explorer's "İpe as" verb: hang the files on the clothesline, no window.
    let hang_args = hang::paths_from_args(&args);
    let background =
        args.iter().any(|a| a == "--background") || !wheel_args.is_empty() || !hang_args.is_empty();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            let wheel = platform::contextmenu::wheel_paths(&args);
            let hang = hang::paths_from_args(&args);
            if !hang.is_empty() {
                hang::import_paths(app, hang);
            } else if !wheel.is_empty() {
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
            // The files rope's thumbnails are shown through the asset protocol; the static
            // scope pattern never matches a Windows path, so allow the folder itself.
            let thumbs = state.paths.files_thumbs_dir();
            let _ = std::fs::create_dir_all(&thumbs);
            if let Err(e) = app.asset_protocol_scope().allow_directory(&thumbs, true) {
                eprintln!("asset scope for files thumbs: {e}");
            }
            app.manage(state);
            tray::create(&handle)?;
            windows::create_overlay(&handle)?;
            if !background {
                windows::show_main(&handle, None);
            }
            platform::start(&handle);
            // Opens once the overlay page is ready (with paths of further invocations).
            windows::queue_wheel_paths(&handle, wheel_args);
            hang::import_paths(&handle, hang_args);
            // Remove download leftovers of earlier runs (crash, failed cleanup), delayed to keep
            // startup light. Clipboard retention runs in `platform::clipboard::start_maintenance`.
            let h = handle.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(10));
                let state = h.state::<state::AppState>();
                // This run's downloads are at most ~10 s old here.
                ipc::downloads::sweep_staging(
                    &state.paths.staging_dir(),
                    std::time::Duration::from_secs(5 * 60),
                );
                // Thumbnails of files that left the files rope (at most once a day).
                files::sweep_thumbs(&h);
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
            // Clipboard v2 and hotkeys (Task 9).
            ipc::clipboard::clip_list,
            ipc::clipboard::clip_get,
            ipc::clipboard::clip_set_field,
            ipc::clipboard::clip_pin,
            ipc::clipboard::clip_trash,
            ipc::clipboard::clip_restore,
            ipc::clipboard::clip_delete_forever,
            ipc::clipboard::clip_empty_trash,
            ipc::clipboard::clip_clear_unsaved,
            ipc::clipboard::clip_folders,
            ipc::clipboard::clip_folder_create,
            ipc::clipboard::clip_folder_rename,
            ipc::clipboard::clip_folder_delete,
            ipc::clipboard::clip_move,
            ipc::clipboard::clip_tags,
            ipc::clipboard::clip_tag,
            ipc::clipboard::clip_copy,
            ipc::clipboard::clip_copy_ocr,
            ipc::clipboard::clip_export,
            ipc::clipboard::clip_save_image,
            ipc::clipboard::clip_save_edited,
            ipc::clipboard::clip_revert_edit,
            ipc::clipboard::clip_stats,
            ipc::clipboard::clip_set_paused,
            ipc::clipboard::clip_get_paused,
            ipc::clipboard::hotkey_status,
            ipc::reveal_path,
            ipc::open_path,
            ipc::show_main,
            ipc::take_pending_nav,
            ipc::overlay_ready,
            // clothesline and float windows
            ipc::line::line_show,
            ipc::line::line_hide,
            ipc::line::line_activate,
            ipc::line::line_state,
            ipc::line::line_drag_out,
            ipc::line::line_open_item,
            ipc::line::line_paste,
            ipc::line::line_files,
            ipc::line::line_file_copy,
            ipc::line::line_file_open,
            ipc::line::line_file_paste,
            ipc::line::line_copy_text,
            ipc::line::line_file_reveal,
            ipc::line::line_file_remove,
            ipc::line::line_open_clip,
            ipc::line::line_share,
            ipc::line::line_file_pin,
            ipc::line::line_clip_rank,
            ipc::line::line_clip_files,
            ipc::line::line_file_rank,
            ipc::line::line_card_color,
            ipc::line::line_clear_card_colors,
            ipc::line::line_file_drag_out,
            ipc::line::open_url,
            ipc::line::float_open,
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
            // watch folders (Task 10)
            watchfolders::watch_status,
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

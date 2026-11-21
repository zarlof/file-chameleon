//! App-wide state. Heavy parts (engine manager, job queue, clipboard store, downloads)
//! are created on first use so startup stays fast and light.

use crate::events;
use fc_core::clip::ClipStore;
use fc_core::engine_mgr::{EngineManager, Manifest};
use fc_core::engines::DefaultExecutor;
use fc_core::files::FileStore;
use fc_core::jobs::queue::JobQueue;
use fc_core::presets::WheelConfig;
use fc_core::settings::Settings;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Manager};

pub struct Paths {
    /// `%APPDATA%\app.filechameleon.desktop` (settings).
    pub config: PathBuf,
    /// `%LOCALAPPDATA%\app.filechameleon.desktop` (engines, clipboard, staging, webview data).
    pub data: PathBuf,
}

impl Paths {
    /// True for files inside the clipboard store (`clipboard`, or a `clipboard-<ms>` fallback).
    pub fn is_clipboard_file(&self, path: &Path) -> bool {
        path.strip_prefix(&self.data)
            .ok()
            .and_then(|rest| rest.components().next())
            .and_then(|c| c.as_os_str().to_str())
            .is_some_and(|first| first.starts_with("clipboard"))
    }
    pub fn settings_file(&self) -> PathBuf {
        self.config.join("settings.json")
    }
    pub fn engines_dir(&self) -> PathBuf {
        self.data.join("engines")
    }
    pub fn clipboard_dir(&self) -> PathBuf {
        self.data.join("clipboard")
    }
    pub fn webview_dir(&self) -> PathBuf {
        self.data.join("webview")
    }
    pub fn staging_dir(&self) -> PathBuf {
        self.data.join("staging")
    }
    /// Cached 256 px thumbnails of the files rope (`<hash>.png`).
    pub fn files_thumbs_dir(&self) -> PathBuf {
        self.data.join("files-thumbs")
    }
    /// Presets, groups and wheel profiles (Task 6).
    pub fn wheel_file(&self) -> PathBuf {
        self.config.join("wheel.json")
    }
}

pub struct AppState {
    pub app: AppHandle,
    pub paths: Paths,
    pub settings: Mutex<Settings>,
    /// Held across read-modify-write of the settings (commands run on several threads).
    settings_tx: Mutex<()>,
    engines: OnceLock<Arc<EngineManager>>,
    jobs: OnceLock<JobQueue>,
    clips: OnceLock<Mutex<ClipStore>>,
    /// The files rope's list (`files.json`), loaded on first use.
    files: OnceLock<Mutex<FileStore>>,
    pub downloads: crate::ipc::downloads::Downloads,
    /// Clipboard capture paused from the tray or the Clipboard view.
    pub clip_paused: AtomicBool,
    /// Clipboard sequence number right after our own write (0 = none); updates with this number are not recorded.
    pub clip_self_write: AtomicU32,
    toast_seq: AtomicU64,
    /// Wheel config (`wheel.json`), loaded on first use (Task 6).
    wheel: OnceLock<Mutex<WheelConfig>>,
    // ---- clipboard v2 (Task 9)
    /// Foreground window (HWND as isize, 0 = none) when the clothesline was opened; `line_paste`
    /// pastes there.
    pub line_prev: AtomicIsize,
    /// Watcher of `clipboard_watch_folders` (only while some are set and the clipboard is on).
    pub clip_watcher: Mutex<Option<fc_core::watch::Watcher>>,
    // ---- watch folders (Task 10)
    pub watch: crate::watchfolders::Holder,
}

impl AppState {
    pub fn init(app: &AppHandle) -> tauri::Result<Self> {
        // Identifier-based folders (%APPDATA% / %LOCALAPPDATA%pp.filechameleon.desktop).
        // A plain "FileChameleon" folder is already used by an unrelated older app of the same
        // name, so sharing it would mix files and let one app's uninstall delete the other's data.
        let paths = Paths {
            config: app.path().app_config_dir()?,
            data: app.path().app_local_data_dir()?,
        };
        let _ = std::fs::create_dir_all(&paths.config);
        let _ = std::fs::create_dir_all(&paths.data);
        let settings = Settings::load(&paths.settings_file());
        let paused = settings.clipboard_paused;
        Ok(Self {
            app: app.clone(),
            paths,
            settings: Mutex::new(settings),
            settings_tx: Mutex::new(()),
            engines: OnceLock::new(),
            jobs: OnceLock::new(),
            clips: OnceLock::new(),
            files: OnceLock::new(),
            downloads: Default::default(),
            clip_paused: AtomicBool::new(paused),
            clip_self_write: AtomicU32::new(0),
            toast_seq: AtomicU64::new(1),
            wheel: OnceLock::new(),
            line_prev: AtomicIsize::new(0),
            clip_watcher: Mutex::new(None),
            watch: Default::default(),
        })
    }

    pub fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }

    pub fn save_settings(&self, s: Settings) {
        let _ = s.save(&self.paths.settings_file());
        *self.settings.lock().unwrap() = s.clone();
        let _ = self.app.emit(events::SETTINGS_CHANGED, s);
    }

    /// Change the settings with `f` and save them if anything changed. Serialized with other
    /// updates (see `settings_transaction`); `f` must not call back into the settings.
    pub fn update_settings(&self, f: impl FnOnce(&mut Settings)) -> Settings {
        let _tx = self.settings_transaction();
        let old = self.settings();
        let mut new = old.clone();
        f(&mut new);
        if new != old {
            self.save_settings(new.clone());
        }
        new
    }

    /// Lock for a read-modify-write of the settings. Reads (`settings()`) stay possible.
    pub fn settings_transaction(&self) -> std::sync::MutexGuard<'_, ()> {
        self.settings_tx.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn engines(&self) -> &Arc<EngineManager> {
        self.engines.get_or_init(|| {
            Arc::new(EngineManager::new(
                self.paths.engines_dir(),
                Manifest::builtin(),
            ))
        })
    }

    pub fn jobs(&self) -> &JobQueue {
        self.jobs.get_or_init(|| {
            let executor = Arc::new(DefaultExecutor {
                tools: self.engines().clone(),
            });
            let app = self.app.clone();
            JobQueue::new(
                executor,
                Arc::new(move |snap| crate::ipc::on_job_update(&app, snap)),
            )
        })
    }

    /// The job queue, only if something already used it (never creates it).
    pub fn jobs_if_started(&self) -> Option<&JobQueue> {
        self.jobs.get()
    }

    pub fn clips(&self) -> &Mutex<ClipStore> {
        self.clips.get_or_init(|| {
            let dir = self.paths.clipboard_dir();
            let mut used = dir.clone();
            let mut store = ClipStore::open(&dir).unwrap_or_else(|_| {
                // A broken store must never stop the app; start empty in a fresh folder.
                let fallback = dir.with_file_name(format!("clipboard-{}", fc_core::now_ms()));
                used = fallback.clone();
                ClipStore::open(&fallback).expect("clipboard store")
            });
            // The static scope pattern never matches a Windows path, so allow the store dir
            // that is actually in use (Tauri escapes the path and adds `**`).
            if let Err(e) = self.app.asset_protocol_scope().allow_directory(&used, true) {
                eprintln!("asset scope for clip store: {e}");
            }
            // Apply retention on open too, so expired items do not wait for the next copy.
            let s = self.settings();
            let _ = store.prune(
                fc_core::now_ms(),
                &fc_core::clip::Retention::from_settings(&s),
            );
            Mutex::new(store)
        })
    }

    /// The files rope's store. A broken or missing `files.json` is an empty list.
    pub fn files(&self) -> &Mutex<FileStore> {
        self.files.get_or_init(|| {
            Mutex::new(FileStore::open(&self.paths.data).unwrap_or_else(|_| {
                FileStore::open(&self.paths.data.join("files-fallback")).expect("files store")
            }))
        })
    }

    pub fn next_toast_id(&self) -> u64 {
        self.toast_seq.fetch_add(1, Ordering::Relaxed)
    }

    pub fn default_download_dir(&self) -> PathBuf {
        self.settings()
            .download_dir
            .or_else(|| self.app.path().download_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."))
    }

    // ---- wheel config (Task 6)

    /// The wheel config, loaded from `wheel.json` on first use (a missing or broken file gives
    /// the built-ins). Do not hold the guard across `update_wheel_config`.
    pub fn wheel_config(&self) -> std::sync::MutexGuard<'_, WheelConfig> {
        self.wheel
            .get_or_init(|| Mutex::new(WheelConfig::load(&self.paths.wheel_file())))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    /// Change a copy of the wheel config with `f`, write it to `wheel.json`, and only then use
    /// it: a failed change or save leaves the config as it was. Returns the new config. The lock
    /// is held throughout, so updates never interleave or share the temp file.
    pub fn update_wheel_config(
        &self,
        f: impl FnOnce(&mut WheelConfig) -> fc_core::Result<()>,
    ) -> fc_core::Result<WheelConfig> {
        let mut current = self.wheel_config();
        let mut next = current.clone();
        f(&mut next)?;
        next.save(&self.paths.wheel_file())?;
        *current = next.clone();
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::Paths;
    use std::path::{Path, PathBuf};

    #[test]
    fn clipboard_files_are_recognised() {
        let paths = Paths {
            config: PathBuf::from("C:/cfg"),
            data: PathBuf::from("C:/data/app"),
        };
        assert!(paths.is_clipboard_file(Path::new("C:/data/app/clipboard/img/1.png")));
        assert!(paths.is_clipboard_file(Path::new("C:/data/app/clipboard-123/img/1.png")));
        assert!(!paths.is_clipboard_file(Path::new("C:/data/app/engines/ffmpeg.exe")));
        assert!(!paths.is_clipboard_file(Path::new("C:/Users/me/clipboard/a.png")));
    }
}

//! App-wide state. Heavy parts (engine manager, job queue, clipboard store, downloads)
//! are created on first use so startup stays fast and light.

use crate::events;
use fc_core::clip::ClipStore;
use fc_core::engine_mgr::{EngineManager, Manifest};
use fc_core::engines::DefaultExecutor;
use fc_core::jobs::queue::JobQueue;
use fc_core::settings::Settings;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
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
    pub downloads: crate::ipc::downloads::Downloads,
    /// Clipboard capture paused from the tray or the Clipboard view.
    pub clip_paused: AtomicBool,
    /// Clipboard sequence number right after our own write (0 = none); updates with this number are not recorded.
    pub clip_self_write: AtomicU32,
    toast_seq: AtomicU64,
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
            downloads: Default::default(),
            clip_paused: AtomicBool::new(paused),
            clip_self_write: AtomicU32::new(0),
            toast_seq: AtomicU64::new(1),
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
            let mut store = ClipStore::open(&dir).unwrap_or_else(|_| {
                // A broken store must never stop the app; start empty in a fresh folder.
                let fallback = dir.with_file_name(format!("clipboard-{}", fc_core::now_ms()));
                ClipStore::open(&fallback).expect("clipboard store")
            });
            // Apply retention on open too, so expired items do not wait for the next copy.
            let s = self.settings();
            let _ = store.prune(
                fc_core::now_ms(),
                s.clipboard_retention_images,
                s.clipboard_max_items as usize,
            );
            Mutex::new(store)
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

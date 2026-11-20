//! App-wide state. Heavy parts (engine manager, job queue, clipboard store, downloads)
//! are created on first use so startup stays fast and light.

use crate::events;
use fc_core::clip::ClipStore;
use fc_core::engine_mgr::{EngineManager, Manifest};
use fc_core::engines::DefaultExecutor;
use fc_core::jobs::queue::JobQueue;
use fc_core::settings::Settings;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Manager};

pub struct Paths {
    /// `%APPDATA%\FileChameleon` (settings).
    pub config: PathBuf,
    /// `%LOCALAPPDATA%\FileChameleon` (engines, clipboard, webview data).
    pub data: PathBuf,
}

impl Paths {
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
        let base_config = app.path().config_dir()?;
        let base_data = app.path().local_data_dir()?;
        let paths = Paths {
            config: base_config.join("FileChameleon"),
            data: base_data.join("FileChameleon"),
        };
        let _ = std::fs::create_dir_all(&paths.config);
        let _ = std::fs::create_dir_all(&paths.data);
        let settings = Settings::load(&paths.settings_file());
        Ok(Self {
            app: app.clone(),
            paths,
            settings: Mutex::new(settings),
            engines: OnceLock::new(),
            jobs: OnceLock::new(),
            clips: OnceLock::new(),
            downloads: Default::default(),
            clip_paused: AtomicBool::new(false),
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

    pub fn clips(&self) -> &Mutex<ClipStore> {
        self.clips.get_or_init(|| {
            let dir = self.paths.clipboard_dir();
            let store = ClipStore::open(&dir).unwrap_or_else(|_| {
                // A broken store must never stop the app; start empty in a fresh folder.
                let fallback = dir.with_file_name(format!("clipboard-{}", fc_core::now_ms()));
                ClipStore::open(&fallback).expect("clipboard store")
            });
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

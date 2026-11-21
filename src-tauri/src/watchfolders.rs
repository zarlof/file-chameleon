//! Watch folders (Task 10): one `Watcher` over the enabled rules of `settings.watch_folders`.
//! A file that settled is run through the rule's preset without any dialog, the results go to
//! `<folder>\converted` (which is never watched), and `watch://status` tells the UI.

use crate::ipc::presets::run_preset;
use crate::state::AppState;
use fc_core::presets::{self, WheelConfig};
use fc_core::settings::{Settings, WatchFolder};
use fc_core::watch::{WatchSpec, Watcher};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// Event: one rule's status changed (`WatchStatus`).
pub const WATCH_STATUS: &str = "watch://status";
/// Name of the output folder inside each watched folder.
pub const OUTPUT_FOLDER: &str = "converted";

/// Mirrors `WatchStatus` in `src/lib/types.ts`. `error` is one of the codes of
/// [`rule_problem`], a job error message, or the text of a folder read error.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchStatus {
    pub id: String,
    pub active: bool,
    pub error: Option<String>,
    /// Files handed to the job queue (not necessarily finished).
    pub processed: u32,
    pub last_file: Option<String>,
}

#[derive(Default)]
struct Counters {
    queued: u32,
    last_file: Option<String>,
    /// The last file that could not be submitted (cleared by the next success).
    error: Option<String>,
}

/// What `AppState` keeps. The two locks are separate: dropping a `Watcher` joins its thread,
/// which may be waiting for `counters`.
#[derive(Default)]
pub struct Holder {
    watcher: Mutex<Option<Watcher>>,
    counters: Mutex<HashMap<String, Counters>>,
}

/// Why a rule cannot run, as a code the UI translates; `None` = fine. `copyTo` without a
/// folder needs a picker, `annotate` the editor, and the file actions (`copyPath`, `openTerminal`,
/// `float`) belong to the shell: none of them works unattended.
pub fn rule_problem(config: &WheelConfig, preset_id: &str) -> Option<&'static str> {
    use fc_core::jobs::Operation;
    match config.preset(preset_id).map(|p| &p.op) {
        None => Some("presetMissing"),
        Some(
            Operation::Annotate | Operation::CopyPath | Operation::OpenTerminal | Operation::Float,
        ) => Some("presetNotUnattended"),
        Some(Operation::CopyTo { dir: None, .. }) => Some("presetNotUnattended"),
        Some(_) => None,
    }
}

/// Comparable form of a folder path: separators unified, no trailing separator, and
/// case-folded on Windows (the file system there ignores case).
fn norm(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    let s = s.trim_end_matches('/');
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s.to_string()
    }
}

/// The fixed folder a `copyTo` preset writes to, when the rule has one.
fn fixed_target(config: &WheelConfig, preset_id: &str) -> Option<PathBuf> {
    use fc_core::jobs::Operation;
    match config.preset(preset_id).map(|p| &p.op) {
        Some(Operation::CopyTo { dir: Some(d), .. }) => Some(d.clone()),
        _ => None,
    }
}

/// Whether the preset writes straight into the watched folder (every result would be a new file
/// to process, forever).
fn is_self_target(config: &WheelConfig, rule: &WatchFolder) -> bool {
    fixed_target(config, &rule.preset).is_some_and(|d| norm(&d) == norm(&rule.path))
}

/// [`rule_problem`] plus the rule-level `selfTarget` check.
pub fn rule_error(config: &WheelConfig, rule: &WatchFolder) -> Option<&'static str> {
    rule_problem(config, &rule.preset)
        .or_else(|| is_self_target(config, rule).then_some("selfTarget"))
}

/// Folders the watcher must not enter: `converted`, and a fixed copy/move target inside the
/// watched folder (only matters with subfolders on, harmless otherwise).
fn excludes(config: &WheelConfig, rule: &WatchFolder) -> Vec<PathBuf> {
    let mut out = vec![rule.path.join(OUTPUT_FOLDER)];
    if let Some(d) = fixed_target(config, &rule.preset) {
        let (target, base) = (norm(&d), norm(&rule.path));
        if target.len() > base.len() && target.starts_with(&format!("{base}/")) {
            out.push(d);
        }
    }
    out
}

/// The watcher specs: enabled rules with a folder and a usable preset. `exclude` is built from
/// the same `dir` the spec carries (the watcher compares the strings `read_dir` yields).
pub fn specs_from_settings(s: &Settings, config: &WheelConfig) -> Vec<WatchSpec> {
    s.watch_folders
        .iter()
        .filter(|w| w.enabled && !w.path.as_os_str().is_empty() && rule_error(config, w).is_none())
        .map(|w| WatchSpec {
            id: w.id.clone(),
            dir: w.path.clone(),
            recursive: w.subfolders,
            exclude: excludes(config, w),
            // Every file is checked against the preset when it fires.
            extensions: None,
        })
        .collect()
}

/// Start, update or stop the watcher for `s`. Only runs a thread while a rule is usable.
pub fn configure(app: &AppHandle, s: &Settings) {
    let state = app.state::<AppState>();
    let specs = specs_from_settings(s, &state.wheel_config());
    {
        let mut counters = state
            .watch
            .counters
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        counters.retain(|id, _| s.watch_folders.iter().any(|w| &w.id == id));
    }
    let mut slot = state
        .watch
        .watcher
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if specs.is_empty() {
        // Dropping joins the thread: after releasing the lock.
        let old = slot.take();
        drop(slot);
        drop(old);
    } else {
        let watcher = slot.get_or_insert_with(|| {
            let app = app.clone();
            Watcher::start(
                Duration::from_secs(2),
                Duration::from_secs(2),
                move |id, path| on_file(&app, &id, &path),
            )
        });
        watcher.set_specs(specs);
        drop(slot);
    }
    for w in &s.watch_folders {
        let _ = app.emit(WATCH_STATUS, status_of(app, w));
    }
}

/// What to do with a settled file.
#[derive(Debug, PartialEq)]
enum Decision {
    /// Not ours (rule gone or off) or not for this preset (a text file next to PNGs).
    Skip,
    /// The rule cannot run (code of [`rule_problem`]); the status row shows it live.
    Blocked(&'static str),
    Run {
        preset: String,
        out_dir: PathBuf,
    },
}

/// Pure decision for `path` of rule `id`; no I/O beyond what `presets::applicable` probes.
fn decide(s: &Settings, config: &WheelConfig, id: &str, path: &Path) -> Decision {
    let Some(rule) = s.watch_folders.iter().find(|w| w.id == id && w.enabled) else {
        return Decision::Skip;
    };
    if let Some(code) = rule_error(config, rule) {
        return Decision::Blocked(code);
    }
    let fits = config
        .preset(&rule.preset)
        .is_some_and(|p| presets::applicable(p, &[path.to_path_buf()]));
    if !fits {
        return Decision::Skip;
    }
    Decision::Run {
        preset: rule.preset.clone(),
        out_dir: rule.path.join(OUTPUT_FOLDER),
    }
}

/// `"<code>:<message>"` for a failed submit: the UI translates the code (`missingTool`,
/// otherwise `failed`) and keeps the message as detail.
fn job_error(code: &str, message: &str) -> String {
    let code = if code == "missingTool" {
        "missingTool"
    } else {
        "failed"
    };
    format!("{code}:{message}")
}

impl Counters {
    fn record(&mut self, path: &Path, result: Result<(), String>) {
        match result {
            Ok(()) => {
                self.queued += 1;
                self.last_file = path.file_name().map(|n| n.to_string_lossy().into_owned());
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }
}

/// Runs on the watcher thread for a file that settled.
fn on_file(app: &AppHandle, id: &str, path: &Path) {
    let state = app.state::<AppState>();
    let s = state.settings();
    // A copy: probing the file must not hold the config lock.
    let config = state.wheel_config().clone();
    let (preset, out_dir) = match decide(&s, &config, id, path) {
        Decision::Run { preset, out_dir } => (preset, out_dir),
        Decision::Skip | Decision::Blocked(_) => return,
    };
    let result = std::fs::create_dir_all(&out_dir)
        .map_err(|e| job_error("io", &e.to_string()))
        .and_then(|()| {
            run_preset(app, vec![path.to_path_buf()], &preset, false, Some(out_dir))
                .map(|_| ())
                .map_err(|e| job_error(&e.code, &e.message))
        });
    state
        .watch
        .counters
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .entry(id.to_string())
        .or_default()
        .record(path, result);
    if let Some(rule) = s.watch_folders.iter().find(|w| w.id == id) {
        let _ = app.emit(WATCH_STATUS, status_of(app, rule));
    }
}

/// Re-read the settings and the wheel config (a preset may have come back or gone).
pub fn reconfigure(app: &AppHandle) {
    let s = app.state::<AppState>().settings();
    configure(app, &s);
}

fn status_of(app: &AppHandle, rule: &WatchFolder) -> WatchStatus {
    let state = app.state::<AppState>();
    let problem = rule_error(&state.wheel_config(), rule);
    let (processed, last_file, job_error) = state
        .watch
        .counters
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&rule.id)
        .map(|c| (c.queued, c.last_file.clone(), c.error.clone()))
        .unwrap_or_default();
    let folder_error = state
        .watch
        .watcher
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|w| {
            w.errors()
                .into_iter()
                .find_map(|(id, msg)| (id == rule.id).then_some(msg))
        });
    build_status(rule, problem, folder_error, job_error, processed, last_file)
}

/// Pure part of `status_of`: the first problem wins (preset, then folder, then last file).
fn build_status(
    rule: &WatchFolder,
    problem: Option<&str>,
    folder_error: Option<String>,
    job_error: Option<String>,
    processed: u32,
    last_file: Option<String>,
) -> WatchStatus {
    let error = problem.map(str::to_string).or(folder_error).or(job_error);
    WatchStatus {
        id: rule.id.clone(),
        active: rule.enabled && error.is_none(),
        error,
        processed,
        last_file,
    }
}

/// Status of every rule (disabled ones too).
pub fn all_status(app: &AppHandle) -> Vec<WatchStatus> {
    let s = app.state::<AppState>().settings();
    s.watch_folders.iter().map(|w| status_of(app, w)).collect()
}

#[tauri::command(async)]
pub fn watch_status(app: AppHandle) -> Vec<WatchStatus> {
    all_status(&app)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fc_core::jobs::Operation;
    use fc_core::presets::Preset;
    use std::sync::mpsc;
    use std::time::Instant;

    fn rule(id: &str, path: &str, preset: &str, enabled: bool) -> WatchFolder {
        WatchFolder {
            id: id.into(),
            path: PathBuf::from(path),
            preset: preset.into(),
            subfolders: false,
            enabled,
        }
    }

    fn settings(rules: Vec<WatchFolder>) -> Settings {
        Settings {
            watch_folders: rules,
            ..Settings::default()
        }
    }

    #[test]
    fn specs_from_settings() {
        let config = WheelConfig::builtin();
        let mut on = rule("a", "C:\\in", "to-webp", true);
        on.subfolders = true;
        let s = settings(vec![
            on,
            rule("b", "C:\\off", "to-webp", false),
            rule("c", "", "to-webp", true),
            rule("d", "C:\\gone", "no-such-preset", true),
        ]);
        let specs = super::specs_from_settings(&s, &config);
        assert_eq!(specs.len(), 1);
        let spec = &specs[0];
        assert_eq!(spec.id, "a");
        assert_eq!(spec.dir, PathBuf::from("C:\\in"));
        assert!(spec.recursive);
        assert_eq!(spec.exclude, [PathBuf::from("C:\\in").join("converted")]);
        assert_eq!(spec.extensions, None);
    }

    #[test]
    fn rule_problems() {
        let mut config = WheelConfig::builtin();
        assert_eq!(rule_problem(&config, "to-webp"), None);
        assert_eq!(rule_problem(&config, "nope"), Some("presetMissing"));
        let user = |id: &str, op: Operation| Preset {
            id: id.into(),
            name: id.into(),
            builtin: false,
            op,
            options: Default::default(),
        };
        config.presets.push(user(
            "pick",
            Operation::CopyTo {
                dir: None,
                move_files: false,
            },
        ));
        config.presets.push(user(
            "fixed",
            Operation::CopyTo {
                dir: Some("D:/x".into()),
                move_files: false,
            },
        ));
        config.presets.push(user("edit", Operation::Annotate));
        for (id, op) in [
            ("cp", Operation::CopyPath),
            ("term", Operation::OpenTerminal),
            ("fl", Operation::Float),
        ] {
            config.presets.push(user(id, op));
            assert_eq!(
                rule_problem(&config, id),
                Some("presetNotUnattended"),
                "{id}"
            );
        }
        assert_eq!(rule_problem(&config, "pick"), Some("presetNotUnattended"));
        assert_eq!(rule_problem(&config, "edit"), Some("presetNotUnattended"));
        assert_eq!(rule_problem(&config, "fixed"), None);
    }

    fn copy_preset(id: &str, dir: &str, move_files: bool) -> Preset {
        Preset {
            id: id.into(),
            name: id.into(),
            builtin: false,
            op: Operation::CopyTo {
                dir: Some(dir.into()),
                move_files,
            },
            options: Default::default(),
        }
    }

    #[test]
    fn a_target_equal_to_the_watched_folder_is_rejected() {
        let mut config = WheelConfig::builtin();
        config
            .presets
            .push(copy_preset("same", "C:/In/Photos/", false));
        config
            .presets
            .push(copy_preset("mv", "c:\\in\\photos", true));
        config.presets.push(copy_preset("other", "D:/out", false));
        let r = |preset: &str| rule("a", "C:\\in\\photos", preset, true);
        assert_eq!(rule_error(&config, &r("same")), Some("selfTarget"));
        assert_eq!(rule_error(&config, &r("mv")), Some("selfTarget"));
        assert_eq!(rule_error(&config, &r("other")), None);
        assert_eq!(rule_error(&config, &r("to-webp")), None);
        // The rule does not start, and a settled file is blocked.
        let s = settings(vec![r("same"), r("other")]);
        let specs = super::specs_from_settings(&s, &config);
        assert_eq!(specs.len(), 1);
        assert_eq!(
            decide(&s, &config, "a", Path::new("C:\\in\\photos\\x.png")),
            Decision::Blocked("selfTarget")
        );
    }

    #[test]
    fn a_target_inside_the_watched_folder_is_excluded() {
        let mut config = WheelConfig::builtin();
        config
            .presets
            .push(copy_preset("inner", "C:/in/photos/sorted", false));
        config
            .presets
            .push(copy_preset("lookalike", "C:/in/photos-sorted", false));
        let mut r = rule("a", "C:\\in\\photos", "inner", true);
        r.subfolders = true;
        assert_eq!(rule_error(&config, &r), None);
        let ex = excludes(&config, &r);
        assert_eq!(ex.len(), 2);
        assert_eq!(ex[0], PathBuf::from("C:\\in\\photos").join("converted"));
        assert_eq!(ex[1], PathBuf::from("C:/in/photos/sorted"));
        // A sibling whose name merely starts the same is not inside.
        r.preset = "lookalike".into();
        assert_eq!(excludes(&config, &r).len(), 1);
    }

    #[test]
    fn status_reports_the_first_problem() {
        let r = rule("a", "C:\\in", "to-webp", true);
        let ok = build_status(&r, None, None, None, 3, Some("x.png".into()));
        assert!(ok.active && ok.error.is_none());
        assert_eq!((ok.processed, ok.last_file.as_deref()), (3, Some("x.png")));
        let gone = build_status(&r, Some("presetMissing"), Some("io".into()), None, 0, None);
        assert!(!gone.active);
        assert_eq!(gone.error.as_deref(), Some("presetMissing"));
        let io = build_status(&r, None, Some("io".into()), Some("job".into()), 0, None);
        assert_eq!(io.error.as_deref(), Some("io"));
        // A disabled rule is never active, even without errors.
        let off = rule("a", "C:\\in", "to-webp", false);
        assert!(!build_status(&off, None, None, None, 0, None).active);
    }

    #[test]
    fn decide_runs_skips_or_blocks() {
        let config = WheelConfig::builtin();
        let s = settings(vec![
            rule("a", "C:\\in", "to-webp", true),
            rule("off", "C:\\off", "to-webp", false),
            rule("gone", "C:\\gone", "nope", true),
        ]);
        let dir = std::env::temp_dir().join(format!("fc-decide-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("a.png");
        let txt = dir.join("a.zzz");
        std::fs::write(&png, b"x").unwrap();
        std::fs::write(&txt, b"x").unwrap();
        assert_eq!(
            decide(&s, &config, "a", &png),
            Decision::Run {
                preset: "to-webp".into(),
                out_dir: PathBuf::from("C:\\in").join("converted")
            }
        );
        assert_eq!(decide(&s, &config, "a", &txt), Decision::Skip);
        assert_eq!(decide(&s, &config, "off", &png), Decision::Skip);
        assert_eq!(decide(&s, &config, "nobody", &png), Decision::Skip);
        assert_eq!(
            decide(&s, &config, "gone", &png),
            Decision::Blocked("presetMissing")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn counters_record_results() {
        let mut c = Counters::default();
        c.record(Path::new("C:\\in\\a.png"), Ok(()));
        c.record(
            Path::new("C:\\in\\b.png"),
            Err(job_error("io", "disk full")),
        );
        assert_eq!((c.queued, c.last_file.as_deref()), (1, Some("a.png")));
        assert_eq!(c.error.as_deref(), Some("failed:disk full"));
        c.record(Path::new("C:\\in\\c.png"), Ok(()));
        assert_eq!((c.queued, c.error.as_deref()), (2, None));
        assert_eq!(job_error("missingTool", "ffmpeg"), "missingTool:ffmpeg");
    }

    /// End to end with a real folder: existing files and our own `converted` outputs never
    /// fire; a file that is still growing fires only once it settles.
    #[test]
    fn watcher_end_to_end() {
        let dir = std::env::temp_dir().join(format!("fc-watch-e2e-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("old.png"), b"old").unwrap();
        let s = settings(vec![rule("a", &dir.to_string_lossy(), "to-webp", true)]);
        let specs = super::specs_from_settings(&s, &WheelConfig::builtin());
        let (tx, rx) = mpsc::channel();
        let watcher = Watcher::start(
            Duration::from_secs(1),
            Duration::from_secs(2),
            move |id, path| {
                let _ = tx.send((id, path));
            },
        );
        watcher.set_specs(specs);

        // Our own output folder.
        std::fs::create_dir_all(dir.join("converted")).unwrap();
        std::fs::write(dir.join("converted").join("x.webp"), b"out").unwrap();
        // A file written in chunks for ~3 s.
        let new = dir.join("new.png");
        let started = Instant::now();
        let mut f = std::fs::File::create(&new).unwrap();
        for _ in 0..6 {
            std::io::Write::write_all(&mut f, b"chunk").unwrap();
            std::io::Write::flush(&mut f).unwrap();
            std::thread::sleep(Duration::from_millis(500));
        }
        drop(f);
        let (id, path) = rx.recv_timeout(Duration::from_secs(10)).expect("new.png");
        assert_eq!(id, "a");
        assert_eq!(path, new);
        assert!(
            started.elapsed() >= Duration::from_secs(3),
            "fired while growing"
        );
        // Nothing else (old.png, converted\x.webp) fires.
        assert!(rx.recv_timeout(Duration::from_secs(4)).is_err());
        drop(watcher);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// With subfolders on, `converted` is still never entered; other subfolders are.
    #[test]
    fn subfolders_still_exclude_converted() {
        let dir = std::env::temp_dir().join(format!("fc-watch-sub-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("converted").join("deep")).unwrap();
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        let mut r = rule("a", &dir.to_string_lossy(), "to-webp", true);
        r.subfolders = true;
        let specs = super::specs_from_settings(&settings(vec![r]), &WheelConfig::builtin());
        assert!(specs[0].recursive);
        let (tx, rx) = mpsc::channel();
        let watcher = Watcher::start(
            Duration::from_secs(1),
            Duration::from_secs(1),
            move |_, path| {
                let _ = tx.send(path);
            },
        );
        watcher.set_specs(specs);
        std::fs::write(dir.join("converted").join("out.webp"), b"o").unwrap();
        std::fs::write(dir.join("converted").join("deep").join("o2.png"), b"o").unwrap();
        let wanted = dir.join("sub").join("new.png");
        std::fs::write(&wanted, b"n").unwrap();
        assert_eq!(rx.recv_timeout(Duration::from_secs(10)).unwrap(), wanted);
        assert!(rx.recv_timeout(Duration::from_secs(3)).is_err());
        drop(watcher);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

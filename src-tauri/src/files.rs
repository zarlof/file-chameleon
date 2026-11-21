//! The files rope's shell side: recording files (converted outputs, downloads, added inputs),
//! listing them with thumbnails for the line, and the commands' work. The store itself is
//! `fc_core::files::FileStore`. Recording runs off the calling thread and only logs failures.

use crate::events;
use crate::state::AppState;
use fc_core::files::{FileSource, FileStore};
use fc_core::formats::{self, Kind};
use fc_core::settings::LineFilesSources;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager};

/// Longest side of a files-rope thumbnail.
pub const THUMB_SIDE: u32 = 256;
/// Thumbnails made inline per `line_files` call; the rest follow in the background.
const INLINE_THUMBS: usize = 40;

/// One card of the files rope (TS `LineFile`).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LineFile {
    pub path: String,
    pub name: String,
    pub source: FileSource,
    pub at: u64,
    pub size: u64,
    pub kind: Option<Kind>,
    pub thumb: Option<String>,
}

/// File name of the cached thumbnail of `path`: a stable 64-bit FNV-1a hash of the path (case
/// and slash direction do not matter, like the store's keys) plus `.png`.
pub fn thumb_name(path: &Path) -> String {
    let key = path.to_string_lossy().replace('/', "\\").to_lowercase();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in key.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}.png")
}

/// The sources the settings let the rope show.
pub fn enabled_sources(s: &LineFilesSources) -> Vec<FileSource> {
    [
        (s.added, FileSource::Added),
        (s.converted, FileSource::Converted),
        (s.downloaded, FileSource::Downloaded),
    ]
    .into_iter()
    .filter_map(|(on, src)| on.then_some(src))
    .collect()
}

/// Which of `paths` to record as "added": only existing files that are not folders, not in the
/// clipboard store (`is_clip`) and not already listed (`known`, compared like the store does).
/// A file that is already on the rope keeps its source (a converted file that is converted again
/// stays "converted").
pub fn added_candidates(
    paths: &[PathBuf],
    is_clip: impl Fn(&Path) -> bool,
    known: &[PathBuf],
) -> Vec<PathBuf> {
    let known: Vec<String> = known.iter().map(|p| norm(p)).collect();
    paths
        .iter()
        .filter(|p| p.is_file() && !is_clip(p) && !known.contains(&norm(p)))
        .cloned()
        .collect()
}

const ALL: [FileSource; 3] = [
    FileSource::Added,
    FileSource::Converted,
    FileSource::Downloaded,
];

fn lock(state: &AppState) -> std::sync::MutexGuard<'_, FileStore> {
    state.files().lock().unwrap_or_else(|e| e.into_inner())
}

/// Store key of a path (case and slash direction do not matter).
fn norm(p: &Path) -> String {
    p.to_string_lossy().replace('/', "\\").to_lowercase()
}

/// The `paths` that are current entries of the rope (`entries`), matched by the store's key.
/// Only these may be opened, revealed, copied or dragged: the commands never act on an
/// arbitrary path a page sends.
pub fn members(entries: &[PathBuf], paths: &[PathBuf]) -> Vec<PathBuf> {
    let keys: Vec<String> = entries.iter().map(|p| norm(p)).collect();
    paths
        .iter()
        .filter(|p| keys.contains(&norm(p)))
        .cloned()
        .collect()
}

/// Which sources have entries (added, converted, downloaded).
fn presence(entries: &[fc_core::files::FileEntry]) -> [bool; 3] {
    let has = |s: FileSource| entries.iter().any(|e| e.source == s);
    [
        has(FileSource::Added),
        has(FileSource::Converted),
        has(FileSource::Downloaded),
    ]
}

/// Does a rope showing `s` have content, given which sources have entries?
fn any_shown(p: [bool; 3], s: &LineFilesSources) -> bool {
    (p[0] && s.added) || (p[1] && s.converted) || (p[2] && s.downloaded)
}

/// Cached "which sources have entries", so showing the line never stats the whole list. Updated
/// by record, remove, prune and list; `None` until first needed.
static PRESENT: Mutex<Option<[bool; 3]>> = Mutex::new(None);

/// Store the new presence; when it changes whether the rope has content, re-fit the line (if it
/// is up). Never call with the store locked: re-fitting asks for the rope count.
fn set_presence(app: &AppHandle, new: [bool; 3]) {
    let old = PRESENT
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .replace(new);
    let s = app.state::<AppState>().settings().line_files_sources;
    if old.is_some_and(|o| any_shown(o, &s) != any_shown(new, &s)) {
        crate::line::refit(app);
    }
}

/// Record `paths` as `source` and tell the line. Runs on its own thread; failures are logged.
pub fn record(app: &AppHandle, paths: Vec<PathBuf>, source: FileSource) {
    if paths.is_empty() {
        return;
    }
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("fc-files-record".into())
        .spawn(move || {
            let state = app.state::<AppState>();
            let result = {
                let mut store = lock(&state);
                let paths = if source == FileSource::Added {
                    let known: Vec<PathBuf> =
                        store.list(&ALL).into_iter().map(|e| e.path).collect();
                    added_candidates(&paths, |p| state.paths.is_clipboard_file(p), &known)
                } else {
                    paths.into_iter().filter(|p| p.is_file()).collect()
                };
                store
                    .record(&paths, source, fc_core::now_ms())
                    .map(|()| (!paths.is_empty()).then(|| presence(&store.list(&ALL))))
            };
            match result {
                Ok(Some(p)) => {
                    let _ = app.emit(events::FILES_CHANGED, ());
                    set_presence(&app, p);
                }
                Ok(None) => {}
                Err(e) => eprintln!("files rope: {e}"),
            }
        });
    if let Err(e) = spawned {
        eprintln!("files rope: {e}");
    }
}

/// Is there anything on the rope for `sources` (the NEW settings may not be saved yet)? Answered
/// from the cache of which sources have entries.
pub fn any_listed(app: &AppHandle, sources: &LineFilesSources) -> bool {
    let state = app.state::<AppState>();
    let cached = *PRESENT.lock().unwrap_or_else(|e| e.into_inner());
    let p = cached.unwrap_or_else(|| {
        let p = presence(&lock(&state).list(&ALL));
        *PRESENT.lock().unwrap_or_else(|e| e.into_inner()) = Some(p);
        p
    });
    any_shown(p, sources)
}

/// Thumbnails that could not be made this session (not an image we can read, over the pixel cap,
/// a failed write), by thumbnail name: they are not read again on every list.
static FAILED: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn thumb_failed(name: &str) -> bool {
    FAILED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .any(|n| n == name)
}

fn make_thumb(src: &Path, dst: &Path) -> bool {
    let ok = crate::ipc::images::write_file_thumbnail(src, dst, THUMB_SIDE);
    if !ok && let Some(name) = dst.file_name() {
        FAILED
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(name.to_string_lossy().into_owned());
    }
    ok
}

/// `line_files`: the rope's files, newest first, with thumbnails of images (made lazily).
pub fn list(app: &AppHandle) -> Vec<LineFile> {
    let state = app.state::<AppState>();
    let sources = enabled_sources(&state.settings().line_files_sources);
    let all = lock(&state).list(&ALL);
    set_presence(app, presence(&all));
    let entries = all.into_iter().filter(|e| sources.contains(&e.source));
    let dir = state.paths.files_thumbs_dir();
    let mut made = 0;
    let mut later: Vec<(PathBuf, PathBuf)> = Vec::new();
    let files = entries
        .map(|e| {
            let kind = formats::detect_path(&e.path).map(|f| f.kind);
            let size = std::fs::metadata(&e.path).map(|m| m.len()).unwrap_or(0);
            let mut thumb = None;
            if kind == Some(Kind::Image) {
                let name = thumb_name(&e.path);
                let dst = dir.join(&name);
                if thumb_fresh(&e.path, &dst) {
                    thumb = Some(dst);
                } else if thumb_failed(&name) {
                    // Known not to work: no card thumbnail, no re-read.
                } else if made < INLINE_THUMBS {
                    made += 1;
                    if make_thumb(&e.path, &dst) {
                        thumb = Some(dst);
                    }
                } else {
                    later.push((e.path.clone(), dst));
                }
            }
            LineFile {
                name: e
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                path: e.path.to_string_lossy().into_owned(),
                source: e.source,
                at: e.at,
                size,
                kind,
                thumb: thumb.map(|t| t.to_string_lossy().into_owned()),
            }
        })
        .collect();
    if !later.is_empty() {
        finish_thumbs_later(app, later);
    }
    files
}

/// Only one background thumbnail worker at a time.
static THUMB_WORKER: AtomicBool = AtomicBool::new(false);

/// Make the thumbnails `list` had no time for, then tell the page to fetch the list again (which
/// continues with what is still missing). A call while a worker runs does nothing.
fn finish_thumbs_later(app: &AppHandle, jobs: Vec<(PathBuf, PathBuf)>) {
    if THUMB_WORKER.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("fc-files-thumbs".into())
        .spawn(move || {
            let mut any = false;
            for (src, dst) in jobs {
                any |= make_thumb(&src, &dst);
            }
            THUMB_WORKER.store(false, Ordering::SeqCst);
            if any {
                let _ = app.emit(events::FILES_CHANGED, ());
            }
        });
    if spawned.is_err() {
        THUMB_WORKER.store(false, Ordering::SeqCst);
    }
}

/// Is the cached thumbnail there and at least as new as its source?
fn thumb_fresh(src: &Path, dst: &Path) -> bool {
    let time = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    match (time(src), time(dst)) {
        (Some(s), Some(d)) => d >= s,
        _ => false,
    }
}

/// `line_file_remove`: take the files off the rope (the files themselves stay) and delete their
/// cached thumbnails.
pub fn remove(app: &AppHandle, paths: &[PathBuf]) -> fc_core::Result<()> {
    let state = app.state::<AppState>();
    let p = {
        let mut store = lock(&state);
        store.remove(paths)?;
        presence(&store.list(&ALL))
    };
    let dir = state.paths.files_thumbs_dir();
    for path in paths {
        let _ = std::fs::remove_file(dir.join(thumb_name(path)));
    }
    let _ = app.emit(events::FILES_CHANGED, ());
    set_presence(app, p);
    Ok(())
}

/// The entries of `paths` that are on the rope. A path that is not (never listed, or its file is
/// gone and was just pruned) is left out and, when the file was pruned, the page is told.
pub fn listed(app: &AppHandle, paths: &[String]) -> Vec<PathBuf> {
    let state = app.state::<AppState>();
    // `list` prunes what no longer exists.
    let all = lock(&state).list(&ALL);
    set_presence(app, presence(&all));
    let entries: Vec<PathBuf> = all.into_iter().map(|e| e.path).collect();
    let wanted: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    let found = members(&entries, &wanted);
    if found.len() < wanted.len() {
        let _ = app.emit(events::FILES_CHANGED, ());
    }
    found
}

/// Is `name` (a file in `files-thumbs`) a thumbnail nothing on the rope uses any more?
pub fn is_orphan(name: &str, keep: &[String]) -> bool {
    name.ends_with(".png") && !keep.iter().any(|k| k == name)
}

const SWEEP_EVERY: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

/// Delete thumbnails of files that left the rope (pruned, removed in an earlier run) and old temp
/// files. At most once a day (a marker file in the folder).
pub fn sweep_thumbs(app: &AppHandle) {
    let state = app.state::<AppState>();
    let dir = state.paths.files_thumbs_dir();
    let marker = dir.join(".swept");
    let recent = std::fs::metadata(&marker)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age < SWEEP_EVERY);
    if recent {
        return;
    }
    let keep: Vec<String> = lock(&state)
        .list(&ALL)
        .iter()
        .map(|e| thumb_name(&e.path))
        .collect();
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return;
    };
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let stale_tmp = name.ends_with(".tmp")
            && entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age > std::time::Duration::from_secs(3600));
        if is_orphan(&name, &keep) || stale_tmp {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    let _ = std::fs::write(&marker, b"");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_thumb_name_stable() {
        let a = thumb_name(Path::new(r"C:\x\a.png"));
        assert_eq!(a, thumb_name(Path::new(r"C:\x\a.png")));
        // Paths compare case-insensitively and slashes are alike, like the store's keys.
        assert_eq!(a, thumb_name(Path::new(r"c:\X\A.PNG")));
        assert_eq!(a, thumb_name(Path::new("C:/x/a.png")));
        assert_ne!(a, thumb_name(Path::new(r"C:\x\b.png")));
        assert!(a.ends_with(".png") && a.len() == 20, "{a}");
    }

    #[test]
    fn sources_follow_the_settings() {
        let all = LineFilesSources::default();
        assert_eq!(
            enabled_sources(&all),
            [
                FileSource::Added,
                FileSource::Converted,
                FileSource::Downloaded
            ]
        );
        let some = LineFilesSources {
            added: false,
            converted: true,
            downloaded: false,
        };
        assert_eq!(enabled_sources(&some), [FileSource::Converted]);
    }

    #[test]
    fn added_skips_known_folders_clips_and_missing() {
        let d = std::env::temp_dir().join(format!("fc-files-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("sub")).unwrap();
        let (a, b, c) = (d.join("a.png"), d.join("b.png"), d.join("clip.png"));
        for p in [&a, &b, &c] {
            std::fs::write(p, b"x").unwrap();
        }
        let paths = vec![
            a.clone(),
            b.clone(),
            c.clone(),
            d.join("sub"),
            d.join("gone.png"),
        ];
        let got = added_candidates(&paths, |p| p == c, std::slice::from_ref(&b));
        assert_eq!(got, vec![a]);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn only_listed_paths_are_members() {
        let entries = vec![PathBuf::from(r"C:\x\a.png"), PathBuf::from(r"C:\x\b.exe")];
        let asked = vec![
            PathBuf::from("c:/X/A.PNG"),
            PathBuf::from(r"C:\Windows\notepad.exe"),
            PathBuf::from(r"C:\x\b.exe"),
        ];
        assert_eq!(
            members(&entries, &asked),
            vec![asked[0].clone(), asked[2].clone()]
        );
        assert!(members(&entries, &[PathBuf::from(r"C:\x")]).is_empty());
        assert!(members(&[], &asked).is_empty());
    }

    #[test]
    fn orphan_thumbs_are_found() {
        let keep = vec!["aaaa.png".to_string()];
        assert!(!is_orphan("aaaa.png", &keep));
        assert!(is_orphan("bbbb.png", &keep));
        assert!(!is_orphan(".swept", &keep));
        assert!(!is_orphan("cccc.png.1.tmp", &keep));
    }

    #[test]
    fn presence_decides_whether_the_rope_has_content() {
        let only_converted = [false, true, false];
        let all = LineFilesSources::default();
        assert!(any_shown(only_converted, &all));
        let no_converted = LineFilesSources {
            converted: false,
            ..all
        };
        assert!(!any_shown(only_converted, &no_converted));
        assert!(!any_shown([false; 3], &all));
    }
}

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
    let norm = |p: &Path| p.to_string_lossy().replace('/', "\\").to_lowercase();
    let known: Vec<String> = known.iter().map(|p| norm(p)).collect();
    paths
        .iter()
        .filter(|p| p.is_file() && !is_clip(p) && !known.contains(&norm(p)))
        .cloned()
        .collect()
}

fn lock(state: &AppState) -> std::sync::MutexGuard<'_, FileStore> {
    state.files().lock().unwrap_or_else(|e| e.into_inner())
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
                    let known: Vec<PathBuf> = store
                        .list(&[
                            FileSource::Added,
                            FileSource::Converted,
                            FileSource::Downloaded,
                        ])
                        .into_iter()
                        .map(|e| e.path)
                        .collect();
                    added_candidates(&paths, |p| state.paths.is_clipboard_file(p), &known)
                } else {
                    paths.into_iter().filter(|p| p.is_file()).collect()
                };
                store
                    .record(&paths, source, fc_core::now_ms())
                    .map(|()| !paths.is_empty())
            };
            match result {
                Ok(true) => {
                    let _ = app.emit(events::FILES_CHANGED, ());
                }
                Ok(false) => {}
                Err(e) => eprintln!("files rope: {e}"),
            }
        });
    if let Err(e) = spawned {
        eprintln!("files rope: {e}");
    }
}

/// Is there anything on the rope for the sources the settings show?
pub fn any_listed(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let sources = enabled_sources(&state.settings().line_files_sources);
    !lock(&state).list(&sources).is_empty()
}

/// `line_files`: the rope's files, newest first, with thumbnails of images (made lazily).
pub fn list(app: &AppHandle) -> Vec<LineFile> {
    let state = app.state::<AppState>();
    let sources = enabled_sources(&state.settings().line_files_sources);
    let entries = lock(&state).list(&sources);
    let dir = state.paths.files_thumbs_dir();
    let mut made = 0;
    let mut later: Vec<(PathBuf, PathBuf)> = Vec::new();
    let files = entries
        .into_iter()
        .map(|e| {
            let kind = formats::detect_path(&e.path).map(|f| f.kind);
            let size = std::fs::metadata(&e.path).map(|m| m.len()).unwrap_or(0);
            let mut thumb = None;
            if kind == Some(Kind::Image) {
                let dst = dir.join(thumb_name(&e.path));
                if thumb_fresh(&e.path, &dst) {
                    thumb = Some(dst);
                } else if made < INLINE_THUMBS {
                    made += 1;
                    if crate::ipc::images::write_file_thumbnail(&e.path, &dst, THUMB_SIDE) {
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

/// Make the thumbnails `list` had no time for, then tell the page to fetch the list again.
fn finish_thumbs_later(app: &AppHandle, jobs: Vec<(PathBuf, PathBuf)>) {
    let app = app.clone();
    let _ = std::thread::Builder::new()
        .name("fc-files-thumbs".into())
        .spawn(move || {
            let mut any = false;
            for (src, dst) in jobs {
                any |= crate::ipc::images::write_file_thumbnail(&src, &dst, THUMB_SIDE);
            }
            if any {
                let _ = app.emit(events::FILES_CHANGED, ());
            }
        });
}

/// Is the cached thumbnail there and at least as new as its source?
fn thumb_fresh(src: &Path, dst: &Path) -> bool {
    let time = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    match (time(src), time(dst)) {
        (Some(s), Some(d)) => d >= s,
        _ => false,
    }
}

/// `line_file_remove`: take the files off the rope (the files themselves stay).
pub fn remove(app: &AppHandle, paths: &[PathBuf]) -> fc_core::Result<()> {
    let state = app.state::<AppState>();
    lock(&state).remove(paths)?;
    let _ = app.emit(events::FILES_CHANGED, ());
    Ok(())
}

/// Drop paths that are gone from the rope and tell the page (a click on a deleted file).
pub fn forget_missing(app: &AppHandle) {
    let state = app.state::<AppState>();
    // `list` prunes what no longer exists.
    let _ = lock(&state).list(&[]);
    let _ = app.emit(events::FILES_CHANGED, ());
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
}

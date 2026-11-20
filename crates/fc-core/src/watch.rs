//! Generic polling folder watcher (settle time, temp-file filter, own-output ignore).
//!
//! [`Scanner`] is the pure part (explicit clock, no threads); [`Watcher`] runs it on one thread.
//! A file fires once its size and modified time have not changed for `settle` and it can be
//! opened for reading; folders already holding files when watching starts never fire for those.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime};

const MAX_DEPTH: usize = 8;
const MAX_IGNORED: usize = 10_000;
/// Globals: no polling loop faster than this.
const MIN_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchSpec {
    pub id: String,
    pub dir: PathBuf,
    pub recursive: bool,
    /// Folders never entered (e.g. the output folder `converted`).
    pub exclude: Vec<PathBuf>,
    /// Lowercase-insensitive extensions without dot; `None` = any file.
    pub extensions: Option<Vec<String>>,
}

/// Names that are half-written or editor lock files.
pub fn is_temp_name(name: &str) -> bool {
    let n = name.to_lowercase();
    n.starts_with("~$")
        || n.starts_with(".~lock")
        || [".crdownload", ".part", ".partial", ".tmp", ".download"]
            .iter()
            .any(|e| n.ends_with(e))
}

fn norm(p: &Path) -> PathBuf {
    PathBuf::from(p.to_string_lossy().to_lowercase())
}

struct Seen {
    size: u64,
    mtime: Option<SystemTime>,
    /// When this size/mtime was first observed.
    since: Instant,
    fired: bool,
}

struct SpecState {
    spec: WatchSpec,
    seen: HashMap<PathBuf, Seen>,
    baselined: bool,
    error: Option<String>,
}

pub struct Scanner {
    settle: Duration,
    specs: Vec<SpecState>,
    ignored: HashSet<PathBuf>,
    /// Insertion order of `ignored`, oldest first.
    ignored_order: VecDeque<PathBuf>,
}

impl Scanner {
    pub fn new(settle: Duration) -> Self {
        Self {
            settle,
            specs: Vec::new(),
            ignored: HashSet::new(),
            ignored_order: VecDeque::new(),
        }
    }

    /// Replace the watched folders. A spec identical to a current one keeps its state; any other
    /// takes a baseline now: files already there never fire.
    pub fn set_specs(&mut self, specs: Vec<WatchSpec>, now: Instant) {
        let mut old = std::mem::take(&mut self.specs);
        for spec in specs {
            if let Some(pos) = old.iter().position(|s| s.spec == spec) {
                self.specs.push(old.swap_remove(pos));
                continue;
            }
            let mut st = SpecState {
                spec,
                seen: HashMap::new(),
                baselined: false,
                error: None,
            };
            self.scan(&mut st, now);
            self.specs.push(st);
        }
    }

    /// Never fire for this path (an output the app wrote itself).
    pub fn ignore(&mut self, path: PathBuf) {
        let key = norm(&path);
        if self.ignored.insert(key.clone()) {
            self.ignored_order.push_back(key);
        }
        // Evict the oldest entries one by one: clearing everything would make self-written
        // outputs look new again.
        while self.ignored_order.len() > MAX_IGNORED {
            if let Some(old) = self.ignored_order.pop_front() {
                self.ignored.remove(&old);
            }
        }
    }

    /// Scan every folder; returns `(spec id, path)` of files that settled since the last tick.
    pub fn tick(&mut self, now: Instant) -> Vec<(String, PathBuf)> {
        let mut specs = std::mem::take(&mut self.specs);
        let mut fired = Vec::new();
        for st in &mut specs {
            for path in self.scan(st, now) {
                fired.push((st.spec.id.clone(), path));
            }
        }
        self.specs = specs;
        fired
    }

    /// `(spec id, message)` for folders that could not be read at the last scan.
    pub fn errors(&self) -> Vec<(String, String)> {
        self.specs
            .iter()
            .filter_map(|s| Some((s.spec.id.clone(), s.error.clone()?)))
            .collect()
    }

    fn collect(&self, spec: &WatchSpec) -> Result<Vec<PathBuf>, String> {
        let excluded: Vec<PathBuf> = spec.exclude.iter().map(|p| norm(p)).collect();
        let exts: Option<Vec<String>> = spec.extensions.as_ref().map(|e| {
            e.iter()
                .map(|x| x.trim_start_matches('.').to_lowercase())
                .collect()
        });
        let mut files = Vec::new();
        let mut stack = vec![(spec.dir.clone(), 0usize)];
        while let Some((dir, depth)) = stack.pop() {
            let entries = match std::fs::read_dir(&dir) {
                Ok(e) => e,
                Err(e) if depth == 0 => return Err(e.to_string()),
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let Ok(ft) = entry.file_type() else { continue };
                if ft.is_dir() {
                    let key = norm(&path);
                    if spec.recursive && depth < MAX_DEPTH && !excluded.contains(&key) {
                        stack.push((path, depth + 1));
                    }
                    continue;
                }
                if !ft.is_file() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                if is_temp_name(&name) || self.ignored.contains(&norm(&path)) {
                    continue;
                }
                if let Some(exts) = &exts {
                    let ext = path
                        .extension()
                        .map(|e| e.to_string_lossy().to_lowercase())
                        .unwrap_or_default();
                    if !exts.contains(&ext) {
                        continue;
                    }
                }
                files.push(path);
            }
        }
        Ok(files)
    }

    /// One pass over a spec. Returns the files that settled.
    fn scan(&self, st: &mut SpecState, now: Instant) -> Vec<PathBuf> {
        let files = match self.collect(&st.spec) {
            Ok(f) => f,
            Err(msg) => {
                st.error = Some(msg);
                st.seen.clear();
                // A folder that comes back later is baselined again.
                st.baselined = false;
                return Vec::new();
            }
        };
        st.error = None;
        let baseline = !st.baselined;
        st.baselined = true;
        let mut fired = Vec::new();
        let mut alive = HashSet::new();
        for path in files {
            let Ok(meta) = std::fs::metadata(&path) else {
                continue;
            };
            let (size, mtime) = (meta.len(), meta.modified().ok());
            alive.insert(path.clone());
            match st.seen.get_mut(&path) {
                Some(s) if s.size == size && s.mtime == mtime => {
                    if !s.fired && now.duration_since(s.since) >= self.settle {
                        // Still locked by the writer: try again on the next tick.
                        if std::fs::File::open(&path).is_ok() {
                            s.fired = true;
                            fired.push(path);
                        }
                    }
                }
                Some(s) => {
                    *s = Seen {
                        size,
                        mtime,
                        since: now,
                        fired: false,
                    };
                }
                None => {
                    st.seen.insert(
                        path,
                        Seen {
                            size,
                            mtime,
                            since: now,
                            fired: baseline,
                        },
                    );
                }
            }
        }
        st.seen.retain(|p, _| alive.contains(p));
        fired.sort();
        fired
    }
}

/// [`Scanner`] on its own thread. Dropping it stops the thread.
pub struct Watcher {
    scanner: Arc<Mutex<Scanner>>,
    stop: Option<mpsc::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

fn lock(m: &Mutex<Scanner>) -> MutexGuard<'_, Scanner> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Watcher {
    /// Poll every `interval` (at least 1 s); `on_file(spec id, path)` runs on the watcher thread.
    pub fn start(
        interval: Duration,
        settle: Duration,
        on_file: impl Fn(String, PathBuf) + Send + 'static,
    ) -> Watcher {
        let scanner = Arc::new(Mutex::new(Scanner::new(settle)));
        let (stop, rx) = mpsc::channel::<()>();
        let interval = interval.max(MIN_INTERVAL);
        let shared = Arc::clone(&scanner);
        let thread = std::thread::spawn(move || {
            while let Err(RecvTimeoutError::Timeout) = rx.recv_timeout(interval) {
                let fired = lock(&shared).tick(Instant::now());
                for (id, path) in fired {
                    on_file(id, path);
                }
            }
        });
        Watcher {
            scanner,
            stop: Some(stop),
            thread: Some(thread),
        }
    }

    pub fn set_specs(&self, specs: Vec<WatchSpec>) {
        lock(&self.scanner).set_specs(specs, Instant::now());
    }

    pub fn ignore(&self, path: PathBuf) {
        lock(&self.scanner).ignore(path);
    }

    pub fn errors(&self) -> Vec<(String, String)> {
        lock(&self.scanner).errors()
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        drop(self.stop.take());
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

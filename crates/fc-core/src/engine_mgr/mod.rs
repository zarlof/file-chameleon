//! On-demand external tools. OWNER: Task C.
//!
//! Layout: `<root>/<id>/` holds the extracted files plus `installed.json`
//! (`{ "version": "...", "sha256": "..." }`). Install = download to `<root>/.staging/<id>.download`
//! (resumable not required), verify SHA-256, extract only the manifest `files` (archive entries are
//! matched by path *suffix*, so a top-level version folder in the zip is fine), then atomically
//! rename the staging folder to `<root>/<id>`. Any failure removes staging. Max download 400 MB.
//! Resolution (`locate`): managed install first, then the system PATH (`system` names),
//! then nothing. yt-dlp: verify against the line for `yt-dlp.exe` in `sha256Url`
//! (SHA2-256SUMS format: `<hex>  <name>`), and enforce `minVersion` on system copies
//! (`yt-dlp --version` prints `YYYY.MM.DD`).

pub mod manifest;

use crate::engines::ToolLocator;
use crate::proc;
use crate::{CancelToken, Error, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use manifest::{Archive, FileMap};
pub use manifest::{Manifest, ToolEntry};

/// Hard cap for a single download.
const MAX_DOWNLOAD: u64 = 400 * 1024 * 1024;
/// Hard cap for one extracted file (zip-bomb guard).
const MAX_EXTRACTED: u64 = 1024 * 1024 * 1024;
const STALE_MS: u64 = 24 * 60 * 60 * 1000;
const VERSION_TIMEOUT: Duration = Duration::from_secs(3);

/// (tool id, exe) -> (exe modification time, version output).
type VersionCache = HashMap<(String, PathBuf), (SystemTime, Option<String>)>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolSource {
    Managed,
    System,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatus {
    pub id: String,
    pub name: String,
    pub purpose: String,
    pub license: String,
    pub source: ToolSource,
    /// Directory or file in use, when not missing.
    pub path: Option<PathBuf>,
    pub version: Option<String>,
    /// Download size in bytes (from the manifest).
    pub size: u64,
}

/// Progress callback: (bytes_done, bytes_total).
pub type InstallProgress<'a> = &'a dyn Fn(u64, u64);

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Installed {
    version: String,
    sha256: String,
    #[serde(default)]
    checked_at: Option<u64>,
}

pub struct EngineManager {
    root: PathBuf,
    manifest: Manifest,
    /// PATH lookups (validated) cached for the process lifetime, keyed by lowercase file name.
    system_cache: Mutex<HashMap<String, Option<PathBuf>>>,
    /// `--version` results by (tool, exe) with the exe's modification time, so `status()` does
    /// not start processes on every call. Cleared on install/uninstall.
    version_cache: Mutex<VersionCache>,
    /// Serializes installs/uninstalls.
    install_lock: Mutex<()>,
}

impl EngineManager {
    /// `root` = `%LOCALAPPDATA%\FileChameleon\engines`. Creates it if missing.
    pub fn new(root: PathBuf, manifest: Manifest) -> Self {
        let _ = fs::create_dir_all(&root);
        let mgr = Self {
            root,
            manifest,
            system_cache: Mutex::new(HashMap::new()),
            version_cache: Mutex::new(HashMap::new()),
            install_lock: Mutex::new(()),
        };
        mgr.cleanup_old();
        mgr
    }

    /// Best-effort removal of `<id>.old-<ms>` folders left behind by earlier reinstalls
    /// (their files may have been in use at the time).
    fn cleanup_old(&self) {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_name().to_string_lossy().contains(".old-") {
                let _ = fs::remove_dir_all(path);
            } else if path.is_dir() {
                // Files moved aside by `swap_in_place`.
                for f in fs::read_dir(&path).into_iter().flatten().flatten() {
                    if f.file_name().to_string_lossy().contains(".old-") {
                        let _ = fs::remove_file(f.path());
                    }
                }
            }
        }
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn tool_dir(&self, id: &str) -> PathBuf {
        self.root.join(id)
    }

    fn staging_dir(&self) -> PathBuf {
        self.root.join(".staging")
    }

    fn read_installed(&self, id: &str) -> Option<Installed> {
        let text = fs::read_to_string(self.tool_dir(id).join("installed.json")).ok()?;
        serde_json::from_str(&text).ok()
    }

    fn is_managed(&self, entry: &ToolEntry) -> bool {
        let dir = self.tool_dir(&entry.id);
        !entry.files.is_empty() && entry.files.iter().all(|f| dir.join(&f.to).is_file())
    }

    /// PATH lookup, cached; yt-dlp style entries must satisfy `minVersion`.
    fn system_lookup(&self, entry: &ToolEntry, file: &str) -> Option<PathBuf> {
        if !entry.system.iter().any(|s| s.eq_ignore_ascii_case(file)) {
            return None;
        }
        let key = file.to_ascii_lowercase();
        if let Ok(cache) = self.system_cache.lock()
            && let Some(hit) = cache.get(&key)
        {
            return hit.clone();
        }
        let mut found = which(file);
        if let (Some(path), Some(min)) = (&found, &entry.min_version) {
            let ok = self
                .version_of(&entry.id, path)
                .map(|v| version_at_least(&v, min))
                .unwrap_or(false);
            if !ok {
                found = None;
            }
        }
        if let Ok(mut cache) = self.system_cache.lock() {
            cache.insert(key, found.clone());
        }
        found
    }

    /// `system_version`, cached per (tool, exe) until the exe's modification time changes.
    fn version_of(&self, tool: &str, exe: &Path) -> Option<String> {
        let mtime = fs::metadata(exe).and_then(|m| m.modified()).ok();
        let key = (tool.to_string(), exe.to_path_buf());
        if let (Some(t), Ok(cache)) = (mtime, self.version_cache.lock())
            && let Some((cached_at, v)) = cache.get(&key)
            && *cached_at == t
        {
            return v.clone();
        }
        let v = system_version(tool, exe);
        if let (Some(t), Ok(mut cache)) = (mtime, self.version_cache.lock()) {
            cache.insert(key, (t, v.clone()));
        }
        v
    }

    fn forget_versions(&self) {
        if let Ok(mut cache) = self.version_cache.lock() {
            cache.clear();
        }
    }

    /// True when tool `id` can be used now (managed install or a valid copy on PATH).
    pub fn is_available(&self, id: &str) -> bool {
        self.manifest
            .get(id)
            .and_then(|e| e.files.first())
            .is_some_and(|f| self.locate(id, &f.to).is_some())
    }

    pub fn status(&self) -> Vec<ToolStatus> {
        self.manifest
            .engines
            .iter()
            .map(|e| {
                let (source, path, version) = if self.is_managed(e) {
                    let mut version = self.read_installed(&e.id).map(|i| i.version);
                    if e.id == "yt-dlp" {
                        // "latest" is not informative; ask the binary.
                        let exe = self.tool_dir(&e.id).join(&e.files[0].to);
                        if let Some(v) = self.version_of(&e.id, &exe) {
                            version = Some(v);
                        }
                    }
                    (ToolSource::Managed, Some(self.tool_dir(&e.id)), version)
                } else if let Some(p) = e.files.first().and_then(|f| self.system_lookup(e, &f.to)) {
                    let v = self.version_of(&e.id, &p);
                    (ToolSource::System, Some(p), v)
                } else {
                    (ToolSource::Missing, None, None)
                };
                ToolStatus {
                    id: e.id.clone(),
                    name: e.name.clone(),
                    purpose: e.purpose.clone(),
                    license: e.license.clone(),
                    source,
                    path,
                    version,
                    size: e.size,
                }
            })
            .collect()
    }

    pub fn install(
        &self,
        id: &str,
        progress: InstallProgress<'_>,
        cancel: &CancelToken,
    ) -> Result<()> {
        let entry = self
            .manifest
            .get(id)
            .ok_or_else(|| Error::Unsupported(format!("unknown tool {id}")))?;
        let _guard = self.install_lock.lock().unwrap_or_else(|p| p.into_inner());
        self.cleanup_old();
        let staging = self.staging_dir();
        let download = staging.join(format!("{id}.download"));
        let extract_dir = staging.join(id);
        let result = self.install_inner(entry, &download, &extract_dir, progress, cancel);
        let _ = fs::remove_file(&download);
        let _ = fs::remove_dir_all(&extract_dir);
        self.forget_versions();
        result
    }

    fn install_inner(
        &self,
        entry: &ToolEntry,
        download: &Path,
        extract_dir: &Path,
        progress: InstallProgress<'_>,
        cancel: &CancelToken,
    ) -> Result<()> {
        if let Some(parent) = download.parent() {
            fs::create_dir_all(parent)?;
        }
        let _ = fs::remove_dir_all(extract_dir);
        let expected = expected_sha256(entry)?;
        let agent = http_agent();
        let actual = download_to(&agent, &entry.url, download, entry.size, progress, cancel)?;
        if !actual.eq_ignore_ascii_case(&expected) {
            return Err(Error::Checksum(entry.id.clone()));
        }
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        fs::create_dir_all(extract_dir)?;
        extract_archive(entry.archive, download, &entry.files, extract_dir)?;
        let installed = Installed {
            version: entry.version.clone(),
            sha256: actual,
            checked_at: Some(crate::now_ms()),
        };
        let json =
            serde_json::to_string_pretty(&installed).map_err(|e| Error::Failed(e.to_string()))?;
        fs::write(extract_dir.join("installed.json"), json)?;
        let target = self.tool_dir(&entry.id);
        // Never delete the live folder first: a running exe makes that fail half way.
        if !target.exists() {
            fs::rename(extract_dir, &target)?;
            return Ok(());
        }
        let stamp = crate::now_ms();
        let aside = self.root.join(format!("{}.old-{stamp}", entry.id));
        if fs::rename(&target, &aside).is_ok() {
            if let Err(e) = fs::rename(extract_dir, &target) {
                let _ = fs::rename(&aside, &target);
                return Err(e.into());
            }
            let _ = fs::remove_dir_all(aside);
            return Ok(());
        }
        // Windows refuses to rename a folder that has open files inside (e.g. a running exe).
        // The files themselves can usually be renamed, so swap them one by one.
        swap_in_place(&target, extract_dir, stamp)
    }

    /// Removes only a managed install; system copies are never touched.
    pub fn uninstall(&self, id: &str) -> Result<()> {
        self.manifest
            .get(id)
            .ok_or_else(|| Error::Unsupported(format!("unknown tool {id}")))?;
        let _guard = self.install_lock.lock().unwrap_or_else(|p| p.into_inner());
        let dir = self.tool_dir(id);
        let result = if dir.exists() {
            fs::remove_dir_all(dir)
        } else {
            Ok(())
        };
        self.forget_versions();
        Ok(result?)
    }

    /// Managed yt-dlp only: re-download latest if the installed one is older than 24 h
    /// since the last check (state kept in `installed.json` as `checkedAt`). Returns true if updated.
    pub fn update_ytdlp_if_stale(&self, cancel: &CancelToken) -> Result<bool> {
        self.update_ytdlp(false, cancel)
    }

    /// Managed yt-dlp: download the latest build when its checksum differs from the installed
    /// one. Without `force` this only checks once per 24 h; with `force` it always checks and
    /// fails with `MissingTool` when there is no managed copy. Returns true if updated.
    pub fn update_ytdlp(&self, force: bool, cancel: &CancelToken) -> Result<bool> {
        let missing = || {
            if force {
                Err(Error::MissingTool("yt-dlp".into()))
            } else {
                Ok(false)
            }
        };
        let Some(entry) = self.manifest.get("yt-dlp") else {
            return missing();
        };
        if !self.is_managed(entry) {
            return missing();
        }
        let Some(mut installed) = self.read_installed("yt-dlp") else {
            return missing();
        };
        let now = crate::now_ms();
        if !force
            && installed
                .checked_at
                .is_some_and(|t| now.saturating_sub(t) < STALE_MS)
        {
            return Ok(false);
        }
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let latest = expected_sha256(entry)?;
        if latest.eq_ignore_ascii_case(&installed.sha256) {
            installed.checked_at = Some(now);
            let json = serde_json::to_string_pretty(&installed)
                .map_err(|e| Error::Failed(e.to_string()))?;
            fs::write(self.tool_dir("yt-dlp").join("installed.json"), json)?;
            return Ok(false);
        }
        self.install("yt-dlp", &|_, _| {}, cancel)?;
        Ok(true)
    }
}

impl ToolLocator for EngineManager {
    fn locate(&self, tool: &str, file: &str) -> Option<PathBuf> {
        let entry = self.manifest.get(tool)?;
        let managed = self.tool_dir(tool).join(file);
        if managed.is_file() {
            return Some(managed);
        }
        self.system_lookup(entry, file)
    }
}

/// Replace the files of `target` with those of `new_dir` without renaming the folder:
/// old files are renamed to `<name>.old-<stamp>` (allowed for a running exe), new files are
/// moved in, then the old ones are deleted best effort (leftovers are removed later by
/// `cleanup_old`). On any failure everything is rolled back and the old install stays intact.
fn swap_in_place(target: &Path, new_dir: &Path, stamp: u64) -> Result<()> {
    let names = |dir: &Path| -> Result<Vec<std::ffi::OsString>> {
        let mut v = Vec::new();
        for e in fs::read_dir(dir)? {
            let e = e?;
            if e.file_type()?.is_file() {
                v.push(e.file_name());
            }
        }
        Ok(v)
    };
    let aside_name = |n: &std::ffi::OsString| {
        let mut a = n.clone();
        a.push(format!(".old-{stamp}"));
        a
    };
    let old = names(target)?;
    let new_names = names(new_dir)?;
    let mut moved_aside: Vec<&std::ffi::OsString> = Vec::new();
    let rollback_aside = |moved: &[&std::ffi::OsString]| {
        for n in moved {
            let _ = fs::rename(target.join(aside_name(n)), target.join(n));
        }
    };
    for n in &old {
        if let Err(e) = fs::rename(target.join(n), target.join(aside_name(n))) {
            rollback_aside(&moved_aside);
            return Err(e.into());
        }
        moved_aside.push(n);
    }
    let mut placed: Vec<std::ffi::OsString> = Vec::new();
    for n in new_names {
        if let Err(e) = fs::rename(new_dir.join(&n), target.join(&n)) {
            for p in &placed {
                let _ = fs::remove_file(target.join(p));
            }
            rollback_aside(&moved_aside);
            return Err(e.into());
        }
        placed.push(n);
    }
    for n in &old {
        let _ = fs::remove_file(target.join(aside_name(n)));
    }
    Ok(())
}

/// Search `PATH` for an executable file name. Exposed for tests.
pub fn which(file: &str) -> Option<PathBuf> {
    which_in(file, &std::env::var_os("PATH")?)
}

/// Like [`which`] but with an explicit PATH-style list (`;` on Windows, `:` elsewhere).
pub fn which_in(file: &str, path_var: &OsStr) -> Option<PathBuf> {
    std::env::split_paths(path_var)
        .map(|dir| dir.join(file))
        .find(|p| p.is_file())
}

/// Lowercase hex SHA-256 of a file.
pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Find the hash for `name` in SHA2-256SUMS text (`<hex>  <name>` or `<hex> *<name>`).
pub fn parse_sums(text: &str, name: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let mut it = line.split_whitespace();
        let hash = it.next()?;
        let file = it.next()?.trim_start_matches('*');
        (file == name && hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| hash.to_ascii_lowercase())
    })
}

/// `true` when dotted-numeric `version` >= `min` (e.g. `2026.10.26`).
pub fn version_at_least(version: &str, min: &str) -> bool {
    fn parts(v: &str) -> Vec<u64> {
        v.trim()
            .split('.')
            .map(|p| {
                p.chars()
                    .take_while(char::is_ascii_digit)
                    .collect::<String>()
                    .parse()
                    .unwrap_or(0)
            })
            .collect()
    }
    let (a, b) = (parts(version), parts(min));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (
            a.get(i).copied().unwrap_or(0),
            b.get(i).copied().unwrap_or(0),
        );
        if x != y {
            return x > y;
        }
    }
    true
}

/// Extract the version string from a tool's version output.
/// ffmpeg: first line, token 3 (`ffmpeg version 9.0.2-...`); yt-dlp: whole first line;
/// deno: first line, token 2 (`deno 2.9.8 (...)`).
pub fn parse_version_output(tool: &str, text: &str) -> Option<String> {
    let line = text.lines().next()?.trim();
    let v = match tool {
        "ffmpeg" => line.split_whitespace().nth(2)?,
        "deno" => line.split_whitespace().nth(1)?,
        _ => line,
    };
    (!v.is_empty()).then(|| v.to_string())
}

/// Run `<exe> -version`/`--version` with a 3 s timeout. Never runs anything for pdfium.
fn system_version(tool: &str, exe: &Path) -> Option<String> {
    let flag = match tool {
        "ffmpeg" => "-version",
        "yt-dlp" | "deno" => "--version",
        _ => return None,
    };
    let mut cmd = proc::command(exe);
    cmd.arg(flag);
    let cancel = CancelToken::new();
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let timer_cancel = cancel.clone();
    std::thread::spawn(move || {
        // Wakes immediately (Disconnected) when the command finished in time.
        if let Err(std::sync::mpsc::RecvTimeoutError::Timeout) = rx.recv_timeout(VERSION_TIMEOUT) {
            timer_cancel.cancel();
        }
    });
    let out = proc::run(cmd, &cancel);
    drop(tx);
    let out = out.ok()?;
    out.status
        .success()
        .then(|| parse_version_output(tool, &out.stdout))?
}

fn http_agent() -> ureq::Agent {
    use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
    let tls = TlsConfig::builder()
        .provider(TlsProvider::NativeTls)
        .root_certs(RootCerts::PlatformVerifier)
        .build();
    ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        // A stalled download must fail (and be cancellable) instead of blocking in a read.
        .timeout_per_read(Some(Duration::from_secs(30)))
        .user_agent("FileChameleon")
        .build()
        .into()
}

/// The pinned hash, or the one from `sha256Url` for the entry's file.
fn expected_sha256(entry: &ToolEntry) -> Result<String> {
    if let Some(h) = &entry.sha256 {
        return Ok(h.to_ascii_lowercase());
    }
    let url = entry
        .sha256_url
        .as_ref()
        .ok_or_else(|| Error::Failed(format!("{}: no checksum source", entry.id)))?;
    let name = entry
        .files
        .first()
        .and_then(|f| f.from.rsplit('/').next())
        .ok_or_else(|| Error::Failed(format!("{}: no files", entry.id)))?;
    let text = http_agent()
        .get(url)
        .call()
        .map_err(|e| Error::Network(e.to_string()))?
        .body_mut()
        .read_to_string()
        .map_err(|e| Error::Network(e.to_string()))?;
    parse_sums(&text, name).ok_or_else(|| Error::Checksum(entry.id.clone()))
}

/// Stream `url` to `dest`, returning its SHA-256. Enforces the 400 MB cap and honors `cancel`.
fn download_to(
    agent: &ureq::Agent,
    url: &str,
    dest: &Path,
    size_hint: u64,
    progress: InstallProgress<'_>,
    cancel: &CancelToken,
) -> Result<String> {
    let mut resp = agent
        .get(url)
        .call()
        .map_err(|e| Error::Network(e.to_string()))?;
    let total = resp.body().content_length().unwrap_or(size_hint);
    if total > MAX_DOWNLOAD {
        return Err(Error::TooLarge(url.to_string()));
    }
    let mut reader = resp.body_mut().as_reader();
    let mut out = fs::File::create(dest)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut done = 0u64;
    progress(0, total);
    loop {
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let n = reader
            .read(&mut buf)
            .map_err(|e| Error::Network(e.to_string()))?;
        if n == 0 {
            break;
        }
        done += n as u64;
        if done > MAX_DOWNLOAD {
            return Err(Error::TooLarge(url.to_string()));
        }
        hasher.update(&buf[..n]);
        out.write_all(&buf[..n])?;
        progress(done, total.max(done));
    }
    out.flush()?;
    Ok(hex(&hasher.finalize()))
}

fn normalize(name: &str) -> String {
    name.replace('\\', "/")
}

fn matches_entry(entry_name: &str, from: &str) -> bool {
    let n = normalize(entry_name);
    let f = from.trim_start_matches('/');
    n == f || n.ends_with(&format!("/{f}"))
}

fn safe_target(dest: &Path, to: &str) -> Result<PathBuf> {
    if to.is_empty() || to.contains("..") || Path::new(to).is_absolute() {
        return Err(Error::Failed(format!("bad target path {to}")));
    }
    Ok(dest.join(to))
}

fn copy_capped(src: &mut dyn Read, target: &Path) -> Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut out = fs::File::create(target)?;
    let copied = std::io::copy(&mut src.take(MAX_EXTRACTED + 1), &mut out)?;
    if copied > MAX_EXTRACTED {
        drop(out);
        let _ = fs::remove_file(target);
        return Err(Error::TooLarge(target.display().to_string()));
    }
    Ok(())
}

/// Extract only the mapped `files` from `archive` into `dest` (entries matched by path suffix).
/// Fails if any mapped file is missing. For `Archive::None` the file is copied as `files[0].to`.
pub fn extract_archive(
    kind: Archive,
    archive: &Path,
    files: &[FileMap],
    dest: &Path,
) -> Result<()> {
    let mut found = vec![false; files.len()];
    match kind {
        Archive::None => {
            let map = files
                .first()
                .ok_or_else(|| Error::Failed("no files".into()))?;
            let target = safe_target(dest, &map.to)?;
            copy_capped(&mut fs::File::open(archive)?, &target)?;
            found[0] = true;
        }
        Archive::Zip => {
            let mut zip = zip::ZipArchive::new(fs::File::open(archive)?)
                .map_err(|e| Error::Failed(format!("zip: {e}")))?;
            for i in 0..zip.len() {
                let mut entry = zip
                    .by_index(i)
                    .map_err(|e| Error::Failed(format!("zip: {e}")))?;
                if entry.is_dir() {
                    continue;
                }
                let Ok(name) = entry.name().map(|n| n.into_owned()) else {
                    continue;
                };
                for (k, map) in files.iter().enumerate() {
                    if !found[k] && matches_entry(&name, &map.from) {
                        copy_capped(&mut entry, &safe_target(dest, &map.to)?)?;
                        found[k] = true;
                        break;
                    }
                }
            }
        }
        Archive::TarGz => {
            let gz = flate2::read::GzDecoder::new(fs::File::open(archive)?);
            let mut tar = tar::Archive::new(gz);
            for entry in tar.entries()? {
                let mut entry = entry?;
                if !entry.header().entry_type().is_file() {
                    continue;
                }
                let name = entry.path()?.to_string_lossy().to_string();
                for (k, map) in files.iter().enumerate() {
                    if !found[k] && matches_entry(&name, &map.from) {
                        copy_capped(&mut entry, &safe_target(dest, &map.to)?)?;
                        found[k] = true;
                        break;
                    }
                }
            }
        }
    }
    if let Some(k) = found.iter().position(|f| !f) {
        return Err(Error::Failed(format!(
            "{} not found in archive",
            files[k].from
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare() {
        assert!(version_at_least("2026.10.26", "2026.10.26"));
        assert!(version_at_least("2026.11.01", "2026.10.26"));
        assert!(!version_at_least("2026.10.25", "2026.10.26"));
        assert!(version_at_least("2027.1.1", "2026.10.26"));
    }

    #[test]
    fn version_outputs() {
        assert_eq!(
            parse_version_output("ffmpeg", "ffmpeg version 9.0.2-essentials Copyright\nx"),
            Some("9.0.2-essentials".into())
        );
        assert_eq!(
            parse_version_output("deno", "deno 2.9.8 (stable, release)\nv8"),
            Some("2.9.8".into())
        );
        assert_eq!(
            parse_version_output("yt-dlp", "2026.10.26\n"),
            Some("2026.10.26".into())
        );
    }

    #[test]
    fn sums_parsing() {
        let h = "a".repeat(64);
        let text = format!("{h}  yt-dlp\n{h}  yt-dlp.exe\n{}  other", "b".repeat(64));
        assert_eq!(parse_sums(&text, "yt-dlp.exe"), Some(h));
        assert_eq!(parse_sums(&text, "nope"), None);
    }
}

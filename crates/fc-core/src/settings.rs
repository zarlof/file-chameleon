//! Persisted settings (`%APPDATA%\FileChameleon\settings.json`). Unknown/missing fields fall back
//! to defaults so old files keep loading.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// "system" | "tr" | "en"
    pub language: String,
    /// "system" | "light" | "dark"
    pub theme: String,
    /// "blue" | "purple" | "pink" | "red" | "orange" | "yellow" | "green" | "graphite"
    pub accent: String,
    pub launch_at_login: bool,

    /// `None` = next to the source file.
    pub output_dir: Option<PathBuf>,
    /// Default image quality 1..=100.
    pub image_quality: u8,

    /// `None` = the user's Downloads folder.
    pub download_dir: Option<PathBuf>,
    pub download_mode: crate::download::DlMode,
    pub download_quality: crate::download::DlQuality,
    /// Opt-in: browser name for yt-dlp `--cookies-from-browser` ("edge", "chrome", "firefox"...).
    pub cookies_browser: Option<String>,

    pub wheel_enabled: bool,
    /// false = only drags that start in Explorer / the desktop.
    pub wheel_any_app: bool,
    /// Most recently used target format ids, newest first, max 7 (shown first in the wheel).
    pub recent_targets: Vec<String>,

    pub clipboard_enabled: bool,
    pub clipboard_images: bool,
    pub clipboard_suggest_links: bool,
    pub clipboard_suggest_images: bool,
    pub clipboard_max_items: u32,
    pub clipboard_retention_days: u32,

    /// Set after the first-run welcome was shown.
    pub onboarded: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: "system".into(),
            theme: "system".into(),
            accent: "blue".into(),
            launch_at_login: false,
            output_dir: None,
            image_quality: 85,
            download_dir: None,
            download_mode: Default::default(),
            download_quality: Default::default(),
            cookies_browser: None,
            wheel_enabled: true,
            wheel_any_app: false,
            recent_targets: Vec::new(),
            clipboard_enabled: true,
            clipboard_images: true,
            clipboard_suggest_links: true,
            clipboard_suggest_images: true,
            clipboard_max_items: 100,
            clipboard_retention_days: 7,
            onboarded: false,
        }
    }
}

impl Settings {
    pub const MAX_RECENT_TARGETS: usize = 7;

    /// Missing or corrupt file -> defaults (never fails).
    pub fn load(path: &Path) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    /// Atomic write (temp + rename), creating the parent directory.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(
            &tmp,
            serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?,
        )?;
        std::fs::rename(&tmp, path)
    }

    /// Move `id` to the front of `recent_targets`, keep at most `MAX_RECENT_TARGETS`.
    pub fn touch_recent_target(&mut self, id: &str) {
        self.recent_targets.retain(|t| t != id);
        self.recent_targets.insert(0, id.to_string());
        self.recent_targets.truncate(Self::MAX_RECENT_TARGETS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_defaults_for_partial_files() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("s/settings.json");
        assert_eq!(Settings::load(&p), Settings::default());
        let s = Settings { language: "tr".into(), ..Settings::default() };
        s.save(&p).unwrap();
        assert_eq!(Settings::load(&p).language, "tr");
        std::fs::write(&p, br#"{"theme":"dark"}"#).unwrap();
        let s = Settings::load(&p);
        assert_eq!(s.theme, "dark");
        assert_eq!(s.clipboard_max_items, 100);
        std::fs::write(&p, b"not json").unwrap();
        assert_eq!(Settings::load(&p), Settings::default());
    }

    #[test]
    fn recent_targets_mru() {
        let mut s = Settings::default();
        for t in ["png", "jpg", "webp", "png", "a", "b", "c", "d", "e"] {
            s.touch_recent_target(t);
        }
        assert_eq!(s.recent_targets, ["e", "d", "c", "b", "a", "png", "webp"]);
    }
}

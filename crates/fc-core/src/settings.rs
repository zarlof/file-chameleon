//! Persisted settings (`settings.json` in the app's config folder,
//! `%APPDATA%\app.filechameleon.desktop`). Missing, unknown or invalid fields fall back to
//! their defaults one by one, so old or hand-edited files keep loading.

use serde::{Deserialize, Serialize};
use serde_json::Value;
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
    /// Closing the main window keeps the app in the tray (false = quit).
    pub close_to_tray: bool,
    /// Explorer right-click verb "Convert with File Chameleon" (HKCU registry, opens the wheel).
    pub context_menu: bool,
    /// Main window zoom in percent, 80..=150.
    pub ui_scale: u16,
    pub reduce_motion: bool,

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
    /// Most recently used target format ids, newest first, max 7 (Convert view ordering).
    pub recent_targets: Vec<String>,
    /// While the wheel is open, the mouse wheel flips front <-> back.
    pub wheel_flip_scroll: bool,
    /// While the wheel is open, Tab flips front <-> back.
    pub wheel_flip_tab: bool,
    /// Ctrl+Shift+drag opens the back face.
    pub wheel_shift_back: bool,
    /// Lowercase exe names ("photoshop.exe") where Ctrl+drag never opens the wheel.
    pub wheel_excluded_apps: Vec<String>,
    /// "auto" | "iconText" | "text" | "icon"
    pub wheel_labels: String,
    /// Wheel opacity in percent, 40..=100.
    pub wheel_opacity: u8,
    /// Hovered-slice colour: "accent" (app accent) or an accent name ("purple", ...).
    pub wheel_color: String,
    /// Wheel size in percent, 80..=130.
    pub wheel_scale: u16,
    /// Show the detail line under slice labels ("70%", "16:9").
    pub wheel_show_detail: bool,

    /// Global hotkeys ("Ctrl+Alt+V" style; "" = off).
    pub hotkey_shelf: String,
    pub hotkey_library: String,
    pub hotkey_download_link: String,
    /// Opens the clothesline.
    pub hotkey_line: String,
    /// Opens the clothesline with the search field focused.
    pub hotkey_line_search: String,
    /// Captures the monitor under the cursor into the clipboard history.
    pub hotkey_capture: String,
    /// Copies the newest image clip back to the clipboard.
    pub hotkey_copy_latest_image: String,

    /// The clothesline: a rope of recent clips that peeks in from a screen edge.
    pub line_enabled: bool,
    /// Moving the pointer to the top edge reveals it.
    pub line_edge_reveal: bool,
    /// It peeks in briefly after a copy.
    pub line_peek_on_copy: bool,
    /// The clips sway gently (off with reduce motion).
    pub line_breeze: bool,
    pub line_ropes: LineRopes,
    /// Rope colour: "accent", an accent name, or a `#rrggbb` hex colour (anything else is accent).
    pub line_color: String,
    /// Rope thickness in px, 1..=6.
    pub line_thickness: u8,
    /// Rope opacity in percent, 30..=100.
    pub line_opacity: u8,
    /// "small" | "medium" | "large" (anything else is medium).
    pub line_size: String,
    /// Hovering an image card for a moment shows an enlarged preview of it.
    pub line_zoom_on_hover: bool,
    /// A soft sound when clips hang or fall.
    pub line_sound: bool,

    /// Watch folders: new files are run through a preset automatically.
    pub watch_folders: Vec<WatchFolder>,

    pub clipboard_enabled: bool,
    /// Capture switches per kind.
    pub clipboard_capture_text: bool,
    pub clipboard_capture_links: bool,
    pub clipboard_images: bool,
    pub clipboard_suggest_links: bool,
    pub clipboard_suggest_images: bool,
    /// Cap on unsaved items (saved = pinned, foldered, tagged, titled or noted).
    pub clipboard_max_items: u32,
    /// Retention of unsaved items per kind, in days; 0 = forever.
    pub clipboard_retention_text: u32,
    pub clipboard_retention_links: u32,
    pub clipboard_retention_images: u32,
    /// Lowercase exe names whose copies are never recorded.
    pub clipboard_ignore_apps: Vec<String>,
    /// Recognize text in images with the built-in Windows OCR (offline).
    pub clipboard_ocr: bool,
    /// Fetch page titles for copied links (network; off by default).
    pub clipboard_link_titles: bool,
    /// Up to 3 folders whose new images are imported into the clipboard (e.g. Screenshots).
    pub clipboard_watch_folders: Vec<PathBuf>,
    /// Shelf rows in order: "images" | "texts" | "links".
    pub shelf_rows: Vec<String>,
    /// "small" | "medium" | "large"
    pub shelf_card_size: String,
    /// true = a click copies and closes; false = a click selects ("select items" mode).
    pub shelf_click_copies: bool,
    /// Capture paused from the tray or the Clipboard view (kept across restarts). Changed only
    /// through `clip_set_paused` / the tray; `set_settings` keeps the stored value.
    pub clipboard_paused: bool,

    /// Set after the first-run welcome was shown.
    pub onboarded: bool,
}

/// Which kinds of clips hang on the clothesline's ropes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LineRopes {
    pub images: bool,
    pub text: bool,
    pub links: bool,
}

impl Default for LineRopes {
    fn default() -> Self {
        Self {
            images: true,
            text: true,
            links: true,
        }
    }
}

/// One watched folder (Settings > Watch folders).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct WatchFolder {
    pub id: String,
    pub path: PathBuf,
    /// Preset id from the wheel config.
    pub preset: String,
    pub subfolders: bool,
    pub enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: "system".into(),
            theme: "system".into(),
            accent: "blue".into(),
            launch_at_login: false,
            close_to_tray: true,
            context_menu: false,
            ui_scale: 100,
            reduce_motion: false,
            output_dir: None,
            image_quality: 85,
            download_dir: None,
            download_mode: Default::default(),
            download_quality: Default::default(),
            cookies_browser: None,
            wheel_enabled: true,
            wheel_any_app: false,
            recent_targets: Vec::new(),
            wheel_flip_scroll: true,
            wheel_flip_tab: true,
            wheel_shift_back: true,
            wheel_excluded_apps: Vec::new(),
            wheel_labels: "text".into(),
            wheel_opacity: 100,
            wheel_color: "purple".into(),
            wheel_scale: 100,
            wheel_show_detail: true,
            hotkey_shelf: "Ctrl+Alt+V".into(),
            hotkey_library: String::new(),
            hotkey_download_link: "Ctrl+Alt+D".into(),
            hotkey_line: "Ctrl+Alt+L".into(),
            hotkey_line_search: "Ctrl+Alt+F".into(),
            hotkey_capture: String::new(),
            hotkey_copy_latest_image: String::new(),
            line_enabled: true,
            line_edge_reveal: true,
            line_peek_on_copy: true,
            line_breeze: true,
            line_ropes: LineRopes::default(),
            line_color: "accent".into(),
            line_thickness: 2,
            line_opacity: 100,
            line_size: "medium".into(),
            line_zoom_on_hover: false,
            line_sound: false,
            watch_folders: Vec::new(),
            clipboard_enabled: true,
            clipboard_capture_text: true,
            clipboard_capture_links: true,
            clipboard_images: true,
            clipboard_suggest_links: true,
            clipboard_suggest_images: true,
            clipboard_max_items: 500,
            clipboard_retention_text: 30,
            clipboard_retention_links: 30,
            clipboard_retention_images: 7,
            clipboard_ignore_apps: Vec::new(),
            clipboard_ocr: true,
            clipboard_link_titles: false,
            clipboard_watch_folders: Vec::new(),
            shelf_rows: vec!["images".into(), "texts".into(), "links".into()],
            shelf_card_size: "medium".into(),
            shelf_click_copies: true,
            clipboard_paused: false,
            onboarded: false,
        }
    }
}

impl Settings {
    pub const MAX_RECENT_TARGETS: usize = 7;

    /// Missing or corrupt file -> defaults (never fails). A field with a bad value (an unknown
    /// enum variant, a wrong type) falls back to its default; the other fields are kept.
    pub fn load(path: &Path) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .map(Self::from_value)
            .unwrap_or_default()
            .clamped()
    }

    /// Pull out-of-range wave-2 line values back into their ranges (on load and on save).
    pub fn clamped(mut self) -> Self {
        self.line_thickness = self.line_thickness.clamp(1, 6);
        self.line_opacity = self.line_opacity.clamp(30, 100);
        self
    }

    /// Merge the fields of `v` onto the defaults, skipping any field that does not deserialize.
    fn from_value(v: Value) -> Self {
        let Value::Object(mut file) = v else {
            return Self::default();
        };
        // v1 -> v2: one retention value became three. Carry the old value over so an upgrade
        // never prunes history under the (shorter) v2 defaults.
        const NEW_KEYS: [&str; 3] = [
            "clipboardRetentionText",
            "clipboardRetentionLinks",
            "clipboardRetentionImages",
        ];
        if let Some(old) = file.get("clipboardRetentionDays").cloned()
            && NEW_KEYS.iter().all(|k| !file.contains_key(*k))
        {
            for key in NEW_KEYS {
                file.insert(key.to_string(), old.clone());
            }
        }
        if let Ok(s) = serde_json::from_value(Value::Object(file.clone())) {
            return s;
        }
        let Ok(Value::Object(mut merged)) = serde_json::to_value(Self::default()) else {
            return Self::default();
        };
        for (key, value) in file {
            // Unknown keys are ignored, as in the normal path.
            let Some(previous) = merged.insert(key.clone(), value) else {
                merged.remove(&key);
                continue;
            };
            if serde_json::from_value::<Self>(Value::Object(merged.clone())).is_err() {
                merged.insert(key, previous);
            }
        }
        serde_json::from_value(Value::Object(merged)).unwrap_or_default()
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
        let s = Settings {
            language: "tr".into(),
            ..Settings::default()
        };
        s.save(&p).unwrap();
        assert_eq!(Settings::load(&p).language, "tr");
        std::fs::write(&p, br#"{"theme":"dark"}"#).unwrap();
        let s = Settings::load(&p);
        assert_eq!(s.theme, "dark");
        assert_eq!(s.clipboard_max_items, 500);
        assert_eq!(s.hotkey_shelf, "Ctrl+Alt+V");
        std::fs::write(&p, b"not json").unwrap();
        assert_eq!(Settings::load(&p), Settings::default());
    }

    #[test]
    fn one_bad_value_keeps_the_other_fields() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.json");
        std::fs::write(
            &p,
            br#"{"theme":"dark","downloadMode":"banana","clipboardMaxItems":42,
                "imageQuality":"high","clipboardPaused":true,"futureField":1}"#,
        )
        .unwrap();
        let s = Settings::load(&p);
        assert_eq!(s.theme, "dark");
        assert_eq!(s.clipboard_max_items, 42);
        assert!(s.clipboard_paused);
        // The invalid values fall back to their defaults.
        assert_eq!(s.download_mode, crate::download::DlMode::default());
        assert_eq!(s.image_quality, Settings::default().image_quality);
        // A non-object file is just defaults.
        std::fs::write(&p, b"[1, 2]").unwrap();
        assert_eq!(Settings::load(&p), Settings::default());
    }

    #[test]
    fn v1_retention_days_migrates_to_all_three_kinds() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.json");
        std::fs::write(&p, br#"{"theme":"dark","clipboardRetentionDays":90}"#).unwrap();
        let s = Settings::load(&p);
        assert_eq!(s.theme, "dark");
        assert_eq!(s.clipboard_retention_text, 90);
        assert_eq!(s.clipboard_retention_links, 90);
        assert_eq!(s.clipboard_retention_images, 90);
        // A v2 file keeps its own values.
        std::fs::write(
            &p,
            br#"{"clipboardRetentionDays":90,"clipboardRetentionText":5}"#,
        )
        .unwrap();
        let s = Settings::load(&p);
        assert_eq!(s.clipboard_retention_text, 5);
        assert_eq!(s.clipboard_retention_links, 30);
        assert_eq!(s.clipboard_retention_images, 7);
    }

    #[test]
    fn old_settings_get_line_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.json");
        std::fs::write(&p, br#"{"theme":"dark","hotkeyShelf":"Ctrl+Alt+V"}"#).unwrap();
        let s = Settings::load(&p);
        assert!(s.line_enabled && s.line_edge_reveal && s.line_peek_on_copy && s.line_breeze);
        assert!(s.line_ropes.images && s.line_ropes.text && s.line_ropes.links);
        assert_eq!(s.hotkey_line, "Ctrl+Alt+L");
        // A partial rope object keeps the other ropes on.
        std::fs::write(&p, br#"{"lineRopes":{"text":false}}"#).unwrap();
        let s = Settings::load(&p);
        assert!(!s.line_ropes.text && s.line_ropes.images && s.line_ropes.links);
    }

    #[test]
    fn old_settings_get_wave2_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.json");
        std::fs::write(&p, br#"{"theme":"dark"}"#).unwrap();
        let s = Settings::load(&p);
        assert_eq!(s.line_color, "accent");
        assert_eq!((s.line_thickness, s.line_opacity), (2, 100));
        assert_eq!(s.line_size, "medium");
        assert!(!s.line_zoom_on_hover && !s.line_sound);
        assert_eq!(s.hotkey_line_search, "Ctrl+Alt+F");
        assert_eq!(s.hotkey_capture, "");
        assert_eq!(s.hotkey_copy_latest_image, "");
        std::fs::write(&p, br#"{"lineThickness":99,"lineOpacity":5}"#).unwrap();
        let s = Settings::load(&p);
        assert_eq!((s.line_thickness, s.line_opacity), (6, 30));
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

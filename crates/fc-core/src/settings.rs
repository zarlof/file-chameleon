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
    /// Rope colour: "natural" (hemp, the default), "accent", an accent name, or a `#rrggbb` hex
    /// colour (anything else is natural).
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
    /// "twisted" | "cord" | "thin"
    pub line_rope_style: String,
    /// Rope sag in px, 0..=40.
    pub line_sag: u8,
    /// "plastic" | "wood" | "metal"
    pub line_pin_style: String,
    /// "auto" or `#rrggbb`.
    pub line_pin_color: String,
    /// "glassDark" | "glassLight" | "paper"
    pub line_card_style: String,
    /// Card corner radius in px, 0..=16.
    pub line_card_radius: u8,
    pub line_card_shadow: bool,
    pub line_show_labels: bool,
    pub line_show_numbers: bool,
    /// Rope order, top to bottom: exactly "images", "text", "links", "files" (sanitized).
    pub line_rope_order: Vec<String>,
    /// Custom rope names; "" = the default name.
    pub line_rope_names: LineRopeNames,
    /// The shortcuts rope's apps, in the user's order.
    pub line_shortcuts: Vec<LineShortcut>,
    /// Which files-rope sources are shown.
    pub line_files_sources: LineFilesSources,
    /// Cards per rope: 0 = all that fit, else 3..=30.
    pub line_max_per_rope: u8,
    /// "top" | "bottom"
    pub line_edge: String,
    /// How long the line stays after a copy, in ms, 1000..=6000.
    pub line_peek_ms: u32,
    /// Pointer dwell time at the edge before it reveals, in ms, 150..=1000.
    pub line_reveal_dwell_ms: u32,
    /// Delay before it hides after the pointer leaves, in ms, 200..=2000.
    pub line_hide_delay_ms: u32,
    /// "copy" | "copyClose" | "paste"
    pub line_click_action: String,
    /// Sway strength in percent, 0..=200.
    pub line_swing: u16,
    /// Sway damping in percent, 50..=200.
    pub line_damping: u16,
    /// Breeze strength in percent, 0..=200.
    pub line_breeze_strength: u16,
    /// Removed clips fall off the rope.
    pub line_fall: bool,
    /// Sound volume in percent, 0..=100.
    pub line_sound_volume: u8,
    /// Settings migration marker: below 3 the wave-3 look migration still has to run.
    pub line_look_version: u8,

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
    pub files: bool,
    /// App shortcuts the user picked (`line_shortcuts`).
    pub shortcuts: bool,
}

impl Default for LineRopes {
    fn default() -> Self {
        Self {
            images: true,
            text: true,
            links: true,
            files: true,
            shortcuts: true,
        }
    }
}

/// One app shortcut on the shortcuts rope: a display name and what to start (an .exe, .lnk,
/// .url or any file or folder).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct LineShortcut {
    pub name: String,
    pub path: String,
}

/// At most this many shortcuts hang on the rope.
pub const MAX_SHORTCUTS: usize = 40;

/// Custom names of the ropes ("" = default).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct LineRopeNames {
    pub images: String,
    pub text: String,
    pub links: String,
    pub files: String,
    pub shortcuts: String,
}

/// Which sources the files rope shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LineFilesSources {
    pub added: bool,
    pub converted: bool,
    pub downloaded: bool,
}

impl Default for LineFilesSources {
    fn default() -> Self {
        Self {
            added: true,
            converted: true,
            downloaded: true,
        }
    }
}

/// The rope keys in their default order.
pub const ROPE_KEYS: [&str; 5] = ["images", "text", "links", "files", "shortcuts"];

fn sanitize_rope_order(order: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for k in order {
        if ROPE_KEYS.contains(&k.as_str()) && !out.contains(k) {
            out.push(k.clone());
        }
    }
    for k in ROPE_KEYS {
        if !out.iter().any(|o| o == k) {
            out.push(k.to_string());
        }
    }
    out
}

/// Shortcuts made safe: no empty paths, no repeats (paths compare case-insensitively), names
/// trimmed to 60 characters (an empty name becomes the file name), at most [`MAX_SHORTCUTS`].
fn sanitize_shortcuts(list: Vec<LineShortcut>) -> Vec<LineShortcut> {
    let mut seen: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for mut s in list {
        s.path = s.path.trim().to_string();
        let key = s.path.to_lowercase().replace('/', "\\");
        if s.path.is_empty() || seen.contains(&key) {
            continue;
        }
        seen.push(key);
        let name = s.name.trim();
        s.name = if name.is_empty() {
            std::path::Path::new(&s.path)
                .file_stem()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| s.path.clone())
        } else {
            name.to_string()
        };
        if s.name.chars().count() > 60 {
            s.name = s.name.chars().take(60).collect();
        }
        out.push(s);
        if out.len() == MAX_SHORTCUTS {
            break;
        }
    }
    out
}

fn is_hex_colour(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
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
            hotkey_shelf: String::new(),
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
            line_color: "#ffffff".into(),
            line_thickness: 2,
            line_opacity: 100,
            line_size: "medium".into(),
            line_zoom_on_hover: false,
            line_sound: false,
            line_rope_style: "web".into(),
            line_sag: 14,
            line_pin_style: "spider".into(),
            line_pin_color: "auto".into(),
            line_card_style: "glassDark".into(),
            line_card_radius: 10,
            line_card_shadow: true,
            line_show_labels: true,
            line_show_numbers: true,
            line_rope_order: ROPE_KEYS.iter().map(|k| k.to_string()).collect(),
            line_rope_names: LineRopeNames::default(),
            line_shortcuts: Vec::new(),
            line_files_sources: LineFilesSources::default(),
            line_max_per_rope: 0,
            line_edge: "top".into(),
            line_peek_ms: 2500,
            line_reveal_dwell_ms: 350,
            line_hide_delay_ms: 400,
            line_click_action: "open".into(),
            line_swing: 100,
            line_damping: 100,
            line_breeze_strength: 100,
            line_fall: true,
            line_sound_volume: 60,
            line_look_version: Self::LOOK_VERSION,
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
    /// Current `line_look_version`: 3 = files rope, natural rope colour, shelf folded into the
    /// line; 4 = white spider-web rope, spiders instead of pegs, a click opens the card.
    pub const LOOK_VERSION: u8 = 4;

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

    /// Pull out-of-range line values back into their ranges and enums back into their sets
    /// (on load and on save).
    pub fn clamped(mut self) -> Self {
        self.line_thickness = self.line_thickness.clamp(1, 6);
        self.line_opacity = self.line_opacity.clamp(30, 100);
        let pick = |v: &mut String, allowed: &[&str], default: &str| {
            if !allowed.contains(&v.as_str()) {
                *v = default.to_string();
            }
        };
        pick(
            &mut self.line_rope_style,
            &["web", "twisted", "cord", "thin"],
            "web",
        );
        pick(
            &mut self.line_pin_style,
            &["spider", "plastic", "wood", "metal"],
            "spider",
        );
        pick(
            &mut self.line_card_style,
            &["glassDark", "glassLight", "paper"],
            "glassDark",
        );
        pick(&mut self.line_edge, &["top", "bottom"], "top");
        pick(
            &mut self.line_click_action,
            &["open", "copy", "copyClose", "paste"],
            "open",
        );
        if self.line_pin_color != "auto" && !is_hex_colour(&self.line_pin_color) {
            self.line_pin_color = "auto".into();
        }
        self.line_sag = self.line_sag.clamp(0, 40);
        self.line_card_radius = self.line_card_radius.clamp(0, 16);
        self.line_max_per_rope = match self.line_max_per_rope {
            0 => 0,
            n => n.clamp(3, 30),
        };
        self.line_peek_ms = self.line_peek_ms.clamp(1000, 6000);
        self.line_reveal_dwell_ms = self.line_reveal_dwell_ms.clamp(150, 1000);
        self.line_hide_delay_ms = self.line_hide_delay_ms.clamp(200, 2000);
        self.line_swing = self.line_swing.min(200);
        self.line_damping = self.line_damping.clamp(50, 200);
        self.line_breeze_strength = self.line_breeze_strength.min(200);
        self.line_sound_volume = self.line_sound_volume.min(100);
        const ACCENTS: [&str; 8] = [
            "blue", "purple", "pink", "red", "orange", "yellow", "green", "graphite",
        ];
        if !(ACCENTS.contains(&self.line_color.as_str())
            || ["accent", "natural"].contains(&self.line_color.as_str())
            || is_hex_colour(&self.line_color))
        {
            self.line_color = "natural".into();
        }
        for n in [
            &mut self.line_rope_names.images,
            &mut self.line_rope_names.text,
            &mut self.line_rope_names.links,
            &mut self.line_rope_names.files,
            &mut self.line_rope_names.shortcuts,
        ] {
            if n.chars().count() > 24 {
                *n = n.chars().take(24).collect();
            }
        }
        self.line_rope_order = sanitize_rope_order(&self.line_rope_order);
        self.line_shortcuts = sanitize_shortcuts(std::mem::take(&mut self.line_shortcuts));
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
        // Wave 3: the shelf folded into the line. Looks only at keys the file really has.
        let version = file
            .get("lineLookVersion")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if version < 4 {
            // Wave 4: the user asked for a white spider-web rope held by spiders, and for a
            // click that opens the card.
            file.insert("lineColor".into(), "#ffffff".into());
            file.insert("lineRopeStyle".into(), "web".into());
            file.insert("linePinStyle".into(), "spider".into());
            file.insert("lineClickAction".into(), "open".into());
        }
        if version < 3 {
            if file.get("lineColor").and_then(Value::as_str) == Some("accent") {
                file.insert("lineColor".into(), "natural".into());
            }
            let shelf = file
                .get("hotkeyShelf")
                .and_then(Value::as_str)
                .filter(|h| !h.is_empty())
                .map(str::to_string);
            let line_is_default = file
                .get("hotkeyLine")
                .and_then(Value::as_str)
                .is_none_or(|h| h == "Ctrl+Alt+L");
            if let Some(shelf) = shelf
                && line_is_default
            {
                file.insert("hotkeyLine".into(), shelf.into());
            }
            file.insert("hotkeyShelf".into(), "".into());
        }
        if version < u64::from(Self::LOOK_VERSION) {
            file.insert("lineLookVersion".into(), Self::LOOK_VERSION.into());
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
        assert_eq!(s.hotkey_shelf, "");
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
        // Wave 3 moves the shelf hotkey onto the (default) line hotkey.
        assert_eq!(s.hotkey_line, "Ctrl+Alt+V");
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
        assert_eq!(s.line_color, "#ffffff");
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
    fn wave3_defaults_and_clamps() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.json");
        std::fs::write(&p, br#"{"theme":"dark","lineLookVersion":3}"#).unwrap();
        let s = Settings::load(&p);
        // Wave 4 replaces the look of a version 3 file.
        assert_eq!(s.line_rope_style, "web");
        assert_eq!((s.line_sag, s.line_card_radius), (14, 10));
        assert_eq!(s.line_pin_style, "spider");
        assert_eq!(s.line_pin_color, "auto");
        assert_eq!(s.line_card_style, "glassDark");
        assert!(s.line_card_shadow && s.line_show_labels && s.line_show_numbers && s.line_fall);
        assert_eq!(
            s.line_rope_order,
            ["images", "text", "links", "files", "shortcuts"]
        );
        assert_eq!(s.line_rope_names, LineRopeNames::default());
        assert!(s.line_ropes.files);
        assert!(s.line_files_sources.added && s.line_files_sources.converted);
        assert!(s.line_files_sources.downloaded);
        assert_eq!(s.line_max_per_rope, 0);
        assert_eq!(s.line_edge, "top");
        assert_eq!(
            (s.line_peek_ms, s.line_reveal_dwell_ms, s.line_hide_delay_ms),
            (2500, 350, 400)
        );
        assert_eq!(s.line_click_action, "open");
        assert_eq!(
            (s.line_swing, s.line_damping, s.line_breeze_strength),
            (100, 100, 100)
        );
        assert_eq!(s.line_sound_volume, 60);
        std::fs::write(
            &p,
            br##"{"lineSag":99,"lineMaxPerRope":1,"linePeekMs":10,"lineRevealDwellMs":9999,
                "lineHideDelayMs":1,"lineSwing":999,"lineDamping":1,"lineCardRadius":99,
                "lineSoundVolume":250,"lineEdge":"left","linePinColor":"red",
                "lineRopeOrder":["files","files","text"],"lineLookVersion":3}"##,
        )
        .unwrap();
        let s = Settings::load(&p);
        assert_eq!(
            (s.line_sag, s.line_max_per_rope, s.line_peek_ms),
            (40, 3, 1000)
        );
        assert_eq!((s.line_reveal_dwell_ms, s.line_hide_delay_ms), (1000, 200));
        assert_eq!(
            (s.line_swing, s.line_damping, s.line_card_radius),
            (200, 50, 16)
        );
        assert_eq!(s.line_sound_volume, 100);
        assert_eq!(
            (s.line_edge.as_str(), s.line_pin_color.as_str()),
            ("top", "auto")
        );
        assert_eq!(
            s.line_rope_order,
            ["files", "text", "images", "links", "shortcuts"]
        );
        std::fs::write(
            &p,
            br#"{"lineLookVersion":4,"lineColor":"banana","lineRopeNames":{"text":"abcdefghijklmnopqrstuvwxyz123"}}"#,
        )
        .unwrap();
        let s = Settings::load(&p);
        assert_eq!(s.line_color, "natural");
        assert_eq!(s.line_rope_names.text.chars().count(), 24);
        std::fs::write(&p, br##"{"lineMaxPerRope":50,"linePinColor":"#aabbcc"}"##).unwrap();
        let s = Settings::load(&p);
        assert_eq!(
            (s.line_max_per_rope, s.line_pin_color.as_str()),
            (30, "#aabbcc")
        );
    }

    #[test]
    fn wave3_migration() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.json");
        std::fs::write(
            &p,
            br#"{"hotkeyShelf":"Ctrl+1","hotkeyLine":"Ctrl+Alt+L","lineColor":"accent"}"#,
        )
        .unwrap();
        let s = Settings::load(&p);
        assert_eq!(s.hotkey_line, "Ctrl+1");
        assert_eq!(s.hotkey_shelf, "");
        assert_eq!(s.line_color, "#ffffff");
        assert_eq!(s.line_look_version, 4);
        // A custom line hotkey is kept.
        std::fs::write(&p, br#"{"hotkeyShelf":"Ctrl+1","hotkeyLine":"Ctrl+Alt+K"}"#).unwrap();
        assert_eq!(Settings::load(&p).hotkey_line, "Ctrl+Alt+K");
        // After the migration the user's own choice survives later loads.
        let mut s = Settings::load(&p);
        s.line_color = "accent".into();
        s.save(&p).unwrap();
        let s = Settings::load(&p);
        assert_eq!((s.line_color.as_str(), s.line_look_version), ("accent", 4));
    }

    #[test]
    fn shortcuts_are_sanitized() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.json");
        std::fs::write(
            &p,
            br#"{"lineShortcuts":[{"name":"","path":"C:\\Apps\\Paint.lnk"},
                {"name":"Again","path":"c:/apps/paint.lnk"},{"name":"x","path":"  "},
                {"name":"  Notes  ","path":"C:\\n.exe"}]}"#,
        )
        .unwrap();
        let s = Settings::load(&p);
        assert!(s.line_ropes.shortcuts);
        let names: Vec<&str> = s.line_shortcuts.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["Paint", "Notes"]);
        let many: Vec<LineShortcut> = (0..60)
            .map(|i| LineShortcut {
                name: "a".repeat(80),
                path: format!("C:\\{i}.exe"),
            })
            .collect();
        let s = Settings {
            line_shortcuts: many,
            ..Settings::default()
        }
        .clamped();
        assert_eq!(s.line_shortcuts.len(), MAX_SHORTCUTS);
        assert_eq!(s.line_shortcuts[0].name.chars().count(), 60);
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

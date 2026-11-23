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
    /// Opens the main window on its home page (Convert).
    pub hotkey_home: String,
    /// Ctrl+Shift+1..9 paste the pinned clips, in their order on the line.
    pub pin_hotkeys: bool,

    /// The clothesline: a rope of recent clips that peeks in from a screen edge.
    pub line_enabled: bool,
    /// Moving the pointer to the top edge reveals it.
    pub line_edge_reveal: bool,
    /// It peeks in briefly after a copy.
    pub line_peek_on_copy: bool,
    /// The clips sway gently (off with reduce motion).
    pub line_breeze: bool,
    pub line_ropes: LineRopes,
    /// Rope colour: "natural" (hemp), "accent", an accent name, or a `#rrggbb` hex colour
    /// (default white `#ffffff`; anything else falls back to the default).
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
    /// Rope sag in px, 0..=40. (The rope is always spider silk and the cards hang from spiders:
    /// the other rope and clothespin styles were removed in 0.4.2; their keys are ignored.)
    pub line_sag: u8,
    /// "paper" (default) | "glassDark" | "glassLight"
    pub line_card_style: String,
    /// Background of every card: "" = the card style's own, or `#rrggbb` (a card's own colour,
    /// set on the line, wins).
    pub line_card_color: String,
    /// Card corner radius in px, 0..=16.
    pub line_card_radius: u8,
    pub line_card_shadow: bool,
    pub line_show_labels: bool,
    pub line_show_numbers: bool,
    /// Rope order, top to bottom: exactly the [`ROPE_KEYS`] (sanitized).
    pub line_rope_order: Vec<String>,
    /// Custom rope names; "" = the default name.
    pub line_rope_names: LineRopeNames,
    /// Ropes the user folded away (only their label shows), by key.
    pub line_folded: Vec<String>,
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
    /// "open" (default) | "copy" | "copyClose" | "paste"
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
    /// Settings migration marker: below [`Settings::LOOK_VERSION`] a look migration still has
    /// to run.
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
    /// Look for a new version on GitHub at start (Settings > About).
    pub update_check: bool,
}

/// Which kinds of clips hang on the clothesline's ropes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LineRopes {
    pub images: bool,
    pub text: bool,
    pub links: bool,
    pub files: bool,
}

impl Default for LineRopes {
    fn default() -> Self {
        Self {
            images: true,
            text: true,
            links: true,
            files: true,
        }
    }
}

/// Custom names of the ropes ("" = default).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct LineRopeNames {
    pub images: String,
    pub text: String,
    pub links: String,
    pub files: String,
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
pub const ROPE_KEYS: [&str; 4] = ["images", "text", "links", "files"];

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

fn is_hex_colour(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// A card colour as stored: `#rrggbb` in lower case, or `None` for anything else (no colour).
pub fn hex_colour(s: Option<&str>) -> Option<String> {
    s.map(str::trim)
        .filter(|s| is_hex_colour(s))
        .map(str::to_ascii_lowercase)
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
            theme: "light".into(),
            accent: "blue".into(),
            launch_at_login: true,
            close_to_tray: true,
            context_menu: true,
            ui_scale: 100,
            reduce_motion: false,
            output_dir: None,
            image_quality: 100,
            download_dir: None,
            download_mode: Default::default(),
            download_quality: Default::default(),
            cookies_browser: None,
            wheel_enabled: true,
            wheel_any_app: true,
            recent_targets: Vec::new(),
            wheel_flip_scroll: true,
            wheel_flip_tab: true,
            wheel_shift_back: true,
            wheel_excluded_apps: Vec::new(),
            wheel_labels: "iconText".into(),
            wheel_opacity: 100,
            wheel_color: "blue".into(),
            wheel_scale: 100,
            wheel_show_detail: true,
            hotkey_shelf: String::new(),
            hotkey_library: "Ctrl+2".into(),
            hotkey_download_link: "Ctrl+7".into(),
            hotkey_line: "Ctrl+1".into(),
            hotkey_line_search: "Ctrl+Alt+F".into(),
            hotkey_capture: "Ctrl+3".into(),
            hotkey_copy_latest_image: String::new(),
            hotkey_home: "Ctrl+4".into(),
            pin_hotkeys: false,
            line_enabled: true,
            line_edge_reveal: true,
            line_peek_on_copy: false,
            line_breeze: true,
            line_ropes: LineRopes::default(),
            line_color: "#ffffff".into(),
            line_thickness: 1,
            line_opacity: 100,
            line_size: "large".into(),
            line_zoom_on_hover: true,
            line_sound: true,
            line_sag: 25,
            line_card_style: "paper".into(),
            line_card_color: String::new(),
            line_card_radius: 10,
            line_card_shadow: true,
            line_show_labels: true,
            line_show_numbers: true,
            line_rope_order: ROPE_KEYS.iter().map(|k| k.to_string()).collect(),
            line_rope_names: LineRopeNames::default(),
            line_folded: Vec::new(),
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
            clipboard_max_items: 200,
            clipboard_retention_text: 30,
            clipboard_retention_links: 30,
            clipboard_retention_images: 30,
            clipboard_ignore_apps: Vec::new(),
            clipboard_ocr: true,
            clipboard_link_titles: false,
            clipboard_watch_folders: Vec::new(),
            shelf_rows: vec!["images".into(), "texts".into(), "links".into()],
            shelf_card_size: "medium".into(),
            shelf_click_copies: true,
            clipboard_paused: false,
            onboarded: false,
            update_check: true,
        }
    }
}

impl Settings {
    pub const MAX_RECENT_TARGETS: usize = 7;
    /// Current `line_look_version`: 3 = files rope, natural rope colour, shelf folded into the
    /// line; 4 = white spider-web rope, spiders instead of pegs, a click opens the card;
    /// 5 = the line no longer peeks on every copy (it opens only when asked).
    pub const LOOK_VERSION: u8 = 5;

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
    /// (on load and on save). An unknown enum value falls back to the default.
    pub fn clamped(mut self) -> Self {
        let d = Self::default();
        self.line_thickness = self.line_thickness.clamp(1, 6);
        self.line_opacity = self.line_opacity.clamp(30, 100);
        let pick = |v: &mut String, allowed: &[&str], default: &str| {
            if !allowed.contains(&v.as_str()) {
                *v = default.to_string();
            }
        };
        pick(
            &mut self.line_card_style,
            &["glassDark", "glassLight", "paper"],
            &d.line_card_style,
        );
        pick(&mut self.line_edge, &["top", "bottom"], &d.line_edge);
        pick(
            &mut self.line_click_action,
            &["open", "copy", "copyClose", "paste"],
            &d.line_click_action,
        );
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
            self.line_color = d.line_color;
        }
        for n in [
            &mut self.line_rope_names.images,
            &mut self.line_rope_names.text,
            &mut self.line_rope_names.links,
            &mut self.line_rope_names.files,
        ] {
            if n.chars().count() > 24 {
                *n = n.chars().take(24).collect();
            }
        }
        self.line_rope_order = sanitize_rope_order(&self.line_rope_order);
        self.line_card_color = hex_colour(Some(&self.line_card_color)).unwrap_or_default();
        let mut folded: Vec<String> = Vec::new();
        for k in std::mem::take(&mut self.line_folded) {
            if ROPE_KEYS.contains(&k.as_str()) && !folded.contains(&k) {
                folded.push(k);
            }
        }
        self.line_folded = folded;
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
        if version < 5 {
            // Wave 5: the user asked that copying never pops the line up.
            file.insert("linePeekOnCopy".into(), false.into());
        }
        if version < 4 {
            // Wave 4: the user asked for a white spider-web rope held by spiders, and for a
            // click that opens the card.
            file.insert("lineColor".into(), "#ffffff".into());
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

    /// The frontend's defaults (`src/lib/stores/default-settings.json`) are generated from
    /// `Settings::default()`; this keeps them in sync. After changing a default run
    /// `UPDATE_DEFAULT_SETTINGS=1 cargo test -p fc-core frontend_defaults` to rewrite the file.
    #[test]
    fn frontend_defaults_match() {
        let file = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../src/lib/stores/default-settings.json");
        let ours = serde_json::to_value(Settings::default()).unwrap();
        if std::env::var_os("UPDATE_DEFAULT_SETTINGS").is_some() {
            let mut text = serde_json::to_string_pretty(&ours).unwrap();
            text.push('\n');
            std::fs::write(&file, text).unwrap();
        }
        let theirs: Value =
            serde_json::from_slice(&std::fs::read(&file).expect("default-settings.json")).unwrap();
        assert_eq!(
            ours, theirs,
            "default-settings.json is stale: rerun with UPDATE_DEFAULT_SETTINGS=1"
        );
    }

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
        assert_eq!(s.clipboard_max_items, 200);
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
        assert_eq!(s.clipboard_retention_images, 30);
    }

    #[test]
    fn old_settings_get_line_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.json");
        std::fs::write(&p, br#"{"theme":"dark","hotkeyShelf":"Ctrl+Alt+V"}"#).unwrap();
        let s = Settings::load(&p);
        assert!(s.line_enabled && s.line_edge_reveal && !s.line_peek_on_copy && s.line_breeze);
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
        // Missing fields take the defaults (compared to `Settings::default()`, so changing a
        // default does not break this test).
        let d = Settings::default();
        assert_eq!(s.theme, "dark");
        assert_eq!(s.line_color, d.line_color);
        assert_eq!(
            (s.line_thickness, s.line_opacity),
            (d.line_thickness, d.line_opacity)
        );
        assert_eq!(s.line_size, d.line_size);
        assert_eq!(
            (s.line_zoom_on_hover, s.line_sound),
            (d.line_zoom_on_hover, d.line_sound)
        );
        assert_eq!(s.hotkey_line_search, d.hotkey_line_search);
        assert_eq!(s.hotkey_capture, d.hotkey_capture);
        assert_eq!(s.hotkey_copy_latest_image, d.hotkey_copy_latest_image);
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
        assert_eq!(s.line_color, "#ffffff");
        let d = Settings::default();
        assert_eq!(
            (s.line_sag, s.line_card_radius),
            (d.line_sag, d.line_card_radius)
        );
        assert_eq!(s.line_card_style, "paper");
        assert!(s.line_card_shadow && s.line_show_labels && s.line_show_numbers && s.line_fall);
        assert_eq!(s.line_rope_order, ["images", "text", "links", "files"]);
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
                "lineSoundVolume":250,"lineEdge":"left","linePinStyle":"wood",
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
        assert_eq!(s.line_edge, "top");
        assert_eq!(s.line_rope_order, ["files", "text", "images", "links"]);
        std::fs::write(
            &p,
            br#"{"lineLookVersion":4,"lineColor":"banana","lineRopeNames":{"text":"abcdefghijklmnopqrstuvwxyz123"}}"#,
        )
        .unwrap();
        let s = Settings::load(&p);
        assert_eq!(s.line_color, Settings::default().line_color);
        assert_eq!(s.line_rope_names.text.chars().count(), 24);
        std::fs::write(&p, br##"{"lineMaxPerRope":50,"lineRopeStyle":"thin"}"##).unwrap();
        let s = Settings::load(&p);
        assert_eq!(s.line_max_per_rope, 30);
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
        assert_eq!(s.line_look_version, 5);
        // A custom line hotkey is kept.
        std::fs::write(&p, br#"{"hotkeyShelf":"Ctrl+1","hotkeyLine":"Ctrl+Alt+K"}"#).unwrap();
        assert_eq!(Settings::load(&p).hotkey_line, "Ctrl+Alt+K");
        // After the migration the user's own choice survives later loads.
        let mut s = Settings::load(&p);
        s.line_color = "accent".into();
        s.save(&p).unwrap();
        let s = Settings::load(&p);
        assert_eq!((s.line_color.as_str(), s.line_look_version), ("accent", 5));
    }

    #[test]
    fn wave5_stops_peeking_on_copy_once() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.json");
        std::fs::write(&p, br#"{"lineLookVersion":4,"linePeekOnCopy":true}"#).unwrap();
        let s = Settings::load(&p);
        assert!(!s.line_peek_on_copy);
        assert_eq!(s.line_look_version, 5);
        // Turned back on by the user after the migration: kept.
        std::fs::write(&p, br#"{"lineLookVersion":5,"linePeekOnCopy":true}"#).unwrap();
        assert!(Settings::load(&p).line_peek_on_copy);
    }

    #[test]
    fn old_shortcuts_rope_and_card_colour() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.json");
        // The shortcuts rope is gone: its keys in an old file are ignored, the rest loads.
        std::fs::write(
            &p,
            br##"{"theme":"dark","lineShortcuts":[{"name":"x","path":"C:/x.exe"}],
                "lineRopes":{"images":false,"shortcuts":true},
                "lineRopeOrder":["shortcuts","files"],"lineFolded":["shortcuts","text"],
                "lineCardColor":"#AABBCC"}"##,
        )
        .unwrap();
        let s = Settings::load(&p);
        assert_eq!(s.theme, "dark");
        assert!(!s.line_ropes.images && s.line_ropes.files);
        assert_eq!(s.line_rope_order, ["files", "images", "text", "links"]);
        assert_eq!(s.line_folded, ["text"]);
        assert_eq!(s.line_card_color, "#aabbcc");
        let s = Settings {
            line_card_color: "red".into(),
            ..Settings::default()
        }
        .clamped();
        assert_eq!(s.line_card_color, "");
        let s = Settings {
            line_folded: vec![
                "links".into(),
                "nope".into(),
                "links".into(),
                "files".into(),
            ],
            ..Settings::default()
        }
        .clamped();
        assert_eq!(s.line_folded, ["links", "files"]);
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

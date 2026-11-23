//! Presets & wheel IPC (Task 6): the wheel config commands, running a preset on files
//! (`run_preset`, shared by the wheel, the Convert view, the install toast and watch folders),
//! and the `overlay://wheel-slots` payload.

use super::{CmdError, CmdResult, submit_with};
use crate::events::{NavPayload, WheelSlots, WheelStyle};
use crate::state::AppState;
use crate::windows;
use fc_core::formats;
use fc_core::jobs::{JobId, JobRequest, Operation};
use fc_core::presets::{self, Face, Preset, Profile, WheelConfig, WheelSlot};
use fc_core::settings::Settings;
use serde_json::json;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, State};

// ---------------------------------------------------------------- wheel config commands

/// Async (as the other config commands): the first call reads `wheel.json`.
#[tauri::command(async)]
pub fn get_wheel_config(state: State<'_, AppState>) -> WheelConfig {
    state.wheel_config().clone()
}

/// Validates (limits, ids, references, read-only built-ins) and saves; returns what was
/// stored (normalized). Error code "invalid". A failed save keeps the old config.
#[tauri::command(async)]
pub fn set_wheel_config(state: State<'_, AppState>, config: WheelConfig) -> CmdResult<WheelConfig> {
    let config = config.validate()?;
    let saved = state.update_wheel_config(|c| {
        *c = config;
        Ok(())
    })?;
    crate::watchfolders::reconfigure(&state.app);
    Ok(saved)
}

/// Built-in profile back to its default arrangement. Custom or unknown id: "invalid".
#[tauri::command(async)]
pub fn reset_wheel_profile(state: State<'_, AppState>, id: String) -> CmdResult<WheelConfig> {
    let saved = state.update_wheel_config(|c| {
        if c.reset_profile(&id) {
            Ok(())
        } else {
            Err(fc_core::Error::Invalid(format!(
                "not a built-in profile: {id}"
            )))
        }
    })?;
    crate::watchfolders::reconfigure(&state.app);
    Ok(saved)
}

/// Everything back to the built-ins (user presets, groups and profiles are removed).
#[tauri::command(async)]
pub fn reset_wheel_config(state: State<'_, AppState>) -> CmdResult<WheelConfig> {
    let saved = state.update_wheel_config(|c| {
        *c = WheelConfig::builtin();
        Ok(())
    })?;
    crate::watchfolders::reconfigure(&state.app);
    Ok(saved)
}

/// Preset ids usable for files with at least one of these extensions (`[]` = unknown files).
#[tauri::command(async)]
pub fn presets_for_extensions(state: State<'_, AppState>, extensions: Vec<String>) -> Vec<String> {
    state.wheel_config().applicable_ids(&extensions)
}

// ---------------------------------------------------------------- running a preset

/// Run preset `preset_id` on `paths`. `interactive` (wheel, app): `Annotate` opens the image
/// editor with the image, `CopyTo` without a folder asks for one (cancel -> `Ok(vec![])`), and a
/// missing tool shows the `toolNeeded` toast (install, then run). Not interactive (watch
/// folders): those cases return an error instead and nothing is shown. `output_dir` overrides
/// the preset's and the settings' output folder. Clipboard-store files are never moved (copied
/// instead). Interactive convert presets update the recent targets (in `submit_with`); watch
/// folder runs do not. Interactive calls block on the folder picker: never make them on the
/// main thread.
pub fn run_preset(
    app: &AppHandle,
    paths: Vec<PathBuf>,
    preset_id: &str,
    interactive: bool,
    output_dir: Option<PathBuf>,
) -> CmdResult<Vec<JobId>> {
    if paths.is_empty() {
        return Err(CmdError::new("unsupported", "no inputs"));
    }
    let state = app.state::<AppState>();
    // A copy: probing checks every file (`is_dir`), which must not block config saves.
    let config = state.wheel_config().clone();
    let Some(preset) = config.preset(preset_id).cloned() else {
        return Err(fc_core::Error::Invalid(format!("unknown preset {preset_id}")).into());
    };
    let slot = probe(&config, preset_id, &paths);
    let slot = slot.ok_or_else(|| {
        CmdError::new(
            "unsupported",
            format!("preset {preset_id} does not fit these files"),
        )
    })?;
    if let Some(tool) = slot
        .needs_tools
        .iter()
        .find(|t| !state.engines().is_available(t))
    {
        if !interactive {
            return Err(fc_core::Error::MissingTool(tool.clone()).into());
        }
        offer_tool(app, &state, tool, &paths, preset_id);
        return Ok(vec![]);
    }
    let preset = match &preset.op {
        Operation::Annotate => {
            if !interactive {
                return Err(
                    fc_core::Error::Invalid("annotate opens the image editor".into()).into(),
                );
            }
            // The preset fits exactly one image.
            windows::show_main(
                app,
                Some(NavPayload {
                    view: "editor".into(),
                    paths: vec![paths[0].to_string_lossy().into_owned()],
                    url: None,
                    clip: None,
                }),
            );
            return Ok(vec![]);
        }
        // File actions: handled here, never queued, no Recent entry.
        Operation::CopyPath | Operation::OpenTerminal | Operation::Float | Operation::CopyText => {
            if !interactive {
                return Err(
                    fc_core::Error::Invalid("file actions are not available here".into()).into(),
                );
            }
            match &preset.op {
                Operation::CopyPath => crate::line::copy_paths(app, &paths)?,
                Operation::OpenTerminal => crate::line::open_terminal(&paths)?,
                Operation::CopyText => {
                    // OCR takes a moment: not on the caller's thread (the wheel's drop).
                    let app = app.clone();
                    std::thread::spawn(move || {
                        if let Err(e) = crate::line::copy_image_text(&app, &paths) {
                            eprintln!("copy text: {}", e.message);
                        }
                    });
                }
                _ => crate::float::open(app, paths),
            }
            return Ok(vec![]);
        }
        Operation::CopyTo {
            dir: None,
            move_files,
        } => {
            if !interactive {
                return Err(fc_core::Error::Invalid("no target folder chosen".into()).into());
            }
            let Some(dir) = pick_folder(app) else {
                return Ok(vec![]);
            };
            Preset {
                op: Operation::CopyTo {
                    dir: Some(dir),
                    move_files: *move_files,
                },
                ..preset
            }
        }
        _ => preset,
    };
    let id = preset.id.clone();
    let single = WheelConfig {
        version: presets::VERSION,
        presets: vec![preset],
        groups: Vec::new(),
        profiles: Vec::new(),
    };
    let mut ids = Vec::new();
    for req in single.requests(&id, &paths)? {
        for mut req in keep_clipboard_files(req, |p| state.paths.is_clipboard_file(p)) {
            if let Some(dir) = &output_dir {
                req.options.output_dir = Some(dir.clone());
            }
            // Watch-folder runs are not the user's choice: no recent-target update.
            ids.extend(submit_with(&state, req, interactive)?);
        }
    }
    Ok(ids)
}

/// The wheel slot `preset_id` gets for `paths`, None when it does not fit every file.
fn probe(config: &WheelConfig, preset_id: &str, paths: &[PathBuf]) -> Option<WheelSlot> {
    let profile = Profile {
        id: String::new(),
        name: String::new(),
        builtin: false,
        extensions: Vec::new(),
        mixed: false,
        front: vec![preset_id.to_string()],
        back: Vec::new(),
    };
    config
        .slots(&profile, Face::Front, paths)
        .into_iter()
        .next()
}

/// Blocking folder picker (off the main thread). None = cancelled.
fn pick_folder(app: &AppHandle) -> Option<PathBuf> {
    use tauri_plugin_dialog::DialogExt;
    app.dialog()
        .file()
        .blocking_pick_folder()
        .and_then(|p| p.into_path().ok())
}

/// `toolNeeded` toast: "Download" installs `tool`, then runs the preset on `paths`
/// (`windows::toast_action`).
fn offer_tool(app: &AppHandle, state: &AppState, tool: &str, paths: &[PathBuf], preset_id: &str) {
    let entry = state.engines().manifest().get(tool);
    windows::show_toast(
        app,
        "toolNeeded",
        json!({
            "tool": tool,
            "name": entry.map(|e| e.name.clone()).unwrap_or_else(|| tool.to_string()),
            "size": entry.map(|e| e.size).unwrap_or(0),
            "paths": paths,
            "target": preset_id,
        }),
        Some(12000),
    );
}

// ---------------------------------------------------------------- wheel payload

/// `wheel_slots` with the app's settings, wheel config and installed tools. `shift` = the drag
/// started with Shift (the back face comes first when `wheel_shift_back` is on).
pub fn wheel_slots_for(
    app: &AppHandle,
    paths: Vec<String>,
    shift: bool,
    click: bool,
) -> WheelSlots {
    let state = app.state::<AppState>();
    let s = state.settings();
    // A copy: the file checks below must not block saving the config.
    let config = state.wheel_config().clone();
    let engines = state.engines().clone();
    let back = shift && s.wheel_shift_back;
    wheel_slots(&config, &s, paths, back, click, |t| engines.is_available(t))
}

/// The fallback profile: unknown files, folders, and profiles with nothing that fits.
const OTHER: &str = "other";

/// The slots of `profile` that are file operations (zip, copy, move; groups of them).
fn file_ops(
    config: &WheelConfig,
    profile: &Profile,
    files: &[PathBuf],
) -> (Vec<WheelSlot>, Vec<WheelSlot>) {
    let file_op = |s: &WheelSlot| s.kind.is_none() && s.children.iter().all(|c| c.kind.is_none());
    let face = |f| {
        config
            .slots(profile, f, files)
            .into_iter()
            .filter(file_op)
            .collect::<Vec<_>>()
    };
    (face(Face::Front), face(Face::Back))
}

/// A profile whose faces have nothing for these files (the user emptied it): the file
/// operations of `other`, or of the built-in `other` when that one is empty too. Evaluated
/// against `config`, so every slot is a preset `run_preset` can find.
fn fallback_slots(
    config: &WheelConfig,
    files: &[PathBuf],
) -> Option<(Profile, Vec<WheelSlot>, Vec<WheelSlot>)> {
    let builtin = WheelConfig::builtin();
    let user_other = config.profiles.iter().find(|p| p.id == OTHER);
    let builtin_other = builtin.profiles.iter().find(|p| p.id == OTHER);
    user_other
        .into_iter()
        .chain(builtin_other)
        .map(|p| {
            let (front, back) = file_ops(config, p, files);
            (p.clone(), front, back)
        })
        .find(|(_, front, back)| !front.is_empty() || !back.is_empty())
}

/// The wheel payload for `paths`: their profile, the slots of both faces that fit every file,
/// the tools those slots miss (`available` says which are installed) and the look from the
/// settings. `back` asks for the back face first (Ctrl+Shift+drag); it is shown only when it
/// has slots, and an empty front shows the back. A profile with no fitting slot on either face
/// falls back to the file operations of `other`, so the wheel is never empty.
pub fn wheel_slots(
    config: &WheelConfig,
    s: &Settings,
    paths: Vec<String>,
    back: bool,
    click: bool,
    available: impl Fn(&str) -> bool,
) -> WheelSlots {
    let files: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    let mut profile = config.profile_for(&files).clone();
    let mut front = config.slots(&profile, Face::Front, &files);
    let mut back_slots = config.slots(&profile, Face::Back, &files);
    if front.is_empty()
        && back_slots.is_empty()
        && let Some((other, f, b)) = fallback_slots(config, &files)
    {
        (profile, front, back_slots) = (other, f, b);
    }
    // Group slots already carry their members' tools. Each tool is checked once.
    let mut checked: Vec<(&str, bool)> = Vec::new();
    let mut missing_tools: Vec<String> = Vec::new();
    for tool in front
        .iter()
        .chain(&back_slots)
        .flat_map(|slot| slot.needs_tools.iter())
    {
        if checked.iter().any(|(t, _)| *t == tool.as_str()) {
            continue;
        }
        let ok = available(tool);
        checked.push((tool, ok));
        if !ok {
            missing_tools.push(tool.clone());
        }
    }
    let face = if !back_slots.is_empty() && (back || front.is_empty()) {
        Face::Back
    } else {
        Face::Front
    };
    let first = files.first().filter(|p| !p.is_dir());
    WheelSlots {
        count: paths.len(),
        first_ext: first.and_then(|p| fc_core::engines::ext_of(p)),
        first_kind: first.and_then(|p| formats::detect_path(p)).map(|f| f.kind),
        profile_id: profile.id,
        profile_name: profile.name,
        paths,
        front,
        back: back_slots,
        face,
        click,
        missing_tools,
        style: wheel_style(s),
    }
}

/// Hovered-slice colours of the accent names (as the app's accents).
const ACCENTS: &[(&str, &str)] = &[
    ("blue", "#007AFF"),
    ("purple", "#7C6CF0"),
    ("pink", "#FF2D55"),
    ("red", "#FF3B30"),
    ("orange", "#FF9500"),
    ("yellow", "#FFCC00"),
    ("green", "#34C759"),
    ("graphite", "#8E8E93"),
];
/// The default wheel colour (purple).
const DEFAULT_WHEEL_COLOR: &str = "#7C6CF0";

fn accent_hex(name: &str) -> &'static str {
    ACCENTS
        .iter()
        .find(|(n, _)| *n == name)
        .map_or(DEFAULT_WHEEL_COLOR, |(_, hex)| hex)
}

/// Wheel size setting as a factor (80..=130 % -> 0.8..=1.3; out-of-range values clamped).
pub fn wheel_scale(s: &Settings) -> f64 {
    f64::from(s.wheel_scale.clamp(80, 130)) / 100.0
}

/// The wheel's look from the settings: "accent" takes the app accent's colour.
pub fn wheel_style(s: &Settings) -> WheelStyle {
    let colour = if s.wheel_color == "accent" {
        &s.accent
    } else {
        &s.wheel_color
    };
    WheelStyle {
        labels: s.wheel_labels.clone(),
        opacity: s.wheel_opacity.clamp(40, 100),
        color: accent_hex(colour).to_string(),
        scale: wheel_scale(s),
        show_detail: s.wheel_show_detail,
        reduce_motion: s.reduce_motion,
    }
}

/// "move-to" must never move files out of the clipboard store (the clipboard history would
/// lose them): those inputs are copied instead.
fn keep_clipboard_files(req: JobRequest, is_clip: impl Fn(&Path) -> bool) -> Vec<JobRequest> {
    let Operation::CopyTo {
        dir,
        move_files: true,
    } = &req.op
    else {
        return vec![req];
    };
    let copy = Operation::CopyTo {
        dir: dir.clone(),
        move_files: false,
    };
    let (clip, other): (Vec<PathBuf>, Vec<PathBuf>) =
        req.inputs.into_iter().partition(|p| is_clip(p));
    let mut out = Vec::with_capacity(2);
    if !other.is_empty() {
        out.push(JobRequest {
            inputs: other,
            op: req.op,
            options: req.options.clone(),
        });
    }
    if !clip.is_empty() {
        out.push(JobRequest {
            inputs: clip,
            op: copy,
            options: req.options,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use fc_core::formats::Kind;
    use fc_core::jobs::Operation;
    use fc_core::presets::Face;
    use std::path::PathBuf;

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn wheel_slots_payload_for_images() {
        let config = WheelConfig::builtin();
        let s = Settings::default();
        let p = wheel_slots(&config, &s, strings(&["a.png"]), false, false, |_| true);
        assert!(!p.front.is_empty());
        assert_eq!(p.count, 1);
        assert_eq!(p.first_ext.as_deref(), Some("png"));
        assert_eq!(p.first_kind, Some(Kind::Image));
        assert_eq!(p.paths, ["a.png"]);
        assert_eq!(p.face, Face::Front);
        assert!(!p.click);
        assert!(p.missing_tools.is_empty());
        // The profile is the images one, and nothing on it converts a PNG to PNG.
        assert_eq!(
            p.profile_id,
            config.profile_for(&[PathBuf::from("a.png")]).id
        );
        assert!(p.front.iter().all(|slot| slot.id != "to-png"));
        assert_eq!(p.style, wheel_style(&s));
    }

    #[test]
    fn back_face_only_when_asked_and_not_empty() {
        let config = WheelConfig::builtin();
        let s = Settings::default();
        let p = wheel_slots(&config, &s, strings(&["a.png"]), true, true, |_| true);
        assert!(p.click);
        assert_eq!(
            p.face,
            if p.back.is_empty() {
                Face::Front
            } else {
                Face::Back
            }
        );
        // A folder gets the "other" profile (file operations only) and no extension or kind.
        let dir = env!("CARGO_MANIFEST_DIR").to_string();
        let p = wheel_slots(&config, &s, vec![dir], false, false, |_| true);
        assert_eq!(p.profile_id, "other");
        assert_eq!(p.first_ext, None);
        assert_eq!(p.first_kind, None);
        assert!(!p.front.is_empty());
    }

    #[test]
    fn empty_profile_falls_back_to_file_operations() {
        let mut config = WheelConfig::builtin();
        let images = config.profile_for(&[PathBuf::from("a.png")]).id.clone();
        let clear = |config: &mut WheelConfig, id: &str| {
            let p = config.profiles.iter_mut().find(|p| p.id == id).unwrap();
            p.front.clear();
            p.back.clear();
        };
        // The user emptied the images profile: the wheel shows the file operations of "other".
        clear(&mut config, &images);
        let s = Settings::default();
        let p = wheel_slots(&config, &s, strings(&["a.png"]), false, false, |_| true);
        assert_eq!(p.profile_id, "other");
        assert!(!p.front.is_empty());
        assert_eq!(p.face, Face::Front);
        let file_op = |slot: &WheelSlot| slot.kind.is_none();
        assert!(p.front.iter().chain(&p.back).all(file_op));
        // "other" emptied too: the built-in file operations, never an empty wheel.
        clear(&mut config, "other");
        let p = wheel_slots(&config, &s, strings(&["a.png"]), false, false, |_| true);
        assert!(!p.front.is_empty());
        assert!(p.front.iter().chain(&p.back).all(file_op));
    }

    #[test]
    fn missing_tools_are_listed_once() {
        let config = WheelConfig::builtin();
        let s = Settings::default();
        let p = wheel_slots(&config, &s, strings(&["a.mp4"]), false, false, |_| false);
        assert_eq!(p.missing_tools, ["ffmpeg"]);
        let p = wheel_slots(&config, &s, strings(&["a.mp4"]), false, false, |_| true);
        assert!(p.missing_tools.is_empty());
    }

    #[test]
    fn wheel_style_resolves_colours_and_limits() {
        let mut s = Settings {
            wheel_color: "purple".into(),
            wheel_scale: 130,
            wheel_opacity: 70,
            wheel_labels: "icon".into(),
            ..Settings::default()
        };
        let st = wheel_style(&s);
        assert_eq!(st.color, "#7C6CF0");
        assert!((st.scale - 1.3).abs() < 1e-9);
        assert_eq!(st.opacity, 70);
        assert_eq!(st.labels, "icon");
        // "accent" follows the app accent; unknown names fall back to purple.
        s.wheel_color = "accent".into();
        s.accent = "green".into();
        assert_eq!(wheel_style(&s).color, "#34C759");
        s.accent = "graphite".into();
        assert_eq!(wheel_style(&s).color, "#8E8E93");
        s.wheel_color = "banana".into();
        assert_eq!(wheel_style(&s).color, "#7C6CF0");
        for (name, hex) in [
            ("blue", "#007AFF"),
            ("pink", "#FF2D55"),
            ("red", "#FF3B30"),
            ("orange", "#FF9500"),
            ("yellow", "#FFCC00"),
        ] {
            s.wheel_color = name.into();
            assert_eq!(wheel_style(&s).color, hex);
        }
        // Out-of-range values from a hand-edited file are clamped.
        s.wheel_scale = 500;
        s.wheel_opacity = 5;
        let st = wheel_style(&s);
        assert!((st.scale - 1.3).abs() < 1e-9);
        assert_eq!(st.opacity, 40);
    }

    #[test]
    fn move_to_never_moves_clipboard_files() {
        let is_clip = |p: &Path| p.starts_with("C:/data/clipboard");
        let dir = Some(PathBuf::from("D:/out"));
        let req = |op: Operation| JobRequest {
            inputs: vec!["C:/x/a.png".into(), "C:/data/clipboard/img/1.png".into()],
            op,
            options: Default::default(),
        };
        let moved = keep_clipboard_files(
            req(Operation::CopyTo {
                dir: dir.clone(),
                move_files: true,
            }),
            is_clip,
        );
        assert_eq!(moved.len(), 2);
        assert_eq!(moved[0].inputs, [PathBuf::from("C:/x/a.png")]);
        assert_eq!(
            moved[0].op,
            Operation::CopyTo {
                dir: dir.clone(),
                move_files: true
            }
        );
        assert_eq!(
            moved[1].inputs,
            [PathBuf::from("C:/data/clipboard/img/1.png")]
        );
        assert_eq!(
            moved[1].op,
            Operation::CopyTo {
                dir: dir.clone(),
                move_files: false
            }
        );
        // Only clipboard files: one copy job. Copies and other operations are untouched.
        let only_clip = keep_clipboard_files(
            JobRequest {
                inputs: vec!["C:/data/clipboard/img/1.png".into()],
                op: Operation::CopyTo {
                    dir: dir.clone(),
                    move_files: true,
                },
                options: Default::default(),
            },
            is_clip,
        );
        assert_eq!(only_clip.len(), 1);
        assert!(matches!(
            only_clip[0].op,
            Operation::CopyTo {
                move_files: false,
                ..
            }
        ));
        let copied = req(Operation::CopyTo {
            dir: dir.clone(),
            move_files: false,
        });
        assert_eq!(keep_clipboard_files(copied.clone(), is_clip), [copied]);
        let zipped = req(Operation::Zip);
        assert_eq!(keep_clipboard_files(zipped.clone(), is_clip), [zipped]);
    }
}

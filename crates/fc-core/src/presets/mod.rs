//! Presets, groups and per-file-type wheel profiles (`<config>/wheel.json`). OWNER: Task 3.
//!
//! - A [`Preset`] is a ready job: an operation plus options. Built-ins have stable ids and an
//!   empty name (the UI localizes them by id); user presets carry their own name.
//! - A [`Group`] puts up to [`MAX_GROUP`] presets behind one wheel slot (`"group:<id>"`).
//! - A [`Profile`] maps file extensions to a wheel arrangement: a front face of up to
//!   [`MAX_FRONT`] slots and a back face of up to [`MAX_BACK`].
//!
//! Loading never fails: a missing or corrupt file gives the built-ins; unknown entries are
//! dropped, dangling references removed, over-long lists cut, and built-ins missing from the
//! file merged in without touching the user's arrangements. Before saving, the shell runs
//! [`WheelConfig::validate`], which enforces the same rules strictly.

mod builtin;

use crate::formats::{self, FormatInfo, Kind, planner, registry::EngineId};
use crate::jobs::{ConvertOptions, ImageOp, JobRequest, Operation};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// `WheelConfig::version` written by this build.
pub const VERSION: u32 = 1;
pub const MAX_FRONT: usize = 8;
pub const MAX_BACK: usize = 12;
pub const MAX_GROUP: usize = 6;
/// Slot ids starting with this name a group (`"group:files"`); others name a preset.
pub const GROUP_PREFIX: &str = "group:";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: String,
    /// "" for built-ins (the UI localizes them by id).
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub builtin: bool,
    pub op: Operation,
    #[serde(default)]
    pub options: ConvertOptions,
}

/// Several presets behind one wheel slot (TS `PresetGroup`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// Preset ids, at most [`MAX_GROUP`].
    #[serde(default)]
    pub presets: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    /// "" for built-ins (the UI shows `presets.profile.<id>`).
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub builtin: bool,
    /// Lowercase, without dot. Empty for the "other" fallback and the mixed profile.
    #[serde(default)]
    pub extensions: Vec<String>,
    /// Used when a selection spans several profiles.
    #[serde(default)]
    pub mixed: bool,
    /// Slot ids: preset id or `"group:<group id>"`.
    #[serde(default)]
    pub front: Vec<String>,
    #[serde(default)]
    pub back: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WheelConfig {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub presets: Vec<Preset>,
    #[serde(default)]
    pub groups: Vec<Group>,
    #[serde(default)]
    pub profiles: Vec<Profile>,
}

/// Wheel face (TS `WheelFace`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Face {
    #[default]
    Front,
    Back,
}

/// One resolved wheel slot: only slots that fit every dragged file are produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WheelSlot {
    /// Preset id, or `"group:<id>"`.
    pub id: String,
    /// User-given name ("" for built-ins).
    pub name: String,
    /// Output format label for convert presets ("JPG"), else `None`.
    pub format: Option<String>,
    /// Detail line: "70%", "16:9", "2×", "1920 px".
    pub detail: Option<String>,
    /// A TS `SlotIcon` key.
    pub icon: String,
    /// Output kind, `None` for file operations.
    pub kind: Option<Kind>,
    /// Tool ids this preset needs for these files ("ffmpeg", "pdfium").
    pub needs_tools: Vec<String>,
    /// Group members (non-empty only for groups).
    pub children: Vec<WheelSlot>,
}

fn builtin_config() -> &'static WheelConfig {
    static BUILTIN: OnceLock<WheelConfig> = OnceLock::new();
    BUILTIN.get_or_init(|| WheelConfig {
        version: VERSION,
        presets: builtin::presets(),
        groups: builtin::groups(),
        profiles: builtin::profiles(),
    })
}

fn invalid(msg: impl Into<String>) -> Error {
    Error::Invalid(msg.into())
}

/// `".JPG "` -> `"jpg"`.
fn normalize_ext(ext: &str) -> String {
    ext.trim().trim_start_matches('.').to_ascii_lowercase()
}

/// Lowercase, no dots, no blanks, no duplicates; order kept.
fn normalize_exts(exts: &mut Vec<String>) {
    let mut seen = HashSet::new();
    *exts = exts
        .iter()
        .map(|e| normalize_ext(e))
        .filter(|e| !e.is_empty() && seen.insert(e.clone()))
        .collect();
}

fn dedupe(ids: &mut Vec<String>) {
    let mut seen = HashSet::new();
    ids.retain(|id| seen.insert(id.clone()));
}

/// Parse each array element on its own, so one bad entry does not cost the others.
fn entries<T: for<'de> Deserialize<'de>>(
    file: &mut serde_json::Map<String, Value>,
    key: &str,
) -> Vec<T> {
    match file.remove(key) {
        Some(Value::Array(items)) => items
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect(),
        _ => Vec::new(),
    }
}

impl WheelConfig {
    /// The built-in presets, the "files" group and the built-in profiles.
    pub fn builtin() -> Self {
        builtin_config().clone()
    }

    /// Never fails (see the module docs). Missing or corrupt file -> [`WheelConfig::builtin`].
    pub fn load(path: &Path) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .map(Self::from_value)
            .unwrap_or_else(Self::builtin)
    }

    fn from_value(v: Value) -> Self {
        let Value::Object(mut file) = v else {
            return Self::builtin();
        };
        let base = builtin_config();

        // Built-in presets are not editable: they always come from this build. User presets
        // follow in file order; ones with a built-in or repeated id, a `builtin` flag (a
        // built-in this build no longer has) or invalid parameters are dropped.
        let mut presets = base.presets.clone();
        for p in entries::<Preset>(&mut file, "presets") {
            let usable = !p.builtin
                && valid_id(&p.id)
                && !p.id.starts_with(GROUP_PREFIX)
                && !presets.iter().any(|q| q.id == p.id)
                && p.op.validate().is_ok()
                && p.options.validate().is_ok();
            if usable {
                presets.push(p);
            }
        }
        let preset_known = |id: &str| presets.iter().any(|p| p.id == id);

        let mut groups: Vec<Group> = Vec::new();
        for g in entries::<Group>(&mut file, "groups") {
            if valid_id(&g.id) && !groups.iter().any(|x| x.id == g.id) {
                groups.push(g);
            }
        }
        for g in &base.groups {
            if !groups.iter().any(|x| x.id == g.id) {
                groups.push(g.clone());
            }
        }
        for g in &mut groups {
            g.presets.retain(|id| preset_known(id));
            dedupe(&mut g.presets);
            g.presets.truncate(MAX_GROUP);
        }

        let mut profiles: Vec<Profile> = Vec::new();
        for mut p in entries::<Profile>(&mut file, "profiles") {
            if !valid_id(&p.id) || profiles.iter().any(|x| x.id == p.id) {
                continue;
            }
            p.builtin = base.profiles.iter().any(|b| b.id == p.id);
            normalize_exts(&mut p.extensions);
            profiles.push(p);
        }
        for p in &base.profiles {
            if !profiles.iter().any(|x| x.id == p.id) {
                profiles.push(p.clone());
            }
        }
        let slot_known = |id: &str| match id.strip_prefix(GROUP_PREFIX) {
            Some(g) => groups.iter().any(|x| x.id == g),
            None => preset_known(id),
        };
        for p in &mut profiles {
            for (face, max) in [(&mut p.front, MAX_FRONT), (&mut p.back, MAX_BACK)] {
                face.retain(|id| slot_known(id));
                dedupe(face);
                face.truncate(max);
            }
        }

        Self {
            version: VERSION,
            presets,
            groups,
            profiles,
        }
    }

    /// Atomic write (temp + rename), creating the parent folder. Does not validate: the shell
    /// calls [`WheelConfig::validate`] first.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| Error::Failed(e.to_string()))?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(&tmp, path).inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })?;
        Ok(())
    }

    /// Strict check before saving: unique non-empty ids, faces and groups within their limits,
    /// no slot twice on one face, every reference resolves, every preset's parameters valid,
    /// built-in presets unchanged (they are read-only: the UI duplicates them instead).
    /// Normalizes `version`, the `builtin` flags and extension spelling.
    pub fn validate(mut self) -> Result<Self> {
        let base = builtin_config();
        let mut ids = HashSet::new();
        for p in &self.presets {
            if !valid_id(&p.id) || p.id.starts_with(GROUP_PREFIX) {
                return Err(invalid(format!("bad preset id {:?}", p.id)));
            }
            if !ids.insert(p.id.as_str()) {
                return Err(invalid(format!("duplicate preset id {}", p.id)));
            }
            // Loading always restores the build's built-ins, so an edit would be lost silently.
            if let Some(b) = base.presets.iter().find(|b| b.id == p.id)
                && (b.op != p.op || b.options != p.options || b.name != p.name)
            {
                return Err(invalid(format!(
                    "built-in presets are read-only; duplicate it: {}",
                    p.id
                )));
            }
            p.op.validate()
                .and_then(|()| p.options.validate())
                .map_err(|e| invalid(format!("preset {}: {e}", p.id)))?;
        }
        let mut group_ids = HashSet::new();
        for g in &self.groups {
            if !valid_id(&g.id) || !group_ids.insert(g.id.as_str()) {
                return Err(invalid(format!("bad or duplicate group id {:?}", g.id)));
            }
            if g.presets.len() > MAX_GROUP {
                return Err(invalid(format!(
                    "group {}: {} presets (max {MAX_GROUP})",
                    g.id,
                    g.presets.len()
                )));
            }
            check_list(&g.presets, &format!("group {}", g.id), |id| {
                ids.contains(id)
            })?;
        }
        let mut profile_ids = HashSet::new();
        for p in &self.profiles {
            if !valid_id(&p.id) || !profile_ids.insert(p.id.as_str()) {
                return Err(invalid(format!("bad or duplicate profile id {:?}", p.id)));
            }
            for (face, list, max) in [("front", &p.front, MAX_FRONT), ("back", &p.back, MAX_BACK)] {
                if list.len() > max {
                    return Err(invalid(format!(
                        "profile {} {face}: {} slots (max {max})",
                        p.id,
                        list.len()
                    )));
                }
                check_list(list, &format!("profile {} {face}", p.id), |id| {
                    match id.strip_prefix(GROUP_PREFIX) {
                        Some(g) => group_ids.contains(g),
                        None => ids.contains(id),
                    }
                })?;
            }
        }
        self.version = VERSION;
        for p in &mut self.presets {
            p.builtin = base.presets.iter().any(|b| b.id == p.id);
        }
        for p in &mut self.profiles {
            p.builtin = base.profiles.iter().any(|b| b.id == p.id);
            normalize_exts(&mut p.extensions);
        }
        Ok(self)
    }

    /// Built-in profile -> back to its default arrangement and options (restoring built-in
    /// groups and presets that arrangement uses). Custom or unknown profile -> `false`.
    pub fn reset_profile(&mut self, id: &str) -> bool {
        let base = builtin_config();
        let Some(default) = base.profiles.iter().find(|p| p.id == id) else {
            return false;
        };
        match self.profiles.iter_mut().find(|p| p.id == id) {
            Some(p) => *p = default.clone(),
            None => self.profiles.push(default.clone()),
        }
        for slot in default.front.iter().chain(&default.back) {
            match slot.strip_prefix(GROUP_PREFIX) {
                Some(g) => {
                    if !self.groups.iter().any(|x| x.id == g)
                        && let Some(b) = base.groups.iter().find(|x| x.id == g)
                    {
                        self.groups.push(b.clone());
                    }
                }
                None => {
                    if self.preset(slot).is_none()
                        && let Some(b) = base.presets.iter().find(|x| x.id == *slot)
                    {
                        self.presets.push(b.clone());
                    }
                }
            }
        }
        true
    }

    pub fn preset(&self, id: &str) -> Option<&Preset> {
        self.presets.iter().find(|p| p.id == id)
    }

    /// The profile for a drop: all files in one profile -> that profile; files from several
    /// profiles (unknown files count as "other") -> the first `mixed` profile; only unknown
    /// files, no files, or no mixed profile -> `other`. Never fails.
    pub fn profile_for(&self, paths: &[PathBuf]) -> &Profile {
        let mut owners = paths.iter().map(|p| self.owner(p));
        let Some(first) = owners.next() else {
            return self.other();
        };
        let same = owners.all(|o| match (o, first) {
            (Some(a), Some(b)) => a.id == b.id,
            (None, None) => true,
            _ => false,
        });
        match (same, first) {
            (true, Some(p)) => p,
            (true, None) => self.other(),
            (false, _) => self
                .profiles
                .iter()
                .find(|p| p.mixed)
                .unwrap_or_else(|| self.other()),
        }
    }

    /// The first profile listing the file's extension.
    fn owner(&self, path: &Path) -> Option<&Profile> {
        if path.is_dir() {
            return None; // a folder is "other", even when named like "photos.png"
        }
        let ext = crate::engines::ext_of(path)?;
        self.profiles.iter().find(|p| p.extensions.contains(&ext))
    }

    fn other(&self) -> &Profile {
        static FALLBACK: OnceLock<Profile> = OnceLock::new();
        self.profiles
            .iter()
            .find(|p| p.id == "other")
            .unwrap_or_else(|| FALLBACK.get_or_init(builtin::other))
    }

    /// The slots of one face that fit every file in `paths`. Groups keep only fitting members
    /// and disappear when none fits. Unknown slot ids are skipped.
    pub fn slots(&self, profile: &Profile, face: Face, paths: &[PathBuf]) -> Vec<WheelSlot> {
        let ids = match face {
            Face::Front => &profile.front,
            Face::Back => &profile.back,
        };
        ids.iter().filter_map(|id| self.slot(id, paths)).collect()
    }

    fn slot(&self, id: &str, paths: &[PathBuf]) -> Option<WheelSlot> {
        let Some(group_id) = id.strip_prefix(GROUP_PREFIX) else {
            return self
                .preset(id)
                .filter(|p| applicable(p, paths))
                .map(|p| describe(p, paths));
        };
        let group = self.groups.iter().find(|g| g.id == group_id)?;
        let children: Vec<WheelSlot> = group
            .presets
            .iter()
            .filter_map(|pid| self.preset(pid))
            .filter(|p| applicable(p, paths))
            .map(|p| describe(p, paths))
            .collect();
        if children.is_empty() {
            return None;
        }
        let tools: BTreeSet<String> = children
            .iter()
            .flat_map(|c| c.needs_tools.iter().cloned())
            .collect();
        Some(WheelSlot {
            id: id.to_string(),
            name: group.name.clone(),
            format: None,
            detail: None,
            icon: "group".into(),
            kind: None,
            needs_tools: tools.into_iter().collect(),
            children,
        })
    }

    /// Ids of presets that fit a file with at least one of these extensions (the wheel editor
    /// dims the others). Merging counts as fitting when it fits two such files. An empty list
    /// (the "other" and mixed profiles) means an unknown file: the file operations.
    pub fn applicable_ids(&self, extensions: &[String]) -> Vec<String> {
        let mut exts: Vec<String> = extensions
            .iter()
            .map(|e| normalize_ext(e))
            .filter(|e| !e.is_empty())
            .collect();
        if exts.is_empty() {
            // No format claims this extension, so the sample is an unknown file.
            exts.push("fc-unknown".into());
        }
        let samples: Vec<[Vec<PathBuf>; 2]> = exts
            .iter()
            .map(|e| {
                let one = PathBuf::from(format!("a.{e}"));
                let two = PathBuf::from(format!("b.{e}"));
                [vec![one.clone()], vec![one, two]]
            })
            .collect();
        self.presets
            .iter()
            .filter(|p| samples.iter().flatten().any(|s| applicable(p, s)))
            .map(|p| p.id.clone())
            .collect()
    }

    /// Job requests for running `preset_id` on `paths`: one request with every path (the queue
    /// splits it per input where needed; merge and zip stay one job). A convert leaves out
    /// files that already have the target format. Does not check that the preset fits the
    /// files. `CopyTo` without a folder and `Annotate` -> `Error::Invalid`: the shell resolves
    /// those first.
    pub fn requests(&self, preset_id: &str, paths: &[PathBuf]) -> Result<Vec<JobRequest>> {
        let preset = self
            .preset(preset_id)
            .ok_or_else(|| invalid(format!("unknown preset {preset_id}")))?;
        preset.op.queueable()?;
        let inputs: Vec<PathBuf> = match &preset.op {
            Operation::Convert { to } => paths
                .iter()
                .filter(|p| formats::detect_path(p).is_none_or(|f| f.id != to))
                .cloned()
                .collect(),
            _ => paths.to_vec(),
        };
        if inputs.is_empty() {
            return Ok(Vec::new());
        }
        Ok(vec![JobRequest {
            inputs,
            op: preset.op.clone(),
            options: preset.options.clone(),
        }])
    }
}

fn valid_id(id: &str) -> bool {
    !id.trim().is_empty()
}

/// Every id resolves and none repeats.
fn check_list(list: &[String], what: &str, known: impl Fn(&str) -> bool) -> Result<()> {
    let mut seen = HashSet::new();
    for id in list {
        if !known(id) {
            return Err(invalid(format!("{what}: unknown {id}")));
        }
        if !seen.insert(id.as_str()) {
            return Err(invalid(format!("{what}: {id} twice")));
        }
    }
    Ok(())
}

/// Whether `preset` can handle *every* file in `paths` (empty -> false):
/// convert needs a route for each file that is not already the target (and one that is not);
/// edits need raster images; crops images or video; merging two or more pdf/images; the
/// editor exactly one image; file operations (zip, copy, move) take anything, and are all
/// that folders and unknown files get.
pub fn applicable(preset: &Preset, paths: &[PathBuf]) -> bool {
    if paths.is_empty() {
        return false;
    }
    let file_op = matches!(
        preset.op,
        Operation::CopyTo { .. } | Operation::Zip | Operation::CopyPath | Operation::OpenTerminal
    );
    // Folders (whatever their name looks like) only take the file operations.
    if paths.iter().any(|p| p.is_dir()) {
        return file_op;
    }
    let Some(fmts) = paths
        .iter()
        .map(|p| formats::detect_path(p))
        .collect::<Option<Vec<&FormatInfo>>>()
    else {
        // So do unknown files.
        return file_op;
    };
    let all = |ok: fn(Kind) -> bool| fmts.iter().all(|f| ok(f.kind));
    match &preset.op {
        Operation::Convert { to } => {
            let mut differs = false;
            for f in &fmts {
                if f.id != to {
                    differs = true;
                    if planner::plan(f.id, to).is_none() {
                        return false;
                    }
                }
            }
            differs
        }
        Operation::Edit { .. } => all(|k| k == Kind::Image),
        Operation::Annotate => paths.len() == 1 && all(|k| k == Kind::Image),
        Operation::CropAspect { .. } | Operation::Crop { .. } => {
            all(|k| matches!(k, Kind::Image | Kind::Video))
        }
        Operation::MergePdf => paths.len() >= 2 && all(|k| matches!(k, Kind::Pdf | Kind::Image)),
        Operation::SplitPdf => all(|k| k == Kind::Pdf),
        Operation::Compress { .. } => {
            all(|k| matches!(k, Kind::Image | Kind::Video | Kind::Audio | Kind::Pdf))
        }
        Operation::Trim { .. } | Operation::Speed { .. } => {
            all(|k| matches!(k, Kind::Video | Kind::Audio))
        }
        Operation::Mute => all(|k| k == Kind::Video),
        Operation::Extract => all(|k| k == Kind::Archive),
        Operation::Float | Operation::CopyText => all(|k| k == Kind::Image),
        Operation::CopyTo { .. }
        | Operation::Zip
        | Operation::CopyPath
        | Operation::OpenTerminal => true,
    }
}

/// `SlotIcon` key of a kind (same spelling as its serde name).
fn kind_icon(kind: Kind) -> &'static str {
    match kind {
        Kind::Image => "image",
        Kind::Vector => "vector",
        Kind::Video => "video",
        Kind::Audio => "audio",
        Kind::Pdf => "pdf",
        Kind::Data => "data",
        Kind::Subtitle => "subtitle",
        Kind::Archive => "archive",
    }
}

fn edit_icon(op: &ImageOp) -> &'static str {
    match op {
        ImageOp::Grayscale => "grayscale",
        ImageOp::RotateLeft => "rotateLeft",
        ImageOp::RotateRight => "rotateRight",
        ImageOp::Rotate180 => "rotate180",
        ImageOp::FlipH => "flipH",
        ImageOp::FlipV => "flipV",
        ImageOp::Trim => "trim",
        ImageOp::Pixelate { .. } => "pixelate",
        ImageOp::Blur { .. } => "blur",
        ImageOp::Scale { .. } => "resize",
    }
}

/// "1920 px" / "1280×720 px" from the max size options.
fn size_detail(o: &ConvertOptions) -> Option<String> {
    match (o.max_width, o.max_height) {
        (Some(w), Some(h)) if w != h => Some(format!("{w}×{h} px")),
        (Some(n), _) | (None, Some(n)) => Some(format!("{n} px")),
        (None, None) => None,
    }
}

/// External tools the routes from each file to `to` need (sorted, deduped).
fn route_tools(fmts: &[&FormatInfo], to: &str) -> BTreeSet<&'static str> {
    fmts.iter()
        .filter(|f| f.id != to)
        .filter_map(|f| planner::plan(f.id, to))
        .flat_map(|r| r.steps.into_iter().map(|s| s.engine))
        .filter_map(EngineId::required_tool)
        .collect()
}

/// The slot for a preset that fits `paths`.
fn describe(p: &Preset, paths: &[PathBuf]) -> WheelSlot {
    let fmts: Vec<&FormatInfo> = paths
        .iter()
        .filter_map(|p| formats::detect_path(p))
        .collect();
    let first_kind = fmts.first().map(|f| f.kind);
    let any = |ok: fn(Kind) -> bool| fmts.iter().any(|f| ok(f.kind));
    let media = any(|k| matches!(k, Kind::Video | Kind::Audio));
    let video = any(|k| k == Kind::Video);

    let mut format = None;
    let mut detail = None;
    let mut tools = BTreeSet::new();
    let ffmpeg = |needed: bool| {
        if needed {
            BTreeSet::from(["ffmpeg"])
        } else {
            BTreeSet::new()
        }
    };
    let (icon, kind) = match &p.op {
        Operation::Convert { to } => {
            let target = formats::by_id(to);
            format = target.map(|f| f.label.to_string());
            let mut parts = Vec::new();
            if let Some(q) = p.options.quality {
                parts.push(format!("{q}%"));
            }
            parts.extend(size_detail(&p.options));
            detail = (!parts.is_empty()).then(|| parts.join(" · "));
            tools = route_tools(&fmts, to);
            let kind = target.map(|f| f.kind);
            let icon = if to == "m4r" {
                "ringtone"
            } else {
                kind.map_or("data", kind_icon)
            };
            (icon, kind)
        }
        Operation::Edit { ops, to } => {
            format = to
                .as_deref()
                .and_then(formats::by_id)
                .map(|f| f.label.to_string());
            detail = ops
                .iter()
                .find_map(|op| match op {
                    ImageOp::Scale { percent } => Some(format!("{percent}%")),
                    _ => None,
                })
                .or_else(|| size_detail(&p.options));
            (ops.first().map_or("resize", edit_icon), Some(Kind::Image))
        }
        Operation::Annotate => ("annotate", Some(Kind::Image)),
        Operation::CropAspect { w, h } => {
            detail = Some(format!("{w}:{h}"));
            tools = ffmpeg(video);
            ("crop", first_kind)
        }
        Operation::Crop { .. } => {
            tools = ffmpeg(video);
            ("crop", first_kind)
        }
        Operation::Speed { factor } => {
            detail = Some(format!("{factor}×"));
            tools = ffmpeg(true);
            ("speed", first_kind)
        }
        Operation::Trim { .. } => {
            tools = ffmpeg(true);
            ("trim", first_kind)
        }
        Operation::Compress { .. } => {
            tools = ffmpeg(media);
            ("compress", first_kind)
        }
        Operation::Mute => {
            tools = ffmpeg(true);
            ("mute", Some(Kind::Video))
        }
        Operation::MergePdf => ("merge", Some(Kind::Pdf)),
        Operation::SplitPdf => ("split", Some(Kind::Pdf)),
        Operation::CopyTo { move_files, .. } => (if *move_files { "move" } else { "copy" }, None),
        Operation::Zip => ("zip", None),
        Operation::Extract => ("extract", None),
        Operation::CopyPath => ("copyPath", None),
        Operation::OpenTerminal => ("openTerminal", None),
        Operation::Float => ("float", Some(Kind::Image)),
        Operation::CopyText => ("copyText", Some(Kind::Image)),
    };
    WheelSlot {
        id: p.id.clone(),
        name: p.name.clone(),
        format,
        detail,
        icon: icon.to_string(),
        kind,
        needs_tools: tools.into_iter().map(String::from).collect(),
        children: Vec::new(),
    }
}

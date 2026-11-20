//! Clipboard history store (v2).
//! Layout: `<dir>/history.json` (items newest first + folders) + `<dir>/img/<id>.png`
//! (+ `<id>.thumb.png`, longest side 256; edited copies `<id>.edit.png` / `<id>.edit.thumb.png`).
//! Writes are atomic (temp file + rename). Text over 1 MB and blank text are not stored.
//! Dedupe: adding content whose hash equals an existing item moves that item to the top
//! (keeps its id, pin, tags...) and restores it when it was in the trash.
//! A v1 index (no folders/tags/...) loads as is: every new field has a serde default.

use super::{ClipKind, ClipQuery, ClipStats, Field, Folder, Retention};
use crate::{Error, Result};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub const MAX_TEXT_BYTES: usize = 1024 * 1024;
const THUMB_SIDE: u32 = 256;
const DAY_MS: u64 = 86_400_000;
/// Trashed items are deleted for good after this long.
const TRASH_DAYS: u64 = 30;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipItem {
    pub id: u64,
    pub kind: ClipKind,
    /// Text or URL for text kinds.
    #[serde(default)]
    pub text: Option<String>,
    /// Absolute paths, image kind only.
    #[serde(default)]
    pub image: Option<PathBuf>,
    #[serde(default)]
    pub thumb: Option<PathBuf>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    /// Site name for media links.
    #[serde(default)]
    pub site: Option<String>,
    /// Hex of a 64-bit content hash (FNV-1a over text bytes or PNG bytes).
    pub hash: String,
    pub created_at: u64,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub folder: Option<u64>,
    /// Text recognized in an image (`Some("")` = recognized, nothing found).
    #[serde(default)]
    pub ocr: Option<String>,
    #[serde(default)]
    pub trashed_at: Option<u64>,
    #[serde(default)]
    pub source_app: Option<String>,
    #[serde(default)]
    pub link_title: Option<String>,
    /// Edited copy of an image; the original `image` is kept.
    #[serde(default)]
    pub edited: Option<PathBuf>,
    #[serde(default)]
    pub edited_thumb: Option<PathBuf>,
    /// Bytes of the text or the original image file.
    #[serde(default)]
    pub size: u64,
}

impl ClipItem {
    /// Pinned, in a folder, tagged, titled or noted: never expires.
    pub fn saved(&self) -> bool {
        self.pinned
            || self.folder.is_some()
            || !self.tags.is_empty()
            || self.title.is_some()
            || self.note.is_some()
    }

    pub fn shelf(&self) -> super::Shelf {
        match self.kind {
            ClipKind::Image => super::Shelf::Images,
            ClipKind::Text => super::Shelf::Texts,
            ClipKind::Link | ClipKind::MediaLink => super::Shelf::Links,
        }
    }
}

/// `saved` is computed, so it is written but never read back.
impl Serialize for ClipItem {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let mut st = s.serialize_struct("ClipItem", 23)?;
        st.serialize_field("id", &self.id)?;
        st.serialize_field("kind", &self.kind)?;
        st.serialize_field("text", &self.text)?;
        st.serialize_field("image", &self.image)?;
        st.serialize_field("thumb", &self.thumb)?;
        st.serialize_field("width", &self.width)?;
        st.serialize_field("height", &self.height)?;
        st.serialize_field("site", &self.site)?;
        st.serialize_field("hash", &self.hash)?;
        st.serialize_field("createdAt", &self.created_at)?;
        st.serialize_field("pinned", &self.pinned)?;
        st.serialize_field("title", &self.title)?;
        st.serialize_field("note", &self.note)?;
        st.serialize_field("tags", &self.tags)?;
        st.serialize_field("folder", &self.folder)?;
        st.serialize_field("ocr", &self.ocr)?;
        st.serialize_field("trashedAt", &self.trashed_at)?;
        st.serialize_field("sourceApp", &self.source_app)?;
        st.serialize_field("linkTitle", &self.link_title)?;
        st.serialize_field("edited", &self.edited)?;
        st.serialize_field("editedThumb", &self.edited_thumb)?;
        st.serialize_field("saved", &self.saved())?;
        st.serialize_field("size", &self.size)?;
        st.end()
    }
}

#[derive(Serialize, Deserialize)]
struct Index {
    items: Vec<ClipItem>,
    #[serde(default)]
    folders: Vec<Folder>,
}

pub struct ClipStore {
    pub(super) dir: PathBuf,
    pub(super) items: Vec<ClipItem>,
    pub(super) folders: Vec<Folder>,
    next_id: u64,
}

fn fnv1a(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

/// Cheap guard so ordinary prose is never run through the URL classifier:
/// a URL has no whitespace and contains a dot or a scheme separator.
fn may_be_url(text: &str) -> bool {
    let t = text.trim();
    t.len() < 2048 && !t.chars().any(char::is_whitespace) && (t.contains('.') || t.contains("://"))
}

/// Kind and site name of copied text.
fn classify(text: &str) -> (ClipKind, Option<String>) {
    if !may_be_url(text) {
        return (ClipKind::Text, None);
    }
    let kind = super::classify_text(text.trim());
    if kind == ClipKind::Text {
        return (kind, None);
    }
    let site = crate::download::classify_url(text.trim()).map(|i| {
        i.site
            .map(String::from)
            .unwrap_or_else(|| i.host.trim_start_matches("www.").to_string())
    });
    (kind, site)
}

/// Lowercase with the Turkish dotted/dotless I folded together (`İ I ı i` all become `i`).
pub(super) fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            'İ' | 'I' | 'ı' | 'i' => out.push('i'),
            _ => out.extend(c.to_lowercase()),
        }
    }
    out
}

fn no_item() -> Error {
    Error::Failed("no such item".into())
}

fn thumbnail(img: image::DynamicImage) -> image::DynamicImage {
    if img.width().max(img.height()) > THUMB_SIDE {
        img.resize(
            THUMB_SIDE,
            THUMB_SIDE,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        img
    }
}

/// Write `png` to `image_path` and its thumbnail to `thumb_path`; nothing is left behind on error.
fn write_png_pair(png: &[u8], image_path: &Path, thumb_path: &Path) -> Result<()> {
    // Decode first so a bad payload never leaves files behind.
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .map_err(|e| Error::Failed(e.to_string()))?;
    let thumb = thumbnail(img);
    let written = std::fs::write(image_path, png)
        .map_err(Error::from)
        .and_then(|()| {
            thumb
                .save_with_format(thumb_path, image::ImageFormat::Png)
                .map_err(|e| Error::Failed(e.to_string()))
        });
    if written.is_err() {
        let _ = std::fs::remove_file(image_path);
        let _ = std::fs::remove_file(thumb_path);
    }
    written
}

fn delete_files(item: &ClipItem) {
    for p in [&item.image, &item.thumb, &item.edited, &item.edited_thumb]
        .into_iter()
        .flatten()
    {
        let _ = std::fs::remove_file(p);
    }
}

impl ClipStore {
    fn index_path(&self) -> PathBuf {
        self.dir.join("history.json")
    }
    fn img_dir(&self) -> PathBuf {
        self.dir.join("img")
    }

    /// Load or create. A corrupt index is renamed to `history.json.bak` and a fresh store starts.
    /// A v1 index is migrated in memory (sizes filled in); it is rewritten on the next change.
    pub fn open(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir.join("img"))?;
        let index = dir.join("history.json");
        let mut items = Vec::new();
        let mut folders = Vec::new();
        if index.exists() {
            let parsed = std::fs::read(&index)
                .ok()
                .and_then(|b| serde_json::from_slice::<Index>(&b).ok());
            match parsed {
                Some(i) => {
                    items = i.items;
                    folders = i.folders;
                }
                None => {
                    let bak = dir.join("history.json.bak");
                    let _ = std::fs::remove_file(&bak);
                    let _ = std::fs::rename(&index, &bak);
                }
            }
        }
        let known: HashSet<u64> = folders.iter().map(|f| f.id).collect();
        for item in &mut items {
            if item.size == 0 {
                item.size = match (&item.text, &item.image) {
                    (Some(t), _) => t.len() as u64,
                    (None, Some(p)) => std::fs::metadata(p).map(|m| m.len()).unwrap_or(0),
                    _ => 0,
                };
            }
            if item.folder.is_some_and(|f| !known.contains(&f)) {
                item.folder = None;
            }
        }
        let next_id = items.iter().map(|i| i.id).max().unwrap_or(0) + 1;
        Ok(Self {
            dir: dir.to_path_buf(),
            items,
            folders,
            next_id,
        })
    }

    pub(super) fn save(&self) -> Result<()> {
        let bytes = serde_json::to_vec(&Index {
            items: self.items.clone(),
            folders: self.folders.clone(),
        })
        .map_err(|e| Error::Failed(e.to_string()))?;
        write_atomic(&self.index_path(), &bytes)
    }

    /// Every stored item, newest first, trashed ones included. Use [`ClipStore::query`] for views.
    pub fn items(&self) -> &[ClipItem] {
        &self.items
    }

    pub fn get(&self, id: u64) -> Option<&ClipItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub(super) fn item_mut(&mut self, id: u64) -> Result<&mut ClipItem> {
        self.items
            .iter_mut()
            .find(|i| i.id == id)
            .ok_or_else(no_item)
    }

    /// Apply `f` to every item in `ids` (unknown ids are skipped), save once, return how many.
    pub(super) fn update_many(
        &mut self,
        ids: &[u64],
        mut f: impl FnMut(&mut ClipItem) -> bool,
    ) -> Result<usize> {
        let mut n = 0;
        for item in self.items.iter_mut().filter(|i| ids.contains(&i.id)) {
            if f(item) {
                n += 1;
            }
        }
        if n > 0 {
            self.save()?;
        }
        Ok(n)
    }

    /// If an item with this hash exists, move it to the top (keeps id, pin, tags) and restore it
    /// from the trash.
    fn bump(&mut self, hash: &str, is_image: bool, now: u64) -> Result<Option<ClipItem>> {
        let Some(pos) = self
            .items
            .iter()
            .position(|i| i.hash == hash && (i.kind == ClipKind::Image) == is_image)
        else {
            return Ok(None);
        };
        let mut item = self.items.remove(pos);
        item.created_at = now;
        item.trashed_at = None;
        self.items.insert(0, item.clone());
        self.save()?;
        Ok(Some(item))
    }

    fn blank_item(&mut self, kind: ClipKind, hash: String, now: u64) -> ClipItem {
        let id = self.next_id;
        self.next_id += 1;
        ClipItem {
            id,
            kind,
            text: None,
            image: None,
            thumb: None,
            width: None,
            height: None,
            site: None,
            hash,
            created_at: now,
            pinned: false,
            title: None,
            note: None,
            tags: Vec::new(),
            folder: None,
            ocr: None,
            trashed_at: None,
            source_app: None,
            link_title: None,
            edited: None,
            edited_thumb: None,
            size: 0,
        }
    }

    /// Returns the added (or moved-to-top) item, `None` if skipped (blank/oversized).
    pub fn add_text(&mut self, text: &str, now: u64) -> Result<Option<ClipItem>> {
        if text.trim().is_empty() || text.len() > MAX_TEXT_BYTES {
            return Ok(None);
        }
        let hash = fnv1a(text.as_bytes());
        if let Some(item) = self.bump(&hash, false, now)? {
            return Ok(Some(item));
        }
        let (kind, site) = classify(text);
        let mut item = self.blank_item(kind, hash, now);
        item.text = Some(text.to_string());
        item.site = site;
        item.size = text.len() as u64;
        self.items.insert(0, item.clone());
        self.save()?;
        Ok(Some(item))
    }

    /// `png` = encoded PNG bytes.
    pub fn add_image_png(
        &mut self,
        png: &[u8],
        width: u32,
        height: u32,
        now: u64,
    ) -> Result<Option<ClipItem>> {
        if png.is_empty() {
            return Ok(None);
        }
        let hash = fnv1a(png);
        if let Some(item) = self.bump(&hash, true, now)? {
            return Ok(Some(item));
        }
        std::fs::create_dir_all(self.img_dir())?;
        let mut item = self.blank_item(ClipKind::Image, hash, now);
        let image_path = self.img_dir().join(format!("{}.png", item.id));
        let thumb_path = self.img_dir().join(format!("{}.thumb.png", item.id));
        if let Err(e) = write_png_pair(png, &image_path, &thumb_path) {
            self.next_id -= 1;
            return Err(e);
        }
        item.image = Some(image_path);
        item.thumb = Some(thumb_path);
        item.width = Some(width);
        item.height = Some(height);
        item.size = png.len() as u64;
        self.items.insert(0, item.clone());
        self.save()?;
        Ok(Some(item))
    }

    pub fn set_source_app(&mut self, id: u64, app: Option<String>) -> Result<()> {
        self.item_mut(id)?.source_app = app;
        self.save()
    }

    /// Matching items, newest first (see [`ClipQuery`]). Search is case-insensitive over text,
    /// title, note, tags, OCR text, link title and site, with Turkish `İ/I/ı/i` folded together.
    pub fn query(&self, q: &ClipQuery) -> Vec<ClipItem> {
        let needle = q
            .text
            .as_deref()
            .map(|t| fold(t.trim()))
            .filter(|t| !t.is_empty());
        let tag = q.tag.as_deref().map(|t| fold(t.trim()));
        let hit = |i: &ClipItem| {
            (i.trashed_at.is_some() == q.trash)
                && q.shelf.is_none_or(|s| s.matches(i.kind))
                && (!q.pinned || i.pinned)
                && (!q.saved || i.saved())
                && q.folder.is_none_or(|f| i.folder == Some(f))
                && tag
                    .as_deref()
                    .is_none_or(|t| i.tags.iter().any(|x| fold(x) == t))
                && needle.as_deref().is_none_or(|n| matches_text(i, n))
        };
        self.items
            .iter()
            .filter(|i| hit(i))
            .take(q.limit.unwrap_or(usize::MAX))
            .cloned()
            .collect()
    }

    /// Returns how many items were found.
    pub fn set_pinned(&mut self, ids: &[u64], pinned: bool) -> Result<usize> {
        self.update_many(ids, |i| {
            i.pinned = pinned;
            true
        })
    }

    /// Title and note: empty clears. Text: text kinds only, re-hashed and re-classified.
    pub fn set_field(&mut self, id: u64, field: Field, value: &str) -> Result<ClipItem> {
        let item = self.item_mut(id)?;
        let trimmed = value.trim();
        match field {
            Field::Title => item.title = Some(trimmed.to_string()).filter(|t| !t.is_empty()),
            Field::Note => item.note = Some(trimmed.to_string()).filter(|t| !t.is_empty()),
            Field::Text => {
                if item.kind == ClipKind::Image {
                    return Err(Error::Failed("not a text item".into()));
                }
                if trimmed.is_empty() || value.len() > MAX_TEXT_BYTES {
                    return Err(Error::Failed("text is empty or too large".into()));
                }
                let (kind, site) = classify(value);
                item.kind = kind;
                item.site = site;
                item.hash = fnv1a(value.as_bytes());
                item.size = value.len() as u64;
                item.text = Some(value.to_string());
                item.link_title = None;
            }
        }
        let out = item.clone();
        self.save()?;
        Ok(out)
    }

    pub fn set_ocr(&mut self, id: u64, text: String) -> Result<()> {
        self.item_mut(id)?.ocr = Some(text);
        self.save()
    }

    pub fn set_link_title(&mut self, id: u64, title: String) -> Result<()> {
        self.item_mut(id)?.link_title = Some(title);
        self.save()
    }

    /// Live images that were not run through OCR yet, newest first.
    pub fn needs_ocr(&self) -> Vec<(u64, PathBuf)> {
        self.items
            .iter()
            .filter(|i| i.kind == ClipKind::Image && i.trashed_at.is_none() && i.ocr.is_none())
            .filter_map(|i| Some((i.id, i.image.clone()?)))
            .collect()
    }

    // ------------------------------------------------------------ edits

    /// Store an edited copy of an image (`img/<id>.edit.png` + thumbnail); the original stays.
    pub fn save_edited(&mut self, id: u64, png: &[u8]) -> Result<ClipItem> {
        let item = self.get(id).ok_or_else(no_item)?;
        if item.kind != ClipKind::Image {
            return Err(Error::Failed("not an image".into()));
        }
        let path = self.img_dir().join(format!("{id}.edit.png"));
        let thumb = self.img_dir().join(format!("{id}.edit.thumb.png"));
        write_png_pair(png, &path, &thumb)?;
        let item = self.item_mut(id)?;
        item.edited = Some(path);
        item.edited_thumb = Some(thumb);
        let out = item.clone();
        self.save()?;
        Ok(out)
    }

    /// Drop the edited copy; the original is shown again.
    pub fn revert_edit(&mut self, id: u64) -> Result<ClipItem> {
        let item = self.item_mut(id)?;
        for p in [item.edited.take(), item.edited_thumb.take()]
            .into_iter()
            .flatten()
        {
            let _ = std::fs::remove_file(p);
        }
        let out = item.clone();
        self.save()?;
        Ok(out)
    }

    // ------------------------------------------------------------ trash and retention

    /// Move to the trash (hidden from the default query). Returns how many changed.
    pub fn trash(&mut self, ids: &[u64], now: u64) -> Result<usize> {
        self.update_many(ids, |i| {
            if i.trashed_at.is_some() {
                return false;
            }
            i.trashed_at = Some(now);
            true
        })
    }

    pub fn restore(&mut self, ids: &[u64]) -> Result<usize> {
        self.update_many(ids, |i| i.trashed_at.take().is_some())
    }

    /// Remove items and their image files for good (trashed or not).
    pub fn delete_forever(&mut self, ids: &[u64]) -> Result<usize> {
        self.drop_where(|i| ids.contains(&i.id))
    }

    pub fn empty_trash(&mut self) -> Result<usize> {
        self.drop_where(|i| i.trashed_at.is_some())
    }

    /// Trash every live unsaved item; returns how many.
    pub fn clear_unsaved(&mut self, now: u64) -> Result<usize> {
        self.update_many(
            &self
                .items
                .iter()
                .filter(|i| i.trashed_at.is_none() && !i.saved())
                .map(|i| i.id)
                .collect::<Vec<_>>(),
            |i| {
                i.trashed_at = Some(now);
                true
            },
        )
    }

    fn drop_where(&mut self, mut pred: impl FnMut(&ClipItem) -> bool) -> Result<usize> {
        let before = self.items.len();
        self.items.retain(|item| {
            let drop = pred(item);
            if drop {
                delete_files(item);
            }
            !drop
        });
        let n = before - self.items.len();
        if n > 0 {
            self.save()?;
        }
        Ok(n)
    }

    /// Apply retention. Unsaved live items older than their kind's days go for good, then the
    /// oldest unsaved ones beyond `max_items` (0 = no cap); trashed items older than 30 days are
    /// deleted for good. Saved items are never touched. Returns how many items were removed.
    pub fn prune(&mut self, now: u64, ret: &Retention) -> Result<usize> {
        let mut unsaved_kept = 0usize;
        self.drop_where(|i| {
            if let Some(t) = i.trashed_at {
                return now.saturating_sub(t) > TRASH_DAYS * DAY_MS;
            }
            if i.saved() {
                return false;
            }
            let days = u64::from(ret.days_for(i.kind));
            if days > 0 && i.created_at < now.saturating_sub(days * DAY_MS) {
                return true;
            }
            // Items are newest first, so the ones beyond the cap are the oldest.
            unsaved_kept += 1;
            ret.max_items > 0 && unsaved_kept > ret.max_items
        })
    }

    pub fn stats(&self) -> ClipStats {
        let file_len = |p: &Option<PathBuf>| {
            p.as_ref()
                .and_then(|p| std::fs::metadata(p).ok())
                .map_or(0, |m| m.len())
        };
        let trash = self.items.iter().filter(|i| i.trashed_at.is_some()).count();
        ClipStats {
            items: self.items.len() - trash,
            trash,
            bytes: self
                .items
                .iter()
                .map(|i| i.size + file_len(&i.edited))
                .sum(),
        }
    }

    // ------------------------------------------------------------ export

    /// Write items into `dir`: images as `<title or "Görsel <stamp>">.png` (the edited version if
    /// any), texts and links as `.txt`. Names never overwrite (`jobs::naming`). Unknown ids are
    /// skipped; returns the number of files written.
    pub fn export(&self, ids: &[u64], dir: &Path) -> Result<usize> {
        std::fs::create_dir_all(dir)?;
        let mut written = 0;
        for id in ids {
            let Some(item) = self.get(*id) else { continue };
            let stamp = stamp(item.created_at);
            let is_image = item.kind == ClipKind::Image;
            let stem = item
                .title
                .as_deref()
                .and_then(file_stem)
                .or_else(|| item.link_title.as_deref().and_then(file_stem))
                .or_else(|| {
                    let first = item
                        .text
                        .as_deref()?
                        .lines()
                        .find(|l| !l.trim().is_empty())?;
                    file_stem(&first.chars().take(40).collect::<String>())
                })
                .unwrap_or_else(|| {
                    if is_image {
                        format!("Görsel {stamp}")
                    } else {
                        format!("Metin {stamp}")
                    }
                });
            let ext = if is_image { "png" } else { "txt" };
            let out = crate::jobs::naming::output_path(
                &dir.join(format!("{stem}.clip")),
                ext,
                Some(dir),
                None,
            );
            if is_image {
                let Some(src) = item.edited.as_ref().or(item.image.as_ref()) else {
                    continue;
                };
                std::fs::copy(src, &out)?;
            } else {
                std::fs::write(&out, item.text.as_deref().unwrap_or(""))?;
            }
            written += 1;
        }
        Ok(written)
    }
}

fn matches_text(i: &ClipItem, needle: &str) -> bool {
    let hit = |s: &Option<String>| s.as_deref().is_some_and(|s| fold(s).contains(needle));
    hit(&i.text)
        || hit(&i.title)
        || hit(&i.note)
        || hit(&i.ocr)
        || hit(&i.link_title)
        || hit(&i.site)
        || i.tags.iter().any(|t| fold(t).contains(needle))
}

/// A safe file stem from free text: invalid characters replaced, trimmed, at most 80 chars.
fn file_stem(s: &str) -> Option<String> {
    let cleaned: String = s
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .take(80)
        .collect();
    let cleaned = cleaned.trim().trim_end_matches('.').trim().to_string();
    (!cleaned.is_empty()).then_some(cleaned)
}

/// UTC `yyyy-mm-dd HH.MM.SS` of a Unix time in ms (no time-zone data in the core).
fn stamp(ms: u64) -> String {
    let secs = ms / 1000;
    let (days, rem) = ((secs / 86_400) as i64, secs % 86_400);
    // Civil-from-days (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}.{:02}.{:02}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

// ---------------------------------------------------------------- image helpers for the platform layer
// (src-tauri has no `image` dependency, so DIB/PNG conversion lives here).

/// Largest image (in pixels) the clipboard recorder accepts.
pub const MAX_IMAGE_PIXELS: u64 = 25_000_000;

/// Width/height from a PNG's IHDR, `None` if `bytes` is not a PNG.
pub fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || bytes[..8] != [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a] {
        return None;
    }
    let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((w, h))
}

fn le32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// Convert a clipboard DIB (`CF_DIB`/`CF_DIBV5`: info header + pixels, no file header) to PNG.
/// Returns `(png, width, height)`. Images over [`MAX_IMAGE_PIXELS`] are `TooLarge`.
pub fn dib_to_png(dib: &[u8]) -> Result<(Vec<u8>, u32, u32)> {
    let bad = || Error::Failed("unsupported bitmap".into());
    let size = le32(dib, 0).ok_or_else(bad)? as usize;
    if size < 40 || dib.len() < size {
        return Err(bad());
    }
    let w = i64::from(le32(dib, 4).ok_or_else(bad)? as i32).unsigned_abs();
    let h = i64::from(le32(dib, 8).ok_or_else(bad)? as i32).unsigned_abs();
    if w == 0 || h == 0 {
        return Err(bad());
    }
    if w * h > MAX_IMAGE_PIXELS {
        return Err(Error::TooLarge(format!("{w}x{h} image")));
    }
    let bits = u32::from(u16::from_le_bytes([dib[14], dib[15]]));
    let compression = le32(dib, 16).ok_or_else(bad)?;
    let used = le32(dib, 32).ok_or_else(bad)?;
    let masks = match (size, compression) {
        (40, 3) => 12,
        (40, 6) => 16,
        _ => 0,
    };
    let colors = if bits <= 8 && used == 0 {
        1u32 << bits
    } else {
        used
    };
    let offset = 14 + size as u32 + masks + colors.saturating_mul(4);
    let mut bmp = Vec::with_capacity(dib.len() + 14);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&(dib.len() as u32 + 14).to_le_bytes());
    bmp.extend_from_slice(&[0; 4]);
    bmp.extend_from_slice(&offset.to_le_bytes());
    bmp.extend_from_slice(dib);
    let img = image::load_from_memory_with_format(&bmp, image::ImageFormat::Bmp)
        .map_err(|e| Error::Failed(e.to_string()))?;
    drop(bmp);
    let mut rgba = img.into_rgba8();
    // 32-bit BI_RGB bitmaps carry an unused 4th byte that is usually 0: treat that as opaque.
    if bits == 32 && rgba.pixels().all(|p| p.0[3] == 0) {
        for p in rgba.pixels_mut() {
            p.0[3] = 255;
        }
    }
    let (w, h) = rgba.dimensions();
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(rgba)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| Error::Failed(e.to_string()))?;
    Ok((png, w, h))
}

/// PNG bytes -> 32-bit bottom-up `CF_DIB` payload, transparency flattened onto white
/// (the registered "PNG" clipboard format keeps the alpha for apps that read it).
pub fn png_to_dib(png: &[u8]) -> Result<Vec<u8>> {
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .map_err(|e| Error::Failed(e.to_string()))?
        .into_rgba8();
    let (w, h) = img.dimensions();
    let mut out = Vec::with_capacity(40 + (w * h * 4) as usize);
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&[0; 4]); // BI_RGB
    out.extend_from_slice(&(w * h * 4).to_le_bytes());
    out.extend_from_slice(&[0; 16]);
    let flat =
        |c: u8, a: u8| ((u32::from(c) * u32::from(a) + 255 * (255 - u32::from(a))) / 255) as u8;
    for y in (0..h).rev() {
        for x in 0..w {
            let [r, g, b, a] = img.get_pixel(x, y).0;
            out.extend_from_slice(&[flat(b, a), flat(g, a), flat(r, a), 255]);
        }
    }
    Ok(out)
}

/// Re-encode a stored PNG as `to` ("png" | "jpg" | "webp") at `output`.
pub fn export_png(png_path: &Path, to: &str, output: &Path) -> Result<()> {
    if !matches!(to, "png" | "jpg" | "webp") {
        return Err(Error::Unsupported(format!("image output format: {to}")));
    }
    let img = image::open(png_path).map_err(|e| match e {
        image::ImageError::IoError(io) => Error::Io(io),
        other => Error::Failed(other.to_string()),
    })?;
    crate::engines::image::encode(&img, to, output, &crate::jobs::ConvertOptions::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dib_roundtrip_and_png_size() {
        let src = png(5, 3, 77);
        assert_eq!(png_size(&src), Some((5, 3)));
        assert_eq!(png_size(b"nope"), None);
        let dib = png_to_dib(&src).unwrap();
        let (back, w, h) = dib_to_png(&dib).unwrap();
        assert_eq!((w, h), (5, 3));
        let a = image::load_from_memory(&src).unwrap().into_rgba8();
        let b = image::load_from_memory(&back).unwrap().into_rgba8();
        assert_eq!(a.as_raw(), b.as_raw());
    }

    #[test]
    fn dib_zero_alpha_is_opaque_and_oversize_rejected() {
        // 2x1, 32-bit BI_RGB, alpha byte 0.
        let mut dib = Vec::new();
        dib.extend_from_slice(&40u32.to_le_bytes());
        dib.extend_from_slice(&2i32.to_le_bytes());
        dib.extend_from_slice(&1i32.to_le_bytes());
        dib.extend_from_slice(&1u16.to_le_bytes());
        dib.extend_from_slice(&32u16.to_le_bytes());
        dib.extend_from_slice(&[0; 24]);
        dib.extend_from_slice(&[10, 20, 30, 0, 40, 50, 60, 0]);
        let (png, ..) = dib_to_png(&dib).unwrap();
        let img = image::load_from_memory(&png).unwrap().into_rgba8();
        assert_eq!(img.get_pixel(0, 0).0, [30, 20, 10, 255]);
        dib[4..8].copy_from_slice(&6000i32.to_le_bytes());
        dib[8..12].copy_from_slice(&6000i32.to_le_bytes());
        assert!(matches!(dib_to_png(&dib), Err(Error::TooLarge(_))));
        assert!(dib_to_png(&[1, 2, 3]).is_err());
    }

    #[test]
    fn export_png_writes_requested_format() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("a.png");
        std::fs::write(&src, png(20, 10, 3)).unwrap();
        for ext in ["png", "jpg", "webp"] {
            let out = d.path().join(format!("o.{ext}"));
            export_png(&src, ext, &out).unwrap();
            assert!(std::fs::metadata(&out).unwrap().len() > 0);
        }
        assert!(export_png(&src, "gif", &d.path().join("o.gif")).is_err());
    }

    fn png(w: u32, h: u32, shade: u8) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba([shade, 20, 30, 255]));
        let mut out = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    fn store() -> (tempfile::TempDir, ClipStore) {
        let d = tempfile::tempdir().unwrap();
        let s = ClipStore::open(d.path()).unwrap();
        (d, s)
    }

    #[test]
    fn images_get_files_thumbnail_and_dedupe() {
        let (_d, mut s) = store();
        let bytes = png(600, 300, 5);
        let it = s.add_image_png(&bytes, 600, 300, 1).unwrap().unwrap();
        assert_eq!(it.kind, ClipKind::Image);
        assert_eq!(it.size, bytes.len() as u64);
        let thumb = image::open(it.thumb.as_ref().unwrap()).unwrap();
        assert_eq!((thumb.width(), thumb.height()), (256, 128));
        assert!(it.image.as_ref().unwrap().exists());
        let dup = s.add_image_png(&bytes, 600, 300, 9).unwrap().unwrap();
        assert_eq!(dup.id, it.id);
        assert_eq!(s.items().len(), 1);
        assert!(s.add_image_png(b"not a png", 1, 1, 1).is_err());
        assert_eq!(s.items().len(), 1);
    }

    #[test]
    fn persists_across_reopen_and_ids_do_not_collide() {
        let d = tempfile::tempdir().unwrap();
        let id = {
            let mut s = ClipStore::open(d.path()).unwrap();
            s.add_text("one", 1).unwrap();
            s.add_text("two", 2).unwrap().unwrap().id
        };
        let mut s = ClipStore::open(d.path()).unwrap();
        assert_eq!(s.items().len(), 2);
        let three = s.add_text("three", 3).unwrap().unwrap();
        assert!(three.id > id);
    }

    #[test]
    fn corrupt_index_is_backed_up() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("history.json"), b"{ not json").unwrap();
        let mut s = ClipStore::open(d.path()).unwrap();
        assert!(s.items().is_empty());
        assert!(d.path().join("history.json.bak").exists());
        s.add_text("fresh", 1).unwrap();
        assert_eq!(ClipStore::open(d.path()).unwrap().items().len(), 1);
    }

    #[test]
    fn fold_merges_turkish_i_and_stamp_is_utc() {
        assert_eq!(fold("İSTANBUL ıspanak"), "istanbul ispanak");
        assert_eq!(stamp(0), "1970-01-01 00.00.00");
        assert_eq!(stamp(1_700_000_000_000), "2023-11-14 22.13.20");
        assert_eq!(file_stem("a/b:c?. "), Some("a_b_c_".into()));
        assert_eq!(file_stem(" . "), None);
    }
}

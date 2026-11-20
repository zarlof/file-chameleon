//! Clipboard history store. OWNER: Task G.
//! Layout: `<dir>/history.json` (items, newest first) + `<dir>/img/<id>.png` (+ `<id>.thumb.png`,
//! longest side 256). Writes are atomic (temp file + rename). Text items over 1 MB are not stored.
//! Dedupe: adding content whose hash equals an existing item moves that item to the top
//! (keeps its id and pin) instead of adding a duplicate.

use super::ClipKind;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const MAX_TEXT_BYTES: usize = 1024 * 1024;
const THUMB_SIDE: u32 = 256;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipItem {
    pub id: u64,
    pub kind: ClipKind,
    /// Text or URL for text kinds.
    pub text: Option<String>,
    /// Absolute paths, image kind only.
    pub image: Option<PathBuf>,
    pub thumb: Option<PathBuf>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Site name for media links.
    pub site: Option<String>,
    /// Hex of a 64-bit content hash (FNV-1a over text bytes or PNG bytes).
    pub hash: String,
    pub created_at: u64,
    pub pinned: bool,
}

#[derive(Serialize, Deserialize)]
struct Index {
    items: Vec<ClipItem>,
}

pub struct ClipStore {
    dir: PathBuf,
    items: Vec<ClipItem>,
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

impl ClipStore {
    fn index_path(&self) -> PathBuf {
        self.dir.join("history.json")
    }
    fn img_dir(&self) -> PathBuf {
        self.dir.join("img")
    }

    /// Load or create. A corrupt index is renamed to `history.json.bak` and a fresh store starts.
    pub fn open(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir.join("img"))?;
        let index = dir.join("history.json");
        let mut items = Vec::new();
        if index.exists() {
            let parsed = std::fs::read(&index)
                .ok()
                .and_then(|b| serde_json::from_slice::<Index>(&b).ok());
            match parsed {
                Some(i) => items = i.items,
                None => {
                    let bak = dir.join("history.json.bak");
                    let _ = std::fs::remove_file(&bak);
                    let _ = std::fs::rename(&index, &bak);
                }
            }
        }
        let next_id = items.iter().map(|i| i.id).max().unwrap_or(0) + 1;
        Ok(Self {
            dir: dir.to_path_buf(),
            items,
            next_id,
        })
    }

    fn save(&self) -> Result<()> {
        let bytes = serde_json::to_vec(&Index {
            items: self.items.clone(),
        })
        .map_err(|e| Error::Failed(e.to_string()))?;
        write_atomic(&self.index_path(), &bytes)
    }

    /// Newest first.
    pub fn items(&self) -> &[ClipItem] {
        &self.items
    }

    /// If an item with this hash exists, move it to the top (keeps id and pin).
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
        self.items.insert(0, item.clone());
        self.save()?;
        Ok(Some(item))
    }

    /// Returns the added (or moved-to-top) item, `None` if skipped (empty/oversized).
    pub fn add_text(&mut self, text: &str, now: u64) -> Result<Option<ClipItem>> {
        if text.trim().is_empty() || text.len() > MAX_TEXT_BYTES {
            return Ok(None);
        }
        let hash = fnv1a(text.as_bytes());
        if let Some(item) = self.bump(&hash, false, now)? {
            return Ok(Some(item));
        }
        let (kind, site) = if may_be_url(text) {
            let kind = super::classify_text(text);
            let site = if kind == ClipKind::Text {
                None
            } else {
                crate::download::classify_url(text).map(|i| {
                    i.site
                        .map(String::from)
                        .unwrap_or_else(|| i.host.trim_start_matches("www.").to_string())
                })
            };
            (kind, site)
        } else {
            (ClipKind::Text, None)
        };
        let item = ClipItem {
            id: self.next_id,
            kind,
            text: Some(text.to_string()),
            image: None,
            thumb: None,
            width: None,
            height: None,
            site,
            hash,
            created_at: now,
            pinned: false,
        };
        self.next_id += 1;
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
        // Decode first so a bad payload never leaves files behind.
        let img = image::load_from_memory_with_format(png, image::ImageFormat::Png)
            .map_err(|e| Error::Failed(e.to_string()))?;
        let thumb = if img.width().max(img.height()) > THUMB_SIDE {
            img.resize(
                THUMB_SIDE,
                THUMB_SIDE,
                image::imageops::FilterType::Lanczos3,
            )
        } else {
            img
        };
        let id = self.next_id;
        let image_path = self.img_dir().join(format!("{id}.png"));
        let thumb_path = self.img_dir().join(format!("{id}.thumb.png"));
        std::fs::create_dir_all(self.img_dir())?;
        let written = std::fs::write(&image_path, png)
            .map_err(Error::from)
            .and_then(|()| {
                thumb
                    .save_with_format(&thumb_path, image::ImageFormat::Png)
                    .map_err(|e| Error::Failed(e.to_string()))
            });
        if let Err(e) = written {
            let _ = std::fs::remove_file(&image_path);
            let _ = std::fs::remove_file(&thumb_path);
            return Err(e);
        }
        let item = ClipItem {
            id,
            kind: ClipKind::Image,
            text: None,
            image: Some(image_path),
            thumb: Some(thumb_path),
            width: Some(width),
            height: Some(height),
            site: None,
            hash,
            created_at: now,
            pinned: false,
        };
        self.next_id += 1;
        self.items.insert(0, item.clone());
        self.save()?;
        Ok(Some(item))
    }

    /// Case-insensitive substring match on text/site; empty query = all.
    pub fn search(&self, query: &str) -> Vec<ClipItem> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return self.items.clone();
        }
        self.items
            .iter()
            .filter(|i| {
                i.text
                    .as_deref()
                    .is_some_and(|t| t.to_lowercase().contains(&q))
                    || i.site
                        .as_deref()
                        .is_some_and(|s| s.to_lowercase().contains(&q))
            })
            .cloned()
            .collect()
    }

    pub fn set_pinned(&mut self, id: u64, pinned: bool) -> Result<bool> {
        let Some(item) = self.items.iter_mut().find(|i| i.id == id) else {
            return Ok(false);
        };
        item.pinned = pinned;
        self.save()?;
        Ok(true)
    }

    fn delete_files(item: &ClipItem) {
        for p in [&item.image, &item.thumb].into_iter().flatten() {
            let _ = std::fs::remove_file(p);
        }
    }

    /// Deletes the item and its image files.
    pub fn remove(&mut self, id: u64) -> Result<bool> {
        let Some(pos) = self.items.iter().position(|i| i.id == id) else {
            return Ok(false);
        };
        let item = self.items.remove(pos);
        Self::delete_files(&item);
        self.save()?;
        Ok(true)
    }

    fn retain(&mut self, mut keep: impl FnMut(&ClipItem) -> bool) -> Result<()> {
        let before = self.items.len();
        self.items.retain(|item| {
            let k = keep(item);
            if !k {
                Self::delete_files(item);
            }
            k
        });
        if self.items.len() != before {
            self.save()?;
        }
        Ok(())
    }

    /// Remove all unpinned items.
    pub fn clear_unpinned(&mut self) -> Result<()> {
        self.retain(|i| i.pinned)
    }

    /// Drop unpinned items older than `retention_days` (0 = no age limit) and beyond `max_items`
    /// (pinned items are never dropped and do not count towards the limit).
    pub fn prune(&mut self, now: u64, retention_days: u32, max_items: usize) -> Result<()> {
        let cutoff = if retention_days == 0 {
            0
        } else {
            now.saturating_sub(u64::from(retention_days) * 86_400_000)
        };
        let mut unpinned_kept = 0usize;
        self.retain(|i| {
            if i.pinned {
                return true;
            }
            if i.created_at < cutoff || unpinned_kept >= max_items {
                return false;
            }
            unpinned_kept += 1;
            true
        })
    }
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
    fn add_dedupe_moves_to_top_and_keeps_pin() {
        let (_d, mut s) = store();
        let a = s.add_text("alpha", 1).unwrap().unwrap();
        s.add_text("beta", 2).unwrap().unwrap();
        s.set_pinned(a.id, true).unwrap();
        let again = s.add_text("alpha", 3).unwrap().unwrap();
        assert_eq!(again.id, a.id);
        assert!(again.pinned);
        assert_eq!(again.created_at, 3);
        assert_eq!(s.items().len(), 2);
        assert_eq!(s.items()[0].id, a.id);
    }

    #[test]
    fn skips_empty_and_oversized_text() {
        let (_d, mut s) = store();
        assert!(s.add_text("   \n", 1).unwrap().is_none());
        let big = "x".repeat(MAX_TEXT_BYTES + 1);
        assert!(s.add_text(&big, 1).unwrap().is_none());
        assert!(s.items().is_empty());
        let max = "x".repeat(MAX_TEXT_BYTES);
        assert!(s.add_text(&max, 1).unwrap().is_some());
    }

    #[test]
    fn search_matches_text_and_site_case_insensitively() {
        let (_d, mut s) = store();
        s.add_text("other", 1).unwrap();
        s.add_text("Hello World", 2).unwrap();
        s.items[1].site = Some("YouTube".into());
        assert_eq!(s.search("hello").len(), 1);
        assert_eq!(s.search("YOUTUBE").len(), 1);
        assert_eq!(s.search("").len(), 2);
        assert!(s.search("zzz").is_empty());
    }

    #[test]
    fn images_get_files_thumbnail_and_dedupe() {
        let (_d, mut s) = store();
        let bytes = png(600, 300, 5);
        let it = s.add_image_png(&bytes, 600, 300, 1).unwrap().unwrap();
        assert_eq!(it.kind, ClipKind::Image);
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
    fn remove_deletes_files() {
        let (_d, mut s) = store();
        let it = s
            .add_image_png(&png(10, 10, 1), 10, 10, 1)
            .unwrap()
            .unwrap();
        let (i, t) = (it.image.clone().unwrap(), it.thumb.clone().unwrap());
        assert!(s.remove(it.id).unwrap());
        assert!(!i.exists() && !t.exists());
        assert!(!s.remove(it.id).unwrap());
    }

    #[test]
    fn clear_unpinned_keeps_pinned() {
        let (_d, mut s) = store();
        let a = s.add_text("a", 1).unwrap().unwrap();
        s.add_text("b", 2).unwrap();
        let img = s.add_image_png(&png(8, 8, 2), 8, 8, 3).unwrap().unwrap();
        s.set_pinned(a.id, true).unwrap();
        s.clear_unpinned().unwrap();
        assert_eq!(s.items().len(), 1);
        assert_eq!(s.items()[0].id, a.id);
        assert!(!img.image.unwrap().exists());
    }

    #[test]
    fn prune_by_age_and_count_keeps_pinned() {
        let (_d, mut s) = store();
        let day = 86_400_000u64;
        let old = s.add_text("old", 0).unwrap().unwrap();
        s.set_pinned(old.id, true).unwrap();
        s.add_text("old2", 1).unwrap();
        for n in 0..5u64 {
            s.add_text(&format!("new{n}"), 10 * day + n).unwrap();
        }
        s.prune(10 * day + 10, 7, 3).unwrap();
        let texts: Vec<_> = s.items().iter().map(|i| i.text.clone().unwrap()).collect();
        assert_eq!(texts, ["new4", "new3", "new2", "old"]);
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
}

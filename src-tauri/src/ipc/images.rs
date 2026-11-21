//! Small image I/O for the wheel and the image editor (Task 6): the wheel's file-chip
//! thumbnail, the editor's downscaled preview, and saving an annotated copy. Images cross IPC
//! as PNG data URLs (`fc_core::b64`); nothing decoded is kept after a call.

use super::{CmdError, CmdResult, blocking};
use crate::state::AppState;
use fc_core::formats::{self, Kind};
use fc_core::jobs::naming;
use image::codecs::png::{CompressionType, FilterType as PngFilter, PngEncoder};
use image::imageops::FilterType;
use image::{DynamicImage, ImageDecoder, ImageReader};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

/// Longest side of the wheel thumbnail.
pub const THUMB_SIDE: u32 = 96;
/// Larger files get no thumbnail (decoding them would delay it past the drop).
pub const THUMB_MAX_BYTES: u64 = 30 * 1024 * 1024;
/// ... and so do images above this many pixels (a small file can still decode to gigabytes).
pub const THUMB_MAX_PIXELS: u64 = 25_000_000;
/// `read_image_preview` never returns a larger side than this (the editor asks for it and
/// warns when an image was reduced to it).
pub const PREVIEW_MAX_SIDE: u32 = 8192;

const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

fn image_error(e: image::ImageError) -> fc_core::Error {
    match e {
        image::ImageError::IoError(io) => fc_core::Error::Io(io),
        other => fc_core::Error::Unsupported(other.to_string()),
    }
}

/// Decode `path` (format from its content), upright according to its EXIF orientation.
fn open_image(path: &Path) -> fc_core::Result<DynamicImage> {
    if !path.is_file() {
        return Err(fc_core::Error::NotFound(path.to_path_buf()));
    }
    let mut decoder = ImageReader::open(path)?
        .with_guessed_format()?
        .into_decoder()
        .map_err(image_error)?;
    let orientation = decoder.orientation().ok();
    let mut img = DynamicImage::from_decoder(decoder).map_err(image_error)?;
    if let Some(o) = orientation {
        img.apply_orientation(o);
    }
    Ok(img)
}

/// `data:image/png;base64,...` of `img` (8-bit RGBA, fast compression: it is only shown).
fn png_data_url(img: &DynamicImage) -> fc_core::Result<String> {
    let mut png = Vec::new();
    let enc = PngEncoder::new_with_quality(&mut png, CompressionType::Fast, PngFilter::Adaptive);
    img.to_rgba8()
        .write_with_encoder(enc)
        .map_err(image_error)?;
    Ok(format!(
        "data:image/png;base64,{}",
        fc_core::b64::encode(&png)
    ))
}

/// Downscaled copy whose longest side is at most `max_side` (never upscaled).
fn preview(path: &Path, max_side: u32) -> fc_core::Result<String> {
    let max = max_side.clamp(1, PREVIEW_MAX_SIDE);
    let img = open_image(path)?;
    let img = if img.width().max(img.height()) > max {
        img.resize(max, max, FilterType::Triangle)
    } else {
        img
    };
    png_data_url(&img)
}

/// Thumbnail for the wheel's file chip: only for raster images up to `THUMB_MAX_BYTES`.
pub fn wheel_thumbnail(path: &str) -> Option<String> {
    let path = Path::new(path);
    if formats::detect_path(path).map(|f| f.kind) != Some(Kind::Image) {
        return None;
    }
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > THUMB_MAX_BYTES {
        return None;
    }
    // Header only: skip huge images before anything is decoded.
    let (w, h) = ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()?;
    if u64::from(w) * u64::from(h) > THUMB_MAX_PIXELS {
        return None;
    }
    let img = open_image(path).ok()?;
    png_data_url(&img.thumbnail(THUMB_SIDE, THUMB_SIDE)).ok()
}

/// Editor: the image at `path` as a PNG data URL, longest side ≤ `max_side` (≤ 8192).
/// Async: decodes and encodes the image.
#[tauri::command]
pub async fn read_image_preview(path: String, max_side: u32) -> CmdResult<String> {
    blocking(move || preview(Path::new(&path), max_side).map_err(CmdError::from)).await?
}

/// " (düzenlendi)" / " (edited)".
fn edited_suffix(lang: &str) -> &'static str {
    if lang == "tr" {
        " (düzenlendi)"
    } else {
        " (edited)"
    }
}

/// Write the PNG `png` as `<stem><suffix>.png` in `dir` (or next to `source`), numbered when
/// taken. The source is never touched; a failed write leaves nothing behind.
fn write_edited(
    source: &Path,
    dir: Option<&Path>,
    suffix: &str,
    png: &[u8],
) -> fc_core::Result<PathBuf> {
    if !png.starts_with(PNG_SIGNATURE) {
        return Err(fc_core::Error::Invalid("edited image is not a PNG".into()));
    }
    if let Some(d) = dir {
        std::fs::create_dir_all(d)?;
    }
    let target = naming::output_path(source, "png", dir, Some(suffix));
    let tmp = naming::temp_path(&target);
    let written = std::fs::write(&tmp, png).and_then(|()| naming::commit(&tmp, &target));
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    Ok(written?)
}

/// Editor: save an edited image (PNG data URL) as "<name> (düzenlendi).png" (English UI:
/// "(edited)") next to `path`; returns the new path. Images from the clipboard store are saved
/// to the Downloads folder instead ("Clipboard <date> (edited).png"), never into the store.
/// Async: writes a file.
#[tauri::command]
pub async fn save_annotated(app: AppHandle, path: String, data_url: String) -> CmdResult<String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let png = fc_core::b64::decode(&data_url)?;
        let lang = crate::platform::ui_language(&state.settings().language);
        let source = PathBuf::from(&path);
        let (source, dir) = if state.paths.is_clipboard_file(&source) {
            let dir = state.default_download_dir();
            let name = format!("Clipboard {}.png", super::clipboard::local_stamp());
            (dir.join(name), Some(dir))
        } else {
            (source, None)
        };
        let out = write_edited(&source, dir.as_deref(), edited_suffix(&lang), &png)?;
        Ok(out.to_string_lossy().into_owned())
    })
    .await?
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh, empty folder under the system temp dir (removed by the test).
    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("fc-images-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn png_bytes(w: u32, h: u32) -> Vec<u8> {
        let img = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            w,
            h,
            image::Rgba([10, 200, 30, 255]),
        ));
        let mut out = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    fn decode_url(url: &str) -> DynamicImage {
        assert!(url.starts_with("data:image/png;base64,"));
        image::load_from_memory(&fc_core::b64::decode(url).unwrap()).unwrap()
    }

    #[test]
    fn preview_fits_the_longest_side_and_never_upscales() {
        let d = temp_dir("preview");
        let p = d.join("wide.png");
        std::fs::write(&p, png_bytes(300, 100)).unwrap();
        let small = decode_url(&preview(&p, 60).unwrap());
        assert_eq!((small.width(), small.height()), (60, 20));
        let same = decode_url(&preview(&p, 4096).unwrap());
        assert_eq!((same.width(), same.height()), (300, 100));
        // 0 and huge values are clamped instead of failing.
        assert_eq!(decode_url(&preview(&p, 0).unwrap()).width(), 1);
        assert_eq!(decode_url(&preview(&p, u32::MAX).unwrap()).width(), 300);
        // The cap is 8192 (the editor asks for it and warns when it was reached).
        let big = d.join("big.png");
        std::fs::write(&big, png_bytes(8200, 2)).unwrap();
        assert_eq!(decode_url(&preview(&big, u32::MAX).unwrap()).width(), 8192);
        assert_eq!(decode_url(&preview(&big, 8192).unwrap()).width(), 8192);
        assert_eq!(decode_url(&preview(&big, 5000).unwrap()).width(), 5000);
        assert!(matches!(
            preview(&d.join("missing.png"), 64),
            Err(fc_core::Error::NotFound(_))
        ));
        std::fs::write(d.join("broken.png"), b"not an image").unwrap();
        assert!(preview(&d.join("broken.png"), 64).is_err());

        let thumb = wheel_thumbnail(p.to_str().unwrap()).unwrap();
        assert_eq!(decode_url(&thumb).width(), THUMB_SIDE);
        // Over the pixel cap the header is enough to say no (a small file can still be huge).
        let huge = d.join("huge.png");
        std::fs::write(&huge, png_bytes(5100, 5000)).unwrap();
        assert!(wheel_thumbnail(huge.to_str().unwrap()).is_none());
        // Not an image by extension: no thumbnail.
        std::fs::write(d.join("a.txt"), b"hello").unwrap();
        assert!(wheel_thumbnail(d.join("a.txt").to_str().unwrap()).is_none());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn edited_copies_are_numbered_and_the_source_is_kept() {
        let d = temp_dir("edited");
        let src = d.join("photo.jpg");
        std::fs::write(&src, b"original").unwrap();
        let png = png_bytes(4, 4);
        let a = write_edited(&src, None, edited_suffix("tr"), &png).unwrap();
        assert_eq!(a, d.join("photo (düzenlendi).png"));
        let b = write_edited(&src, None, edited_suffix("tr"), &png).unwrap();
        assert_eq!(b, d.join("photo (düzenlendi) (1).png"));
        let c = write_edited(&src, None, edited_suffix("en"), &png).unwrap();
        assert_eq!(c, d.join("photo (edited).png"));
        assert_eq!(std::fs::read(&a).unwrap(), png);
        assert_eq!(std::fs::read(&src).unwrap(), b"original");
        // Another folder (clipboard images go to Downloads).
        let out = d.join("out");
        let e = write_edited(&src, Some(&out), " (edited)", &png).unwrap();
        assert_eq!(e, out.join("photo (edited).png"));
        // Not a PNG: rejected, nothing written.
        assert!(matches!(
            write_edited(&src, None, " (edited)", b"GIF89a"),
            Err(fc_core::Error::Invalid(_))
        ));
        assert_eq!(std::fs::read_dir(&d).unwrap().count(), 5);
        std::fs::remove_dir_all(&d).unwrap();
    }
}

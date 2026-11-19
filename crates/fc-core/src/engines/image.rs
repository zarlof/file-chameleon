//! Raster image engine (`image` crate; lossy WebP via `webp`). OWNER: Task B.
//! Limits: refuse inputs over 200 megapixels (`Error::TooLarge`) before decoding pixels
//! (read dimensions first). GIF input decodes the first frame only.

use super::quality_or;
use crate::jobs::{CompressLevel, ConvertOptions};
use crate::{CancelToken, Error, Result};
use image::codecs::gif::GifEncoder;
use image::codecs::ico::{IcoEncoder, IcoFrame};
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType as PngFilter, PngEncoder};
use image::codecs::tiff::TiffEncoder;
use image::imageops::FilterType;
use image::{
    DynamicImage, ExtendedColorType, ImageDecoder, ImageEncoder, ImageReader, RgbImage, RgbaImage,
};
use std::borrow::Cow;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

pub const MAX_PIXELS: u64 = 200_000_000;

const ICO_SIZES: [u32; 6] = [16, 32, 48, 64, 128, 256];

pub(crate) fn map_image_error(e: image::ImageError) -> Error {
    match e {
        image::ImageError::Unsupported(u) => Error::Unsupported(u.to_string()),
        image::ImageError::IoError(io) => Error::Io(io),
        other => Error::Failed(other.to_string()),
    }
}

fn open_error(path: &Path, e: std::io::Error) -> Error {
    if e.kind() == std::io::ErrorKind::NotFound {
        Error::NotFound(path.to_path_buf())
    } else {
        Error::Io(e)
    }
}

/// Refuse images over [`MAX_PIXELS`].
pub(crate) fn check_dimensions(width: u32, height: u32) -> Result<()> {
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(Error::TooLarge(format!(
            "image is {width}x{height}, limit is 200 megapixels"
        )));
    }
    Ok(())
}

/// Decode `path` (format guessed from content) with the dimension guard applied before any
/// pixel is decoded. EXIF orientation is applied.
pub(crate) fn decode(path: &Path) -> Result<DynamicImage> {
    let mut reader = ImageReader::open(path)
        .map_err(|e| open_error(path, e))?
        .with_guessed_format()?;
    reader.no_limits();
    let mut decoder = reader.into_decoder().map_err(map_image_error)?;
    let (w, h) = decoder.dimensions();
    check_dimensions(w, h)?;
    let orientation = decoder.orientation().ok();
    let mut img = DynamicImage::from_decoder(decoder).map_err(map_image_error)?;
    if let Some(o) = orientation {
        img.apply_orientation(o);
    }
    Ok(img)
}

/// Target size when fitting `(w, h)` into the bounds. Never upscales.
fn fit_size(w: u32, h: u32, max_w: Option<u32>, max_h: Option<u32>) -> (u32, u32) {
    let mut scale = 1.0_f64;
    if let Some(mw) = max_w.filter(|v| *v > 0) {
        scale = scale.min(f64::from(mw) / f64::from(w));
    }
    if let Some(mh) = max_h.filter(|v| *v > 0) {
        scale = scale.min(f64::from(mh) / f64::from(h));
    }
    if scale >= 1.0 {
        return (w, h);
    }
    let nw = ((f64::from(w) * scale).round() as u32).max(1);
    let nh = ((f64::from(h) * scale).round() as u32).max(1);
    (nw, nh)
}

/// Size after applying `max_width`/`max_height` (downscale only).
pub(crate) fn fit_dimensions(w: u32, h: u32, options: &ConvertOptions) -> (u32, u32) {
    fit_size(w, h, options.max_width, options.max_height)
}

fn fit_owned(img: DynamicImage, options: &ConvertOptions) -> DynamicImage {
    let (nw, nh) = fit_dimensions(img.width(), img.height(), options);
    if (nw, nh) == (img.width(), img.height()) {
        img
    } else {
        img.resize_exact(nw, nh, FilterType::Lanczos3)
    }
}

/// Flatten any alpha onto white.
pub(crate) fn flatten_rgb(img: &DynamicImage) -> RgbImage {
    if !img.color().has_alpha() {
        return img.to_rgb8();
    }
    let rgba = img.to_rgba8();
    let mut out = RgbImage::new(rgba.width(), rgba.height());
    for (o, p) in out.pixels_mut().zip(rgba.pixels()) {
        let a = u32::from(p[3]);
        for c in 0..3 {
            o[c] = ((u32::from(p[c]) * a + 255 * (255 - a) + 127) / 255) as u8;
        }
    }
    out
}

/// 8-bit RGB, or RGBA when the source has alpha.
fn normalize(img: &DynamicImage) -> Cow<'_, DynamicImage> {
    match img {
        DynamicImage::ImageRgb8(_) | DynamicImage::ImageRgba8(_) => Cow::Borrowed(img),
        _ if img.color().has_alpha() => Cow::Owned(DynamicImage::ImageRgba8(img.to_rgba8())),
        _ => Cow::Owned(DynamicImage::ImageRgb8(img.to_rgb8())),
    }
}

fn create(output: &Path) -> Result<BufWriter<File>> {
    Ok(BufWriter::new(File::create(output)?))
}

fn write_png(img: &DynamicImage, output: &Path, compression: CompressionType) -> Result<()> {
    let mut w = create(output)?;
    let enc = PngEncoder::new_with_quality(&mut w, compression, PngFilter::Adaptive);
    normalize(img)
        .write_with_encoder(enc)
        .map_err(map_image_error)?;
    w.flush()?;
    Ok(())
}

fn write_jpeg(img: &DynamicImage, output: &Path, quality: u8) -> Result<()> {
    let rgb = flatten_rgb(img);
    let mut w = create(output)?;
    JpegEncoder::new_with_quality(&mut w, quality)
        .write_image(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            ExtendedColorType::Rgb8,
        )
        .map_err(map_image_error)?;
    w.flush()?;
    Ok(())
}

fn write_webp(img: &DynamicImage, output: &Path, quality: u8) -> Result<()> {
    let (w, h) = (img.width(), img.height());
    if w > 16383 || h > 16383 {
        return Err(Error::Unsupported(
            "WebP supports at most 16383 px per side".into(),
        ));
    }
    let rgba: RgbaImage = img.to_rgba8();
    let encoder = webp::Encoder::from_rgba(rgba.as_raw(), w, h);
    let data = if quality >= 100 {
        encoder.encode_simple(true, 75.0)
    } else {
        encoder.encode_simple(false, f32::from(quality))
    }
    .map_err(|e| Error::Failed(format!("webp encode failed: {e:?}")))?;
    std::fs::write(output, &*data)?;
    Ok(())
}

fn write_gif(img: &DynamicImage, output: &Path) -> Result<()> {
    let mut w = create(output)?;
    {
        let mut enc = GifEncoder::new_with_speed(&mut w, 10);
        enc.encode_frame(image::Frame::new(img.to_rgba8()))
            .map_err(map_image_error)?;
    }
    w.flush()?;
    Ok(())
}

fn write_bmp(img: &DynamicImage, output: &Path) -> Result<()> {
    let mut w = create(output)?;
    let enc = image::codecs::bmp::BmpEncoder::new(&mut w);
    normalize(img)
        .write_with_encoder(enc)
        .map_err(map_image_error)?;
    w.flush()?;
    Ok(())
}

fn write_tiff(img: &DynamicImage, output: &Path) -> Result<()> {
    let mut w = create(output)?;
    let enc = TiffEncoder::new(&mut w);
    normalize(img)
        .write_with_encoder(enc)
        .map_err(map_image_error)?;
    w.flush()?;
    Ok(())
}

/// Multi-size icon: sizes <= the smaller side (at least 16). Non-square sources are fitted
/// and centred on a transparent square.
fn write_ico(img: &DynamicImage, output: &Path) -> Result<()> {
    let smaller = img.width().min(img.height());
    let mut sizes: Vec<u32> = ICO_SIZES.into_iter().filter(|s| *s <= smaller).collect();
    if sizes.is_empty() {
        sizes.push(16);
    }
    let longer = img.width().max(img.height());
    let mut frames = Vec::with_capacity(sizes.len());
    for s in sizes {
        // Scale so the longer side is exactly `s` (also upscales tiny sources to 16).
        let k = f64::from(s) / f64::from(longer);
        let fw = ((f64::from(img.width()) * k).round() as u32).clamp(1, s);
        let fh = ((f64::from(img.height()) * k).round() as u32).clamp(1, s);
        let scaled = img.resize_exact(fw, fh, FilterType::Lanczos3).to_rgba8();
        let mut canvas = RgbaImage::new(s, s);
        image::imageops::replace(
            &mut canvas,
            &scaled,
            i64::from((s - fw) / 2),
            i64::from((s - fh) / 2),
        );
        frames.push(
            IcoFrame::as_png(canvas.as_raw(), s, s, ExtendedColorType::Rgba8)
                .map_err(map_image_error)?,
        );
    }
    let mut w = create(output)?;
    IcoEncoder::new(&mut w)
        .encode_images(&frames)
        .map_err(map_image_error)?;
    w.flush()?;
    Ok(())
}

fn write_as(img: &DynamicImage, to: &str, output: &Path, options: &ConvertOptions) -> Result<()> {
    let r = match to {
        "png" => write_png(img, output, CompressionType::Default),
        "jpg" | "jpeg" => write_jpeg(img, output, quality_or(options, 85)),
        "webp" => write_webp(img, output, quality_or(options, 85)),
        "gif" => write_gif(img, output),
        "bmp" => write_bmp(img, output),
        "tiff" | "tif" => write_tiff(img, output),
        "ico" => write_ico(img, output),
        other => Err(Error::Unsupported(format!("image output format: {other}"))),
    };
    if r.is_err() {
        let _ = std::fs::remove_file(output);
    }
    r
}

/// Convert `input` to raster format `to` ("png","jpg","webp","gif","bmp","tiff","ico").
/// Applies `max_width`/`max_height` (downscale only, Lanczos3). JPG has no alpha:
/// flatten onto white. WebP: quality 100 => lossless, else lossy at `quality` (default 85).
/// ICO: multi-size icon (16, 32, 48, 64, 128, 256 — only sizes <= source's smaller side, at least 16).
pub fn convert(
    input: &Path,
    to: &str,
    output: &Path,
    options: &ConvertOptions,
    cancel: &CancelToken,
) -> Result<()> {
    if cancel.is_cancelled() {
        return Err(Error::Cancelled);
    }
    let img = fit_owned(decode(input)?, options);
    if cancel.is_cancelled() {
        return Err(Error::Cancelled);
    }
    write_as(&img, to, output, options)
}

/// Encode an already-decoded image to `to`, same rules as [`convert`]. Used by svg/pdfium.
pub fn encode(
    img: &image::DynamicImage,
    to: &str,
    output: &Path,
    options: &ConvertOptions,
) -> Result<()> {
    let (nw, nh) = fit_dimensions(img.width(), img.height(), options);
    if (nw, nh) == (img.width(), img.height()) {
        write_as(img, to, output, options)
    } else {
        let resized = img.resize_exact(nw, nh, FilterType::Lanczos3);
        write_as(&resized, to, output, options)
    }
}

/// Same format, smaller file. jpg/webp: re-encode at quality Light 85 / Balanced 72 / Strong 55;
/// png: re-encode with best compression (Strong also quantizes nothing — keep lossless);
/// other formats: re-encode as-is. If the result is not smaller, copy the original bytes.
///
/// GIF is copied unchanged (re-encoding would drop every frame but the first).
pub fn compress(
    input: &Path,
    output: &Path,
    level: CompressLevel,
    cancel: &CancelToken,
) -> Result<()> {
    if cancel.is_cancelled() {
        return Err(Error::Cancelled);
    }
    let format = crate::formats::detect_path(input)
        .map(|f| f.id)
        .filter(|id| matches!(*id, "png" | "jpg" | "webp" | "gif" | "bmp" | "tiff" | "ico"))
        .ok_or_else(|| Error::Unsupported("not a raster image".into()))?;
    if format == "gif" {
        std::fs::copy(input, output)?;
        return Ok(());
    }
    let quality = match level {
        CompressLevel::Light => 85,
        CompressLevel::Balanced => 72,
        CompressLevel::Strong => 55,
    };
    let options = ConvertOptions {
        quality: Some(quality),
        ..ConvertOptions::default()
    };
    let img = decode(input)?;
    if cancel.is_cancelled() {
        return Err(Error::Cancelled);
    }
    if format == "png" {
        write_png(&img, output, CompressionType::Best)?;
    } else {
        write_as(&img, format, output, &options)?;
    }
    drop(img);
    let original = std::fs::metadata(input)?.len();
    let result = std::fs::metadata(output)?.len();
    if result >= original {
        std::fs::copy(input, output)?;
    }
    Ok(())
}

/// Crop to the rectangle (clamped to the image bounds; empty result => `Error::Failed`).
/// The output format follows `output`'s extension (falling back to the input's).
pub fn crop(input: &Path, output: &Path, x: u32, y: u32, width: u32, height: u32) -> Result<()> {
    let img = decode(input)?;
    if x >= img.width() || y >= img.height() || width == 0 || height == 0 {
        return Err(Error::Failed("crop rectangle is empty".into()));
    }
    let w = width.min(img.width() - x);
    let h = height.min(img.height() - y);
    let cropped = img.crop_imm(x, y, w, h);
    drop(img);
    let to = super::ext_of(output)
        .or_else(|| super::ext_of(input))
        .unwrap_or_else(|| "png".into());
    write_as(&cropped, &to, output, &ConvertOptions::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_limit_is_enforced() {
        assert!(check_dimensions(10_000, 20_000).is_ok());
        assert!(matches!(
            check_dimensions(20_000, 10_001),
            Err(Error::TooLarge(_))
        ));
        assert!(matches!(
            check_dimensions(u32::MAX, u32::MAX),
            Err(Error::TooLarge(_))
        ));
    }

    #[test]
    fn fit_never_upscales_and_keeps_aspect() {
        assert_eq!(fit_size(100, 50, Some(200), None), (100, 50));
        assert_eq!(fit_size(100, 50, Some(50), None), (50, 25));
        assert_eq!(fit_size(100, 50, None, Some(10)), (20, 10));
        assert_eq!(fit_size(100, 50, Some(0), Some(0)), (100, 50));
        assert_eq!(fit_size(1000, 1, Some(10), None), (10, 1));
    }
}

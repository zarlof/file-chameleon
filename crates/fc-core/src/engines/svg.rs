//! SVG rasterizer (`resvg`). OWNER: Task B.

use crate::engines::image as raster;
use crate::jobs::ConvertOptions;
use crate::{CancelToken, Error, Result};
use resvg::{tiny_skia, usvg};
use std::path::Path;
use std::sync::{Arc, OnceLock};

const MIN_LONG_SIDE: f32 = 1024.0;
const MAX_LONG_SIDE: f32 = 8192.0;
/// SVG sources are text; anything bigger than this is not a sane icon/illustration.
const MAX_SVG_BYTES: u64 = 64 * 1024 * 1024;

fn fontdb() -> Arc<usvg::fontdb::Database> {
    static DB: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = usvg::fontdb::Database::new();
        db.load_system_fonts();
        Arc::new(db)
    })
    .clone()
}

/// Output pixel size for an SVG of intrinsic size `(w, h)`: long side raised to at least
/// 1024, then the `max_width`/`max_height` fit, then the 8192 cap.
pub(crate) fn target_size(w: f32, h: f32, options: &ConvertOptions) -> (u32, u32) {
    let long = w.max(h);
    let up = if long < MIN_LONG_SIDE {
        MIN_LONG_SIDE / long
    } else {
        1.0
    };
    let (sw, sh) = (w * up, h * up);
    let (mut tw, mut th) = raster::fit_dimensions(
        (sw.round() as u32).max(1),
        (sh.round() as u32).max(1),
        options,
    );
    let long = tw.max(th) as f32;
    if long > MAX_LONG_SIDE {
        let k = MAX_LONG_SIDE / long;
        tw = ((tw as f32 * k).round() as u32).max(1);
        th = ((th as f32 * k).round() as u32).max(1);
    }
    (tw, th)
}

/// Render `input` (.svg) and encode it to `to` via `image::encode`.
/// Size: the SVG's intrinsic size, scaled so the longer side is at least 1024 px
/// (vector art should not come out tiny), then `max_width`/`max_height` fit applies.
/// Hard cap 8192 px on the longer side.
pub fn rasterize(
    input: &Path,
    to: &str,
    output: &Path,
    options: &ConvertOptions,
    cancel: &CancelToken,
) -> Result<()> {
    if cancel.is_cancelled() {
        return Err(Error::Cancelled);
    }
    let meta = std::fs::metadata(input).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Error::NotFound(input.to_path_buf())
        } else {
            Error::Io(e)
        }
    })?;
    if meta.len() > MAX_SVG_BYTES {
        return Err(Error::TooLarge("SVG file over 64 MB".into()));
    }
    let data = std::fs::read(input)?;
    let opt = usvg::Options {
        fontdb: fontdb(),
        // Never read local files referenced from an untrusted SVG; data: URLs still work.
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: usvg::ImageHrefResolver::default_data_resolver(),
            resolve_string: Box::new(|_, _| None),
        },
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_data(&data, &opt)
        .map_err(|e| Error::Failed(format!("invalid SVG: {e}")))?;
    drop(data);

    let size = tree.size();
    let (iw, ih) = (size.width(), size.height());
    if !(iw.is_finite() && ih.is_finite() && iw > 0.0 && ih > 0.0) {
        return Err(Error::Failed("SVG has no usable size".into()));
    }
    let (w, h) = target_size(iw, ih, options);
    let mut pixmap = tiny_skia::Pixmap::new(w, h)
        .ok_or_else(|| Error::Failed("could not allocate render surface".into()))?;
    if cancel.is_cancelled() {
        return Err(Error::Cancelled);
    }
    let transform = tiny_skia::Transform::from_scale(w as f32 / iw, h as f32 / ih);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    drop(tree);

    // tiny-skia is premultiplied; `image` wants straight alpha. Converts in place and
    // hands over the pixmap's own buffer (no second full-size copy).
    let raw = pixmap.take_demultiplied();
    let rgba = ::image::RgbaImage::from_raw(w, h, raw)
        .ok_or_else(|| Error::Failed("render buffer size mismatch".into()))?;
    if cancel.is_cancelled() {
        return Err(Error::Cancelled);
    }
    // Size rules are already applied; encode must not resize again.
    let encode_opts = ConvertOptions {
        max_width: None,
        max_height: None,
        ..options.clone()
    };
    raster::encode(
        &::image::DynamicImage::ImageRgba8(rgba),
        to,
        output,
        &encode_opts,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_svg_is_scaled_up_to_1024() {
        assert_eq!(
            target_size(100.0, 50.0, &ConvertOptions::default()),
            (1024, 512)
        );
    }

    #[test]
    fn large_svg_keeps_size_until_cap() {
        let o = ConvertOptions::default();
        assert_eq!(target_size(2000.0, 1000.0, &o), (2000, 1000));
        assert_eq!(target_size(20000.0, 10000.0, &o), (8192, 4096));
    }

    #[test]
    fn max_bounds_apply_after_upscale() {
        let o = ConvertOptions {
            max_width: Some(256),
            ..ConvertOptions::default()
        };
        assert_eq!(target_size(100.0, 50.0, &o), (256, 128));
    }
}

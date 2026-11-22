//! Builds the app icon from `assets/icon-artwork.png`.
//!
//! ```text
//! cargo run -p fc-core --release --example make_icon -- cutout   # artwork -> assets/icon-source.png
//! npx tauri icon assets/icon-source.png -o src-tauri/icons       # then delete icons/android and icons/ios
//! cargo run -p fc-core --release --example make_icon -- ico      # sharper 16-48 px frames -> icon.ico + 32x32.png
//! ```
//!
//! `cutout` drops the dark rounded tile of the artwork and keeps the chameleon, the folder and
//! the paper on a transparent 1024 px canvas. `ico` writes `src-tauri/icons/icon.ico` with PNG
//! frames whose small sizes get a light sharpen, so the chameleon stays readable.

use image::codecs::png::{CompressionType, FilterType as PngFilter, PngEncoder};
use image::imageops::{self, FilterType};
use image::{ImageEncoder, Rgba, Rgba32FImage, RgbaImage};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    let root = root();
    let source = root.join("assets/icon-source.png");
    match mode.as_str() {
        "cutout" => {
            let art = image::open(root.join("assets/icon-artwork.png")).expect("icon-artwork.png");
            save_png(&cutout(&art.to_rgb8()), &source);
        }
        "ico" => {
            let big = image::open(&source).expect("icon-source.png (run cutout first)");
            let icons = root.join("src-tauri/icons");
            std::fs::write(icons.join("icon.ico"), ico(&big.to_rgba8())).expect("write icon.ico");
            save_png(&frame(&big.to_rgba8(), 32), &icons.join("32x32.png"));
        }
        _ => {
            eprintln!("usage: make_icon cutout|ico");
            std::process::exit(2);
        }
    }
}

fn save_png(img: &RgbaImage, path: &Path) {
    std::fs::write(path, png_bytes(img))
        .unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}

fn png_bytes(img: &RgbaImage) -> Vec<u8> {
    let mut out = Vec::new();
    PngEncoder::new_with_quality(&mut out, CompressionType::Best, PngFilter::Adaptive)
        .write_image(
            img,
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgba8,
        )
        .expect("encode png");
    out
}

// ---------------------------------------------------------------- binary masks

/// A boolean image, row-major.
#[derive(Clone)]
struct Mask {
    w: usize,
    h: usize,
    px: Vec<bool>,
}

impl Mask {
    fn new(w: usize, h: usize, f: impl Fn(usize) -> bool) -> Self {
        Self {
            w,
            h,
            px: (0..w * h).map(f).collect(),
        }
    }

    /// 4-neighbours of pixel `i`.
    fn neighbours(&self, i: usize) -> impl Iterator<Item = usize> + use<> {
        let (w, h) = (self.w, self.h);
        let (x, y) = (i % w, i / w);
        [
            (x > 0).then(|| i - 1),
            (x + 1 < w).then(|| i + 1),
            (y > 0).then(|| i - w),
            (y + 1 < h).then(|| i + w),
        ]
        .into_iter()
        .flatten()
    }

    /// Connected regions (4-connectivity): a label per pixel (0 = background) and each label's
    /// size (index `label - 1`).
    fn label(&self) -> (Vec<u32>, Vec<usize>) {
        let mut labels = vec![0u32; self.px.len()];
        let mut sizes = Vec::new();
        let mut queue = VecDeque::new();
        for start in 0..self.px.len() {
            if !self.px[start] || labels[start] != 0 {
                continue;
            }
            sizes.push(0);
            let id = sizes.len() as u32;
            labels[start] = id;
            queue.push_back(start);
            while let Some(i) = queue.pop_front() {
                sizes[id as usize - 1] += 1;
                for n in self.neighbours(i) {
                    if self.px[n] && labels[n] == 0 {
                        labels[n] = id;
                        queue.push_back(n);
                    }
                }
            }
        }
        (labels, sizes)
    }

    /// Grow `self` into the `mask` pixels connected to it (like `scipy.ndimage.binary_propagation`).
    fn propagate(&self, mask: &Mask) -> Mask {
        let mut out = self.clone();
        let mut queue: VecDeque<usize> = (0..out.px.len()).filter(|&i| out.px[i]).collect();
        while let Some(i) = queue.pop_front() {
            for n in self.neighbours(i) {
                if mask.px[n] && !out.px[n] {
                    out.px[n] = true;
                    queue.push_back(n);
                }
            }
        }
        out
    }

    /// Erosion with the 4-neighbour cross, `times` times; pixels outside the image count as off.
    fn erode(&self, times: usize) -> Mask {
        let mut cur = self.clone();
        for _ in 0..times {
            let prev = cur.clone();
            for i in 0..cur.px.len() {
                let (x, y) = (i % cur.w, i / cur.w);
                let edge = x == 0 || y == 0 || x + 1 == cur.w || y + 1 == cur.h;
                cur.px[i] = prev.px[i] && !edge && prev.neighbours(i).all(|n| prev.px[n]);
            }
        }
        cur
    }
}

/// For every pixel, the index of the nearest `on` pixel (exact Euclidean distance transform,
/// Felzenszwalb & Huttenlocher, run on columns then rows).
fn nearest_on(m: &Mask) -> Vec<usize> {
    const INF: f64 = 1e20;
    let (w, h) = (m.w, m.h);
    // Columns: squared vertical distance and the row of the nearest `on` pixel.
    let mut col_d = vec![INF; w * h];
    let mut col_y = vec![0usize; w * h];
    for x in 0..w {
        let f: Vec<f64> = (0..h)
            .map(|y| if m.px[y * w + x] { 0.0 } else { INF })
            .collect();
        let (d, arg) = edt_1d(&f);
        for y in 0..h {
            col_d[y * w + x] = d[y];
            col_y[y * w + x] = arg[y];
        }
    }
    // Rows: combine with the horizontal distance.
    let mut out = vec![0usize; w * h];
    for y in 0..h {
        let f = &col_d[y * w..(y + 1) * w];
        let (_, arg) = edt_1d(f);
        for x in 0..w {
            let nx = arg[x];
            out[y * w + x] = col_y[y * w + nx] * w + nx;
        }
    }
    out
}

/// 1-D squared distance transform of `f`: the distance and the index that attains it.
fn edt_1d(f: &[f64]) -> (Vec<f64>, Vec<usize>) {
    let n = f.len();
    let mut v = vec![0usize; n];
    let mut z = vec![0f64; n + 1];
    let mut k = 0usize;
    z[0] = f64::NEG_INFINITY;
    z[1] = f64::INFINITY;
    let sq = |q: usize| (q * q) as f64;
    for q in 1..n {
        loop {
            let p = v[k];
            let s = ((f[q] + sq(q)) - (f[p] + sq(p))) / (2.0 * (q as f64 - p as f64));
            if s <= z[k] && k > 0 {
                k -= 1;
                continue;
            }
            if s <= z[k] {
                // k == 0: the new parabola replaces the first one.
                v[0] = q;
                z[1] = f64::INFINITY;
                break;
            }
            k += 1;
            v[k] = q;
            z[k] = s;
            z[k + 1] = f64::INFINITY;
            break;
        }
    }
    let (mut d, mut arg) = (vec![0f64; n], vec![0usize; n]);
    k = 0;
    for q in 0..n {
        while z[k + 1] < q as f64 {
            k += 1;
        }
        let p = v[k];
        d[q] = (q as f64 - p as f64).powi(2) + f[p];
        arg[q] = p;
    }
    (d, arg)
}

/// Gaussian blur of a float plane (`scipy.ndimage.gaussian_filter`: radius 4σ, mirrored edges).
fn gaussian(plane: &[f32], w: usize, h: usize, sigma: f32) -> Vec<f32> {
    let r = (4.0 * sigma + 0.5) as isize;
    let mut kernel: Vec<f32> = (-r..=r)
        .map(|i| (-((i * i) as f32) / (2.0 * sigma * sigma)).exp())
        .collect();
    let sum: f32 = kernel.iter().sum();
    kernel.iter_mut().for_each(|k| *k /= sum);
    // "reflect": d c b a | a b c d | d c b a
    let mirror = |i: isize, n: usize| -> usize {
        let n = n as isize;
        let period = 2 * n;
        let m = i.rem_euclid(period);
        (if m < n { m } else { period - 1 - m }) as usize
    };
    let pass = |src: &[f32], horizontal: bool| -> Vec<f32> {
        let mut dst = vec![0f32; src.len()];
        for y in 0..h {
            for x in 0..w {
                let mut acc = 0f32;
                for (j, k) in kernel.iter().enumerate() {
                    let o = j as isize - r;
                    let (sx, sy) = if horizontal {
                        (mirror(x as isize + o, w), y)
                    } else {
                        (x, mirror(y as isize + o, h))
                    };
                    acc += k * src[sy * w + sx];
                }
                dst[y * w + x] = acc;
            }
        }
        dst
    };
    pass(&pass(plane, true), false)
}

// ---------------------------------------------------------------- cutout

fn cutout(art: &image::RgbImage) -> RgbaImage {
    let (w, h) = (art.width() as usize, art.height() as usize);
    let rgb: Vec<[f32; 3]> = art.pixels().map(|p| p.0.map(f32::from)).collect();
    let sat = |c: &[f32; 3]| {
        c.iter().copied().fold(f32::MIN, f32::max) - c.iter().copied().fold(f32::MAX, f32::min)
    };
    let lum: Vec<f32> = rgb
        .iter()
        .map(|c| 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2])
        .collect();

    // Tile pixels are dark and grey. Regions of them count as background when they touch the
    // border or are large (the gap between the tail and the folder); small ones (the pupil) stay.
    let tile = Mask::new(w, h, |i| sat(&rgb[i]) < 32.0 && lum[i] < 120.0);
    let (labels, sizes) = tile.label();
    let mut is_bg = vec![false; sizes.len() + 1];
    for (id, &size) in sizes.iter().enumerate() {
        is_bg[id + 1] = size > 1000;
    }
    for x in 0..w {
        is_bg[labels[x] as usize] |= labels[x] != 0;
        let b = labels[(h - 1) * w + x];
        is_bg[b as usize] |= b != 0;
    }
    for y in 0..h {
        for b in [labels[y * w], labels[y * w + w - 1]] {
            is_bg[b as usize] |= b != 0;
        }
    }
    let bg = Mask::new(w, h, |i| labels[i] != 0 && is_bg[labels[i] as usize]);
    // Grow into the very dark shading that borders it (dark rims along the gap and the edges).
    let grow = Mask::new(w, h, |i| bg.px[i] || lum[i] < 75.0);
    let bg = bg.propagate(&grow);

    // Largest blob only (drops specks). No hole filling: the gap behind the tail is a hole.
    let subject = Mask::new(w, h, |i| !bg.px[i]);
    let (blobs, blob_sizes) = subject.label();
    let biggest = 1 + blob_sizes
        .iter()
        .enumerate()
        .max_by_key(|(_, s)| **s)
        .map_or(0, |(i, _)| i) as u32;
    let subject = Mask::new(w, h, |i| blobs[i] == biggest);

    // Edge pixels are mixed with the dark tile: give the rim the colour of the nearest inner pixel.
    let near = nearest_on(&subject.erode(3));
    let edge: Vec<f32> = subject
        .erode(1)
        .px
        .iter()
        .map(|&b| f32::from(u8::from(b)))
        .collect();
    let alpha = gaussian(&edge, w, h, 0.9);

    let mut out = RgbaImage::new(w as u32, h as u32);
    for (i, px) in out.pixels_mut().enumerate() {
        let c = rgb[near[i]];
        let a = ((alpha[i] - 0.1) / 0.8).clamp(0.0, 1.0);
        *px = Rgba([c[0] as u8, c[1] as u8, c[2] as u8, (a * 255.0) as u8]);
    }
    fit_canvas(&out, 1024, 20)
}

/// Crop `img` to its visible pixels and centre it on a transparent `side`² canvas with its
/// longer side `side - 2 * margin` long.
fn fit_canvas(img: &RgbaImage, side: u32, margin: u32) -> RgbaImage {
    let visible = img.enumerate_pixels().filter(|(_, _, p)| p[3] > 0);
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for (x, y, _) in visible {
        (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
    }
    assert!(x0 < x1, "the cutout is empty");
    let cropped = imageops::crop_imm(img, x0, y0, x1 - x0, y1 - y0).to_image();
    let scale = f64::from(side - 2 * margin) / f64::from(cropped.width().max(cropped.height()));
    let size = |v: u32| (f64::from(v) * scale).round_ties_even() as u32;
    let scaled = resize(&cropped, size(cropped.width()), size(cropped.height()));
    let mut canvas = RgbaImage::new(side, side);
    let (ox, oy) = ((side - scaled.width()) / 2, (side - scaled.height()) / 2);
    imageops::overlay(&mut canvas, &scaled, i64::from(ox), i64::from(oy));
    canvas
}

/// Lanczos resize on premultiplied alpha (as Pillow does), so transparent pixels never bleed.
fn resize(img: &RgbaImage, w: u32, h: u32) -> RgbaImage {
    let pre = Rgba32FImage::from_fn(img.width(), img.height(), |x, y| {
        let p = img.get_pixel(x, y).0.map(|v| f32::from(v) / 255.0);
        Rgba([p[0] * p[3], p[1] * p[3], p[2] * p[3], p[3]])
    });
    let small = imageops::resize(&pre, w, h, FilterType::Lanczos3);
    RgbaImage::from_fn(w, h, |x, y| {
        let [r, g, b, a] = small.get_pixel(x, y).0;
        let a = a.clamp(0.0, 1.0);
        let un = |c: f32| {
            if a > 0.0 {
                (c / a).clamp(0.0, 1.0)
            } else {
                0.0
            }
        };
        let q = |c: f32| (c * 255.0).round() as u8;
        Rgba([q(un(r)), q(un(g)), q(un(b)), q(a)])
    })
}

// ---------------------------------------------------------------- ico

/// One icon frame: `s`² px, with a light unsharp mask at 48 px and below.
fn frame(big: &RgbaImage, s: u32) -> RgbaImage {
    let f = resize(big, s, s);
    if s > 48 {
        return f;
    }
    // Pillow's UnsharpMask(radius=0.6, percent=60, threshold=1), on every channel.
    let (w, h) = (s as usize, s as usize);
    let mut out = f.clone();
    for c in 0..4 {
        let plane: Vec<f32> = f.pixels().map(|p| f32::from(p[c])).collect();
        let blurred = gaussian(&plane, w, h, 0.6);
        for (i, px) in out.pixels_mut().enumerate() {
            let diff = plane[i] - blurred[i];
            if diff.abs() >= 1.0 {
                px[c] = (plane[i] + diff * 0.6).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}

/// An .ico file with PNG frames of 16 to 256 px.
fn ico(big: &RgbaImage) -> Vec<u8> {
    const SIZES: [u32; 6] = [16, 24, 32, 48, 64, 256];
    let frames: Vec<Vec<u8>> = SIZES.iter().map(|&s| png_bytes(&frame(big, s))).collect();
    let mut out = Vec::new();
    for v in [0u16, 1, SIZES.len() as u16] {
        out.extend(v.to_le_bytes());
    }
    let mut offset = 6 + 16 * SIZES.len() as u32;
    for (s, png) in SIZES.iter().zip(&frames) {
        let dim = (s % 256) as u8; // 0 means 256
        out.extend([dim, dim, 0, 0]);
        out.extend(1u16.to_le_bytes()); // colour planes
        out.extend(32u16.to_le_bytes()); // bits per pixel
        out.extend((png.len() as u32).to_le_bytes());
        out.extend(offset.to_le_bytes());
        offset += png.len() as u32;
    }
    frames.iter().for_each(|f| out.extend(f));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edt_finds_the_nearest_pixel() {
        let m = Mask::new(5, 1, |i| i == 0 || i == 4);
        let near = nearest_on(&m);
        assert_eq!(near, [0, 0, 0, 4, 4]);
        let m = Mask::new(3, 3, |i| i == 8);
        assert!(nearest_on(&m).iter().all(|&n| n == 8));
    }

    #[test]
    fn erosion_eats_the_border_and_thin_parts() {
        let m = Mask::new(5, 5, |_| true);
        let e = m.erode(1);
        assert_eq!(e.px.iter().filter(|&&b| b).count(), 9);
        assert_eq!(m.erode(2).px.iter().filter(|&&b| b).count(), 1);
    }

    #[test]
    fn labels_use_four_neighbours() {
        // Two diagonal pixels are two regions.
        let m = Mask::new(2, 2, |i| i == 0 || i == 3);
        let (_, sizes) = m.label();
        assert_eq!(sizes, [1, 1]);
    }

    #[test]
    fn ico_header_lists_every_frame() {
        let big = RgbaImage::from_pixel(64, 64, Rgba([10, 200, 30, 255]));
        let data = ico(&big);
        assert_eq!(&data[..6], &[0, 0, 1, 0, 6, 0]);
        assert_eq!(data[6], 16);
        assert_eq!(data[6 + 16 * 5], 0); // 256 px
    }
}

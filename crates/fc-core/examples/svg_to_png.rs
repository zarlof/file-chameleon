//! Renders an SVG to a square PNG (used for the app icon).
//! `cargo run -p fc-core --example svg_to_png -- assets/icon.svg assets/icon-source.png 1024`
//! then `npx tauri icon assets/icon-source.png -o src-tauri/icons`.

use resvg::{tiny_skia, usvg};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [input, output, size] = args.as_slice() else {
        eprintln!("usage: svg_to_png <in.svg> <out.png> <size>");
        std::process::exit(2);
    };
    let size: u32 = size.parse().expect("size");
    let data = std::fs::read(input).expect("read svg");
    let tree = usvg::Tree::from_data(&data, &usvg::Options::default()).expect("parse svg");
    let mut pixmap = tiny_skia::Pixmap::new(size, size).expect("pixmap");
    let scale = size as f32 / tree.size().width().max(tree.size().height());
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    // tiny-skia stores premultiplied RGBA; PNG wants straight alpha.
    let mut rgba = pixmap.take();
    for px in rgba.as_chunks_mut::<4>().0 {
        let a = px[3] as u32;
        if a > 0 && a < 255 {
            for c in &mut px[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    image::RgbaImage::from_raw(size, size, rgba)
        .expect("buffer")
        .save(output)
        .expect("write png");
    println!("wrote {output}");
}

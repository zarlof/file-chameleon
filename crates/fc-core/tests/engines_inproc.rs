//! Integration tests for the in-process engines (image, svg, data, pdf).
//! All inputs are generated in code.

use fc_core::engines::{data, image as img_engine, pdf, svg};
use fc_core::jobs::{CompressLevel, ConvertOptions};
use fc_core::{CancelToken, Error};
use image::{GenericImageView, Rgba, RgbaImage};
use std::path::{Path, PathBuf};

fn opts() -> ConvertOptions {
    ConvertOptions::default()
}

fn tok() -> CancelToken {
    CancelToken::new()
}

/// Opaque gradient with some structure.
fn gradient(w: u32, h: u32) -> RgbaImage {
    RgbaImage::from_fn(w, h, |x, y| {
        Rgba([
            (x * 255 / w) as u8,
            (y * 255 / h) as u8,
            ((x ^ y) & 0xff) as u8,
            255,
        ])
    })
}

fn write_png(dir: &Path, name: &str, img: &RgbaImage) -> PathBuf {
    let p = dir.join(name);
    img.save(&p).unwrap();
    p
}

#[test]
fn every_raster_pair_converts_with_right_dimensions() {
    let dir = tempfile::tempdir().unwrap();
    let src = write_png(dir.path(), "base.png", &gradient(64, 48));
    let formats = ["png", "jpg", "webp", "gif", "bmp", "tiff"];
    // Make one source file per format first.
    let mut sources = Vec::new();
    for f in formats {
        let p = dir.path().join(format!("src.{f}"));
        img_engine::convert(&src, f, &p, &opts(), &tok()).unwrap();
        sources.push((f, p));
    }
    for (from, input) in &sources {
        for to in formats.iter().chain(["ico"].iter()) {
            let out = dir.path().join(format!("{from}-to.{to}"));
            img_engine::convert(input, to, &out, &opts(), &tok())
                .unwrap_or_else(|e| panic!("{from}->{to}: {e}"));
            let decoded = image::open(&out).unwrap_or_else(|e| panic!("{from}->{to} decode: {e}"));
            if *to == "ico" {
                // 48 px smaller side => 16, 32, 48
                assert!(decoded.width() >= 16);
            } else {
                assert_eq!(decoded.dimensions(), (64, 48), "{from}->{to}");
            }
        }
    }
}

#[test]
fn jpg_has_no_alpha_and_flattens_to_white() {
    let dir = tempfile::tempdir().unwrap();
    let transparent = RgbaImage::from_pixel(20, 20, Rgba([255, 0, 0, 0]));
    let src = write_png(dir.path(), "t.png", &transparent);
    let out = dir.path().join("t.jpg");
    img_engine::convert(&src, "jpg", &out, &opts(), &tok()).unwrap();
    let d = image::open(&out).unwrap();
    assert!(!d.color().has_alpha());
    let px = d.to_rgb8().get_pixel(10, 10).0;
    assert!(px.iter().all(|c| *c > 245), "expected white, got {px:?}");
}

#[test]
fn webp_q100_is_lossless() {
    let dir = tempfile::tempdir().unwrap();
    let g = gradient(33, 21);
    let src = write_png(dir.path(), "g.png", &g);
    let out = dir.path().join("g.webp");
    let o = ConvertOptions {
        quality: Some(100),
        ..opts()
    };
    img_engine::convert(&src, "webp", &out, &o, &tok()).unwrap();
    let back = image::open(&out).unwrap().to_rgba8();
    assert_eq!(back.as_raw(), g.as_raw());

    // Lossy at lower quality is smaller than lossless for a noisy image.
    let lossy = dir.path().join("g-lossy.webp");
    let o = ConvertOptions {
        quality: Some(40),
        ..opts()
    };
    img_engine::convert(&src, "webp", &lossy, &o, &tok()).unwrap();
    assert!(std::fs::metadata(&lossy).unwrap().len() < std::fs::metadata(&out).unwrap().len());
}

#[test]
fn ico_contains_multiple_sizes() {
    let dir = tempfile::tempdir().unwrap();
    let src = write_png(dir.path(), "big.png", &gradient(300, 200));
    let out = dir.path().join("big.ico");
    img_engine::convert(&src, "ico", &out, &opts(), &tok()).unwrap();
    let bytes = std::fs::read(&out).unwrap();
    let count = u16::from_le_bytes([bytes[4], bytes[5]]);
    assert_eq!(count, 5, "16,32,48,64,128 (smaller side is 200)");
    let widths: Vec<u8> = (0..count as usize).map(|i| bytes[6 + i * 16]).collect();
    assert_eq!(widths, vec![16, 32, 48, 64, 128]);

    // Tiny source still yields a 16 px icon.
    let tiny = write_png(dir.path(), "tiny.png", &gradient(8, 8));
    let out = dir.path().join("tiny.ico");
    img_engine::convert(&tiny, "ico", &out, &opts(), &tok()).unwrap();
    let bytes = std::fs::read(&out).unwrap();
    assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), 1);
    assert_eq!(bytes[6], 16);
}

#[test]
fn max_width_downscales_and_never_upscales() {
    let dir = tempfile::tempdir().unwrap();
    let src = write_png(dir.path(), "s.png", &gradient(200, 100));
    let out = dir.path().join("o.png");
    let o = ConvertOptions {
        max_width: Some(50),
        ..opts()
    };
    img_engine::convert(&src, "png", &out, &o, &tok()).unwrap();
    assert_eq!(image::open(&out).unwrap().dimensions(), (50, 25));

    let o = ConvertOptions {
        max_width: Some(1000),
        max_height: Some(1000),
        ..opts()
    };
    img_engine::convert(&src, "png", &out, &o, &tok()).unwrap();
    assert_eq!(image::open(&out).unwrap().dimensions(), (200, 100));

    let o = ConvertOptions {
        max_height: Some(20),
        ..opts()
    };
    img_engine::convert(&src, "png", &out, &o, &tok()).unwrap();
    assert_eq!(image::open(&out).unwrap().dimensions(), (40, 20));
}

#[test]
fn huge_declared_dimensions_are_rejected_before_decoding() {
    // 24-bit BMP header claiming 20000 x 20000 (400 MP) with no pixel data.
    let mut b = Vec::new();
    b.extend_from_slice(b"BM");
    b.extend_from_slice(&54u32.to_le_bytes()); // file size (lies)
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&54u32.to_le_bytes()); // pixel data offset
    b.extend_from_slice(&40u32.to_le_bytes()); // DIB header size
    b.extend_from_slice(&20000i32.to_le_bytes());
    b.extend_from_slice(&20000i32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&24u16.to_le_bytes());
    b.extend_from_slice(&[0u8; 24]);
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("huge.bmp");
    std::fs::write(&p, b).unwrap();
    let out = dir.path().join("huge.png");
    let err = img_engine::convert(&p, "png", &out, &opts(), &tok()).unwrap_err();
    assert!(matches!(err, Error::TooLarge(_)), "got {err:?}");
    assert!(!out.exists());
}

#[test]
fn cancelled_token_stops_convert() {
    let dir = tempfile::tempdir().unwrap();
    let src = write_png(dir.path(), "s.png", &gradient(10, 10));
    let t = tok();
    t.cancel();
    let err = img_engine::convert(&src, "jpg", &dir.path().join("o.jpg"), &opts(), &t).unwrap_err();
    assert!(matches!(err, Error::Cancelled));
}

#[test]
fn missing_input_is_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let err = img_engine::convert(
        &dir.path().join("nope.png"),
        "jpg",
        &dir.path().join("o.jpg"),
        &opts(),
        &tok(),
    )
    .unwrap_err();
    assert!(matches!(err, Error::NotFound(_)), "got {err:?}");
}

#[test]
fn crop_clamps_and_rejects_empty() {
    let dir = tempfile::tempdir().unwrap();
    let src = write_png(dir.path(), "c.png", &gradient(100, 80));
    let out = dir.path().join("c-out.png");
    img_engine::crop(&src, &out, 10, 20, 30, 40).unwrap();
    assert_eq!(image::open(&out).unwrap().dimensions(), (30, 40));
    img_engine::crop(&src, &out, 90, 70, 500, 500).unwrap();
    assert_eq!(image::open(&out).unwrap().dimensions(), (10, 10));
    let err = img_engine::crop(&src, &out, 100, 0, 10, 10).unwrap_err();
    assert!(matches!(err, Error::Failed(_)));
}

#[test]
fn image_compress_is_never_larger_and_decodes() {
    let dir = tempfile::tempdir().unwrap();
    let src = write_png(dir.path(), "p.png", &gradient(120, 90));
    let jpg = dir.path().join("p.jpg");
    let o = ConvertOptions {
        quality: Some(98),
        ..opts()
    };
    img_engine::convert(&src, "jpg", &jpg, &o, &tok()).unwrap();
    let out = dir.path().join("p-small.jpg");
    img_engine::compress(&jpg, &out, CompressLevel::Strong, &tok()).unwrap();
    assert!(std::fs::metadata(&out).unwrap().len() <= std::fs::metadata(&jpg).unwrap().len());
    assert_eq!(image::open(&out).unwrap().dimensions(), (120, 90));

    let out = dir.path().join("p-small.png");
    img_engine::compress(&src, &out, CompressLevel::Light, &tok()).unwrap();
    assert!(std::fs::metadata(&out).unwrap().len() <= std::fs::metadata(&src).unwrap().len());
    assert_eq!(image::open(&out).unwrap().to_rgba8(), gradient(120, 90));
}

// ---------------------------------------------------------------- svg

const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50" viewBox="0 0 100 50">
<rect width="50" height="50" fill="#ff0000"/></svg>"##;

#[test]
fn svg_rasterizes_to_at_least_1024_and_respects_max() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("a.svg");
    std::fs::write(&src, SVG).unwrap();
    let out = dir.path().join("a.png");
    svg::rasterize(&src, "png", &out, &opts(), &tok()).unwrap();
    let d = image::open(&out).unwrap();
    assert_eq!(d.dimensions(), (1024, 512));
    // Left half red, right half transparent.
    assert_eq!(d.get_pixel(100, 100).0, [255, 0, 0, 255]);
    assert_eq!(d.get_pixel(900, 100).0[3], 0);

    let o = ConvertOptions {
        max_width: Some(300),
        ..opts()
    };
    svg::rasterize(&src, "png", &out, &o, &tok()).unwrap();
    assert_eq!(image::open(&out).unwrap().dimensions(), (300, 150));

    let jpg = dir.path().join("a.jpg");
    svg::rasterize(&src, "jpg", &jpg, &opts(), &tok()).unwrap();
    let d = image::open(&jpg).unwrap();
    assert!(!d.color().has_alpha());
    assert!(d.to_rgb8().get_pixel(900, 100).0.iter().all(|c| *c > 245));
}

#[test]
fn invalid_svg_is_an_error_not_a_panic() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("bad.svg");
    std::fs::write(&src, "this is not svg").unwrap();
    let err = svg::rasterize(&src, "png", &dir.path().join("o.png"), &opts(), &tok()).unwrap_err();
    assert!(matches!(err, Error::Failed(_)), "got {err:?}");
}

// ---------------------------------------------------------------- data

fn convert_text(dir: &Path, name: &str, content: &str, from: &str, to: &str) -> String {
    let input = dir.join(name);
    std::fs::write(&input, content).unwrap();
    let out = dir.join(format!("{name}.{to}"));
    data::convert(&input, from, to, &out).unwrap();
    std::fs::read_to_string(out).unwrap()
}

#[test]
fn markdown_to_html_has_title_h1_and_extensions() {
    let dir = tempfile::tempdir().unwrap();
    let html = convert_text(
        dir.path(),
        "Tom & Jerry.md",
        "# Hello\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n~~gone~~ and\n\n- [x] done\n",
        "md",
        "html",
    );
    assert!(html.contains("<h1>Hello</h1>"));
    assert!(html.contains("<title>Tom &amp; Jerry</title>"));
    assert!(html.contains("<meta charset=\"utf-8\">"));
    assert!(html.contains("<table>"));
    assert!(html.contains("<del>gone</del>"));
    assert!(html.contains("checkbox"));
    assert!(html.contains("prefers-color-scheme"));
}

#[test]
fn txt_to_html_escapes() {
    let dir = tempfile::tempdir().unwrap();
    let html = convert_text(dir.path(), "n.txt", "a < b && <script>", "txt", "html");
    assert!(html.contains("<pre>a &lt; b &amp;&amp; &lt;script&gt;</pre>"));
    assert!(html.contains("<title>n</title>"));
}

const CSV: &str = "name,age\nAda,36\n\"Bob, Jr.\",41\n";

#[test]
fn csv_json_yaml_roundtrips() {
    let dir = tempfile::tempdir().unwrap();
    let json = convert_text(dir.path(), "t.csv", CSV, "csv", "json");
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v[0]["name"], "Ada");
    assert_eq!(v[0]["age"], "36"); // csv values stay strings
    assert_eq!(v[1]["name"], "Bob, Jr.");

    // json -> csv: header is the sorted union of keys; compare as parsed records.
    let back = convert_text(dir.path(), "t.json", &json, "json", "csv");
    let mut rdr = csv::Reader::from_reader(back.as_bytes());
    let headers: Vec<String> = rdr.headers().unwrap().iter().map(String::from).collect();
    assert_eq!(headers.len(), 2);
    let name_idx = headers.iter().position(|h| h == "name").unwrap();
    let names: Vec<String> = rdr
        .records()
        .map(|r| r.unwrap().get(name_idx).unwrap().to_owned())
        .collect();
    assert_eq!(names, ["Ada", "Bob, Jr."]);

    // csv -> yaml -> json == csv -> json
    let yaml = convert_text(dir.path(), "t2.csv", CSV, "csv", "yaml");
    let json2 = convert_text(dir.path(), "t2.yaml", &yaml, "yaml", "json");
    let v2: serde_json::Value = serde_json::from_str(&json2).unwrap();
    assert_eq!(v, v2);
}

#[test]
fn json_toml_roundtrips() {
    let dir = tempfile::tempdir().unwrap();
    let src = r#"{"title":"x","n":3,"flag":true,"list":[1,2],"nested":{"a":"b"},"gone":null}"#;
    let toml_text = convert_text(dir.path(), "o.json", src, "json", "toml");
    let back = convert_text(dir.path(), "o.toml", &toml_text, "toml", "json");
    let v: serde_json::Value = serde_json::from_str(&back).unwrap();
    assert_eq!(
        v,
        serde_json::json!({"title":"x","n":3,"flag":true,"list":[1,2],"nested":{"a":"b"}})
    );

    // Top-level arrays are wrapped as { items = [...] }.
    let wrapped = convert_text(dir.path(), "a.json", r#"[{"k":1},{"k":2}]"#, "json", "toml");
    let back = convert_text(dir.path(), "a.toml", &wrapped, "toml", "json");
    let v: serde_json::Value = serde_json::from_str(&back).unwrap();
    assert_eq!(v, serde_json::json!({"items":[{"k":1},{"k":2}]}));

    // TOML datetimes become strings.
    let dt = convert_text(
        dir.path(),
        "d.toml",
        "when = 1979-05-27T07:32:00Z\n",
        "toml",
        "json",
    );
    let v: serde_json::Value = serde_json::from_str(&dt).unwrap();
    assert_eq!(v["when"], "1979-05-27T07:32:00Z");
}

#[test]
fn to_csv_requires_array_of_objects_and_encodes_nested_values() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("bad.json");
    std::fs::write(&input, r#"{"a":{"b":1}}"#).unwrap();
    let err = data::convert(&input, "json", "csv", &dir.path().join("o.csv")).unwrap_err();
    assert!(matches!(err, Error::Failed(_)), "got {err:?}");
    assert!(!dir.path().join("o.csv").exists());

    let csv_text = convert_text(
        dir.path(),
        "n.json",
        r#"[{"id":1,"meta":{"x":[1,2]}},{"id":2,"extra":true}]"#,
        "json",
        "csv",
    );
    let mut rdr = csv::Reader::from_reader(csv_text.as_bytes());
    let headers: Vec<String> = rdr.headers().unwrap().iter().map(String::from).collect();
    assert_eq!(headers, ["id", "meta", "extra"]);
    let recs: Vec<csv::StringRecord> = rdr.records().map(|r| r.unwrap()).collect();
    assert_eq!(&recs[0][1], r#"{"x":[1,2]}"#);
    assert_eq!(&recs[1][2], "true");
}

#[test]
fn data_rejects_unsupported_pairs_and_invalid_input() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("x.json");
    std::fs::write(&p, "{").unwrap();
    assert!(matches!(
        data::convert(&p, "json", "yaml", &dir.path().join("o")).unwrap_err(),
        Error::Failed(_)
    ));
    assert!(matches!(
        data::convert(&p, "json", "html", &dir.path().join("o")).unwrap_err(),
        Error::Unsupported(_)
    ));
}

// ---------------------------------------------------------------- pdf

fn make_images(dir: &Path, n: u32) -> Vec<PathBuf> {
    (0..n)
        .map(|i| write_png(dir, &format!("img{i}.png"), &gradient(40 + i * 10, 30)))
        .collect()
}

fn make_pdf(dir: &Path, name: &str, pages: u32) -> PathBuf {
    let imgs = make_images(dir, pages);
    let out = dir.join(name);
    pdf::images_to_pdf(&imgs, &out, &tok(), &|_| {}).unwrap();
    out
}

#[test]
fn images_to_pdf_page_sizes_and_count() {
    let dir = tempfile::tempdir().unwrap();
    // png (with alpha), jpeg
    let mut imgs = make_images(dir.path(), 2);
    let jpg = dir.path().join("j.jpg");
    img_engine::convert(&imgs[0], "jpg", &jpg, &opts(), &tok()).unwrap();
    imgs.push(jpg);
    let out = dir.path().join("o.pdf");
    pdf::images_to_pdf(&imgs, &out, &tok(), &|_| {}).unwrap();
    assert_eq!(pdf::page_count(&out).unwrap(), 3);

    let doc = lopdf::Document::load(&out).unwrap();
    let pages = doc.get_pages();
    let sizes: Vec<(i64, i64)> = pages
        .values()
        .map(|id| {
            let mb = doc.get_dictionary(*id).unwrap().get(b"MediaBox").unwrap();
            let a = mb.as_array().unwrap();
            (a[2].as_i64().unwrap(), a[3].as_i64().unwrap())
        })
        .collect();
    assert_eq!(sizes, [(40, 30), (50, 30), (40, 30)]);

    // The JPEG is embedded untouched (DCTDecode).
    let has_dct = doc.objects.values().any(|o| {
        o.as_stream()
            .ok()
            .and_then(|s| s.dict.get(b"Filter").ok())
            .and_then(|f| f.as_name().ok())
            == Some(b"DCTDecode".as_slice())
    });
    assert!(has_dct);
}

#[test]
fn merge_sums_page_counts_and_accepts_images() {
    let dir = tempfile::tempdir().unwrap();
    let a = make_pdf(dir.path(), "a.pdf", 2);
    let b = make_pdf(dir.path(), "b.pdf", 3);
    let img = write_png(dir.path(), "x.png", &gradient(20, 20));
    let out = dir.path().join("merged.pdf");
    pdf::merge(&[a.clone(), img, b.clone()], &out, &tok(), &|_| {}).unwrap();
    assert_eq!(pdf::page_count(&out).unwrap(), 6);
    // Fully parseable and every page resolvable.
    let doc = lopdf::Document::load(&out).unwrap();
    assert_eq!(doc.get_pages().len(), 6);
    for id in doc.get_pages().values() {
        assert!(doc.get_dictionary(*id).unwrap().has(b"MediaBox"));
    }
}

#[test]
fn split_writes_one_file_per_page() {
    let dir = tempfile::tempdir().unwrap();
    let src = make_pdf(dir.path(), "doc.pdf", 3);
    let out_dir = dir.path().join("parts");
    std::fs::create_dir(&out_dir).unwrap();
    let files = pdf::split(&src, &out_dir, &tok(), &|_| {}).unwrap();
    assert_eq!(files.len(), 3);
    for (i, f) in files.iter().enumerate() {
        assert_eq!(
            f.file_name().unwrap().to_str().unwrap(),
            format!("doc-{}.pdf", i + 1)
        );
        assert_eq!(pdf::page_count(f).unwrap(), 1);
    }
    // Page i has the i-th page's size.
    let widths: Vec<i64> = files
        .iter()
        .map(|f| {
            let d = lopdf::Document::load(f).unwrap();
            let id = *d.get_pages().values().next().unwrap();
            d.get_dictionary(id)
                .unwrap()
                .get(b"MediaBox")
                .unwrap()
                .as_array()
                .unwrap()[2]
                .as_i64()
                .unwrap()
        })
        .collect();
    assert_eq!(widths, [40, 50, 60]);
    assert_eq!(pdf::page_count(&src).unwrap(), 3, "source untouched");
}

#[test]
fn pdf_compress_output_parses() {
    let dir = tempfile::tempdir().unwrap();
    let src = make_pdf(dir.path(), "c.pdf", 2);
    let out = dir.path().join("c-small.pdf");
    pdf::compress(&src, &out).unwrap();
    assert_eq!(pdf::page_count(&out).unwrap(), 2);
    lopdf::Document::load(&out).unwrap();
    assert!(std::fs::metadata(&out).unwrap().len() <= std::fs::metadata(&src).unwrap().len());
}

#[test]
fn pdf_errors_on_garbage_and_missing_files() {
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("bad.pdf");
    std::fs::write(&bad, "not a pdf").unwrap();
    assert!(pdf::page_count(&bad).is_err());
    assert!(matches!(
        pdf::page_count(&dir.path().join("nope.pdf")).unwrap_err(),
        Error::NotFound(_)
    ));
    let out = dir.path().join("m.pdf");
    assert!(pdf::merge(&[bad], &out, &tok(), &|_| {}).is_err());
    assert!(!out.exists());
}

#[test]
fn render_pages_reports_missing_pdfium_and_renders_when_available() {
    let dir = tempfile::tempdir().unwrap();
    let src = make_pdf(dir.path(), "r.pdf", 2);
    let outs = vec![dir.path().join("r-1.png"), dir.path().join("r-2.png")];

    let err = pdf::render_pages(
        &dir.path().join("no-such-pdfium.dll"),
        &src,
        "png",
        &outs,
        &opts(),
        &tok(),
        &|_| {},
    );
    // Only meaningful when pdfium was not already bound by the optional run below.
    if std::env::var_os("PDFIUM_DLL").is_none() {
        assert!(matches!(err.unwrap_err(), Error::MissingTool(t) if t == "pdfium"));
        println!("PDFIUM_DLL not set; skipping the real render check");
        return;
    }
    let dll = PathBuf::from(std::env::var_os("PDFIUM_DLL").unwrap());
    pdf::render_pages(&dll, &src, "png", &outs, &opts(), &tok(), &|_| {}).unwrap();
    for o in &outs {
        let d = image::open(o).unwrap();
        // 40x30 pt at 150 dpi ~ 83x63 (page 2 is 50x30 pt).
        assert!(d.width() > 60 && d.height() > 40, "{:?}", d.dimensions());
    }
}

// ------------------------------------------------ nested page tree fixture

/// Page tree: Catalog -> Pages(root: MediaBox + Resources) -> [Pages(mid) -> [p1, p2], p3].
/// MediaBox and Resources exist only on the root node and are inherited by all three pages.
fn nested_pdf(path: &Path) {
    use lopdf::{Dictionary, Document, Object, Stream};
    let mut doc = Document::with_version("1.5");
    let root = doc.new_object_id();
    let mid = doc.new_object_id();
    let font = doc.add_object(Object::Dictionary({
        let mut d = Dictionary::new();
        d.set("Type", "Font");
        d.set("Subtype", "Type1");
        d.set("BaseFont", "Courier");
        d
    }));
    let mut fonts = Dictionary::new();
    fonts.set("F1", Object::Reference(font));
    let mut resources = Dictionary::new();
    resources.set("Font", Object::Dictionary(fonts));

    let mut page_ids = Vec::new();
    for (i, parent) in [mid, mid, root].into_iter().enumerate() {
        let content = format!("BT /F1 12 Tf 10 10 Td (page {}) Tj ET", i + 1);
        let c = doc.add_object(Stream::new(Dictionary::new(), content.into_bytes()));
        let mut p = Dictionary::new();
        p.set("Type", "Page");
        p.set("Parent", Object::Reference(parent));
        p.set("Contents", Object::Reference(c));
        page_ids.push(doc.add_object(Object::Dictionary(p)));
    }
    // Page 1 carries a Link annotation whose destination is page 3.
    let mut annot = Dictionary::new();
    annot.set("Type", "Annot");
    annot.set("Subtype", "Link");
    annot.set(
        "Rect",
        Object::Array(vec![0.into(), 0.into(), 10.into(), 10.into()]),
    );
    annot.set(
        "Dest",
        Object::Array(vec![Object::Reference(page_ids[2]), "Fit".into()]),
    );
    let annot_id = doc.add_object(Object::Dictionary(annot));
    if let Some(Object::Dictionary(p)) = doc.objects.get_mut(&page_ids[0]) {
        p.set("Annots", Object::Array(vec![Object::Reference(annot_id)]));
    }
    let mut mid_d = Dictionary::new();
    mid_d.set("Type", "Pages");
    mid_d.set("Parent", Object::Reference(root));
    mid_d.set(
        "Kids",
        Object::Array(vec![
            Object::Reference(page_ids[0]),
            Object::Reference(page_ids[1]),
        ]),
    );
    mid_d.set("Count", 2);
    doc.objects.insert(mid, Object::Dictionary(mid_d));
    let mut root_d = Dictionary::new();
    root_d.set("Type", "Pages");
    root_d.set(
        "Kids",
        Object::Array(vec![Object::Reference(mid), Object::Reference(page_ids[2])]),
    );
    root_d.set("Count", 3);
    root_d.set(
        "MediaBox",
        Object::Array(vec![0.into(), 0.into(), 300.into(), 400.into()]),
    );
    root_d.set("Resources", Object::Dictionary(resources));
    doc.objects.insert(root, Object::Dictionary(root_d));
    let mut cat = Dictionary::new();
    cat.set("Type", "Catalog");
    cat.set("Pages", Object::Reference(root));
    let cat_id = doc.add_object(Object::Dictionary(cat));
    doc.trailer.set("Root", Object::Reference(cat_id));
    doc.save(path).unwrap();
}

/// The first `fixture_pages` pages resolve MediaBox, a Resources dict holding the font,
/// and non-empty content directly on the page (no reliance on a parent node).
fn assert_pages_self_contained(path: &Path, expect_pages: usize, fixture_pages: usize) {
    let doc = lopdf::Document::load(path).unwrap();
    let pages = doc.get_pages();
    assert_eq!(pages.len(), expect_pages);
    for (n, id) in pages.iter().take(fixture_pages) {
        let d = doc.get_dictionary(*id).unwrap();
        let mb = d.get(b"MediaBox").unwrap().as_array().unwrap();
        assert_eq!(mb[2].as_i64().unwrap(), 300, "page {n} MediaBox");
        assert_eq!(mb[3].as_i64().unwrap(), 400, "page {n} MediaBox");
        let res = doc
            .dereference(d.get(b"Resources").unwrap())
            .unwrap()
            .1
            .as_dict()
            .unwrap();
        let fonts = doc
            .dereference(res.get(b"Font").unwrap())
            .unwrap()
            .1
            .as_dict()
            .unwrap();
        let (_, font) = doc.dereference(fonts.get(b"F1").unwrap()).unwrap();
        assert_eq!(
            font.as_dict()
                .unwrap()
                .get(b"BaseFont")
                .unwrap()
                .as_name()
                .unwrap(),
            b"Courier",
            "page {n} font resolves"
        );
        let content = doc.get_page_content(*id);
        assert!(!content.is_empty(), "page {n} content");
    }
}

#[test]
fn merge_keeps_inherited_page_attributes() {
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("nested.pdf");
    nested_pdf(&nested);
    assert_eq!(pdf::page_count(&nested).unwrap(), 3);
    let flat = make_pdf(dir.path(), "flat.pdf", 2);
    let out = dir.path().join("m.pdf");
    pdf::merge(&[nested, flat], &out, &tok(), &|_| {}).unwrap();
    assert_pages_self_contained(&out, 5, 3);
}

#[test]
fn split_keeps_inherited_page_attributes() {
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("nested.pdf");
    nested_pdf(&nested);
    let out_dir = dir.path().join("parts");
    std::fs::create_dir(&out_dir).unwrap();
    let files = pdf::split(&nested, &out_dir, &tok(), &|_| {}).unwrap();
    assert_eq!(files.len(), 3);
    for f in &files {
        assert_pages_self_contained(f, 1, 1);
    }
    // Each part holds only its own page's text.
    for (i, f) in files.iter().enumerate() {
        let d = lopdf::Document::load(f).unwrap();
        let id = *d.get_pages().values().next().unwrap();
        let c = String::from_utf8_lossy(&d.get_page_content(id)).into_owned();
        assert!(c.contains(&format!("page {}", i + 1)), "{c}");
        assert!(!c.contains(&format!("page {}", (i + 1) % 3 + 1)), "{c}");
    }
}

#[test]
fn csv_rejects_duplicate_headers_and_huge_tables() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("d.csv");
    std::fs::write(&input, "a,b,a\n1,2,3\n").unwrap();
    let err = data::convert(&input, "csv", "json", &dir.path().join("o.json")).unwrap_err();
    assert!(
        matches!(&err, Error::Failed(m) if m == "duplicate CSV column: a"),
        "{err:?}"
    );

    // 1000 columns x 1001 rows > 1,000,000 cells
    let header: Vec<String> = (0..1000).map(|i| format!("c{i}")).collect();
    let mut text = header.join(",");
    text.push('\n');
    let row = vec!["x"; 1000].join(",");
    for _ in 0..1001 {
        text.push_str(&row);
        text.push('\n');
    }
    let big = dir.path().join("big.csv");
    std::fs::write(&big, text).unwrap();
    let err = data::convert(&big, "csv", "json", &dir.path().join("big.json")).unwrap_err();
    assert!(matches!(err, Error::TooLarge(_)), "{err:?}");
    assert!(!dir.path().join("big.json").exists());
}

#[test]
fn split_does_not_drag_in_pages_reached_by_links() {
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("nested.pdf");
    nested_pdf(&nested);
    let out_dir = dir.path().join("parts");
    std::fs::create_dir(&out_dir).unwrap();
    let files = pdf::split(&nested, &out_dir, &tok(), &|_| {}).unwrap();
    let part = lopdf::Document::load(&files[0]).unwrap();
    let page_objects = part
        .objects
        .values()
        .filter(|o| o.type_name().ok() == Some(b"Page".as_slice()))
        .count();
    assert_eq!(page_objects, 1);
    assert!(
        !part
            .objects
            .values()
            .any(|o| o.type_name().ok() == Some(b"Pages".as_slice())
                && o.as_dict().unwrap().has(b"MediaBox")),
        "original page tree must not be copied"
    );
    let src_len = std::fs::metadata(&nested).unwrap().len();
    assert!(std::fs::metadata(&files[0]).unwrap().len() < src_len);
}

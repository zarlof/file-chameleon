//! PDF engine. lopdf (in-process): images -> pdf, merge, split, compress.
//! pdfium (on-demand dll, dynamic binding): pdf -> raster pages. OWNER: Task B.

use crate::engines::image as raster;
use crate::jobs::ConvertOptions;
use crate::{CancelToken, Error, Result};
use ::image::{ImageDecoder, ImageReader};
use lopdf::{Dictionary, Document, Object, ObjectId, SaveOptions, Stream};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const DPI: f32 = 150.0;
const RENDER_MAX_LONG_SIDE: f32 = 4096.0;

fn pdf_error(e: lopdf::Error) -> Error {
    match e {
        lopdf::Error::IO(io) => Error::Io(io),
        other => Error::Failed(format!("invalid or unsupported PDF: {other}")),
    }
}

fn check_cancel(cancel: &CancelToken) -> Result<()> {
    if cancel.is_cancelled() {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}

fn load(path: &Path) -> Result<Document> {
    if !path.exists() {
        return Err(Error::NotFound(path.to_path_buf()));
    }
    let doc = Document::load(path).map_err(pdf_error)?;
    if doc.is_encrypted() {
        return Err(Error::Unsupported("password-protected PDF".into()));
    }
    Ok(doc)
}

fn int_array(vals: [i64; 4]) -> Object {
    Object::Array(vals.into_iter().map(Object::Integer).collect())
}

/// Value of an inheritable page attribute, looked up through the `Parent` chain.
fn inherited(doc: &Document, page: ObjectId, key: &[u8]) -> Option<Object> {
    let mut current = page;
    for _ in 0..64 {
        let node = doc.get_dictionary(current).ok()?;
        if let Ok(v) = node.get(key) {
            return Some(v.clone());
        }
        current = node.get(b"Parent").ok()?.as_reference().ok()?;
    }
    None
}

/// Copy inherited attributes onto the page itself so it survives being moved to a new tree.
fn materialize_page(doc: &mut Document, page: ObjectId) {
    let media_box =
        inherited(doc, page, b"MediaBox").unwrap_or_else(|| int_array([0, 0, 612, 792]));
    let resources = inherited(doc, page, b"Resources");
    let crop_box = inherited(doc, page, b"CropBox");
    let rotate = inherited(doc, page, b"Rotate");
    if let Some(Object::Dictionary(d)) = doc.objects.get_mut(&page) {
        d.set("MediaBox", media_box);
        d.set(
            "Resources",
            resources.unwrap_or_else(|| Object::Dictionary(Dictionary::new())),
        );
        if let Some(c) = crop_box {
            d.set("CropBox", c);
        }
        if let Some(r) = rotate {
            d.set("Rotate", r);
        }
    }
}

fn save_plain(doc: &mut Document, output: &Path) -> Result<()> {
    let r = (|| -> Result<()> {
        let mut w = BufWriter::new(File::create(output)?);
        doc.save_to(&mut w)?;
        w.flush()?;
        Ok(())
    })();
    if r.is_err() {
        let _ = std::fs::remove_file(output);
    }
    r
}

/// Accumulates pages (from images and/or other PDFs) into one new document.
struct Builder {
    doc: Document,
    pages_id: ObjectId,
    kids: Vec<ObjectId>,
}

impl Builder {
    fn new() -> Self {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        Builder {
            doc,
            pages_id,
            kids: Vec::new(),
        }
    }

    /// One page sized to the image (1 px = 1 pt). JPEG is embedded as-is.
    fn add_image(&mut self, path: &Path) -> Result<()> {
        let reader = ImageReader::open(path)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Error::NotFound(path.to_path_buf())
                } else {
                    Error::Io(e)
                }
            })?
            .with_guessed_format()?;
        let is_jpeg = reader.format() == Some(::image::ImageFormat::Jpeg);

        let (w, h, stream) = match self.try_embed_jpeg(path, is_jpeg)? {
            Some(v) => v,
            None => {
                let img = raster::decode(path)?;
                let rgb = raster::flatten_rgb(&img);
                let (w, h) = (rgb.width(), rgb.height());
                drop(img);
                let dict = image_dict(w, h, "DeviceRGB", 8);
                let mut stream = Stream::new(dict, rgb.into_raw());
                let _ = stream.compress();
                (w, h, stream)
            }
        };

        let image_id = self.doc.add_object(stream);
        let content = format!("q {w} 0 0 {h} 0 0 cm /Im0 Do Q");
        let content_id = self
            .doc
            .add_object(Stream::new(Dictionary::new(), content.into_bytes()));
        let mut xobjects = Dictionary::new();
        xobjects.set("Im0", Object::Reference(image_id));
        let mut resources = Dictionary::new();
        resources.set("XObject", Object::Dictionary(xobjects));
        let mut page = Dictionary::new();
        page.set("Type", "Page");
        page.set("MediaBox", int_array([0, 0, i64::from(w), i64::from(h)]));
        page.set("Resources", Object::Dictionary(resources));
        page.set("Contents", Object::Reference(content_id));
        let page_id = self.doc.add_object(Object::Dictionary(page));
        self.kids.push(page_id);
        Ok(())
    }

    /// Raw JPEG bytes as a DCTDecode image, when it is plain 8-bit gray/RGB, unrotated.
    fn try_embed_jpeg(&self, path: &Path, is_jpeg: bool) -> Result<Option<(u32, u32, Stream)>> {
        if !is_jpeg {
            return Ok(None);
        }
        let reader = ImageReader::open(path)?.with_guessed_format()?;
        let Ok(mut decoder) = reader.into_decoder() else {
            return Ok(None);
        };
        let (w, h) = decoder.dimensions();
        raster::check_dimensions(w, h)?;
        let space = match decoder.color_type() {
            ::image::ColorType::L8 => "DeviceGray",
            ::image::ColorType::Rgb8 => "DeviceRGB",
            _ => return Ok(None),
        };
        if !matches!(
            decoder.orientation(),
            Ok(::image::metadata::Orientation::NoTransforms)
        ) {
            return Ok(None);
        }
        drop(decoder);
        let bytes = std::fs::read(path)?;
        let mut dict = image_dict(w, h, space, 8);
        dict.set("Filter", "DCTDecode");
        let stream = Stream::new(dict, bytes).with_compression(false);
        Ok(Some((w, h, stream)))
    }

    /// Append every page of a PDF.
    fn add_pdf(&mut self, path: &Path) -> Result<()> {
        let mut src = load(path)?;
        let ids: Vec<ObjectId> = src.get_pages().into_values().collect();
        if ids.is_empty() {
            return Err(Error::Failed("PDF has no pages".into()));
        }
        for id in ids {
            materialize_page(&mut src, id);
        }
        src.renumber_objects_with(self.doc.max_id + 1);
        let pages: Vec<ObjectId> = src.get_pages().into_values().collect();
        let max_id = src.max_id;
        for (id, obj) in std::mem::take(&mut src.objects) {
            let skip = matches!(
                obj.type_name().unwrap_or(b""),
                b"Catalog" | b"Pages" | b"Outlines" | b"Outline" | b"ObjStm" | b"XRef"
            );
            if !skip {
                self.doc.objects.insert(id, obj);
            }
        }
        self.doc.max_id = self.doc.max_id.max(max_id);
        self.kids.extend(pages);
        Ok(())
    }

    fn finish(mut self, output: &Path) -> Result<()> {
        if self.kids.is_empty() {
            return Err(Error::Failed("nothing to write".into()));
        }
        for kid in &self.kids {
            if let Some(Object::Dictionary(d)) = self.doc.objects.get_mut(kid) {
                d.set("Parent", Object::Reference(self.pages_id));
            }
        }
        let mut pages = Dictionary::new();
        pages.set("Type", "Pages");
        pages.set(
            "Kids",
            Object::Array(self.kids.iter().map(|k| Object::Reference(*k)).collect()),
        );
        pages.set("Count", Object::Integer(self.kids.len() as i64));
        self.doc
            .objects
            .insert(self.pages_id, Object::Dictionary(pages));
        let mut catalog = Dictionary::new();
        catalog.set("Type", "Catalog");
        catalog.set("Pages", Object::Reference(self.pages_id));
        let catalog_id = self.doc.add_object(Object::Dictionary(catalog));
        self.doc.trailer.set("Root", Object::Reference(catalog_id));
        self.doc.prune_objects();
        self.doc.compress();
        save_plain(&mut self.doc, output)
    }
}

fn image_dict(w: u32, h: u32, space: &str, bpc: i64) -> Dictionary {
    let mut d = Dictionary::new();
    d.set("Type", "XObject");
    d.set("Subtype", "Image");
    d.set("Width", Object::Integer(i64::from(w)));
    d.set("Height", Object::Integer(i64::from(h)));
    d.set("ColorSpace", space);
    d.set("BitsPerComponent", Object::Integer(bpc));
    d
}

/// Object ids referenced from `obj` (recursing through arrays, dictionaries, stream dicts).
fn collect_refs(obj: &Object, skip_parent: bool, out: &mut Vec<ObjectId>) {
    match obj {
        Object::Reference(id) => out.push(*id),
        Object::Array(a) => a.iter().for_each(|o| collect_refs(o, false, out)),
        Object::Dictionary(d) => {
            for (k, v) in d.iter() {
                if !(skip_parent && k.as_slice() == b"Parent") {
                    collect_refs(v, false, out);
                }
            }
        }
        Object::Stream(s) => {
            for (_, v) in s.dict.iter() {
                collect_refs(v, false, out);
            }
        }
        _ => {}
    }
}

/// A new document holding only `page` and the objects reachable from it (its page-tree
/// `Parent` link is not followed; attributes were materialized onto the page beforehand).
fn single_page_doc(src: &Document, page: ObjectId) -> Document {
    let mut objects: std::collections::BTreeMap<ObjectId, Object> = Default::default();
    let mut stack = vec![page];
    while let Some(id) = stack.pop() {
        if objects.contains_key(&id) {
            continue;
        }
        let Some(obj) = src.objects.get(&id) else {
            continue;
        };
        let mut refs = Vec::new();
        collect_refs(obj, id == page, &mut refs);
        stack.extend(refs);
        objects.insert(id, obj.clone());
    }
    let mut doc = Document::with_version("1.5");
    doc.max_id = src.max_id;
    doc.objects = objects;
    let pages_id = doc.new_object_id();
    if let Some(Object::Dictionary(d)) = doc.objects.get_mut(&page) {
        d.set("Parent", Object::Reference(pages_id));
    }
    let mut pages = Dictionary::new();
    pages.set("Type", "Pages");
    pages.set("Kids", Object::Array(vec![Object::Reference(page)]));
    pages.set("Count", Object::Integer(1));
    doc.objects.insert(pages_id, Object::Dictionary(pages));
    let mut catalog = Dictionary::new();
    catalog.set("Type", "Catalog");
    catalog.set("Pages", Object::Reference(pages_id));
    let catalog_id = doc.add_object(Object::Dictionary(catalog));
    doc.trailer.set("Root", Object::Reference(catalog_id));
    doc
}

const MAX_TOTAL_INPUT_BYTES: u64 = 1024 * 1024 * 1024;

/// lopdf keeps every page's bytes in memory until the file is written, so refuse inputs
/// that total more than 1 GB.
fn check_total_size(inputs: &[PathBuf]) -> Result<()> {
    let mut total = 0u64;
    for p in inputs {
        total = total.saturating_add(std::fs::metadata(p).map(|m| m.len()).unwrap_or(0));
    }
    if total > MAX_TOTAL_INPUT_BYTES {
        return Err(Error::TooLarge("inputs total more than 1 GB".into()));
    }
    Ok(())
}

/// Raster images -> one PDF, one image per page, page size = image size at 72 dpi
/// (so 1 px = 1 pt), in input order. JPEG embedded as-is (DCTDecode); everything else
/// decoded and embedded as Flate RGB (alpha flattened onto white).
///
/// Known limit: lopdf holds every page's (compressed) bytes in memory until the file is
/// written, so the inputs may total at most 1 GB (`Error::TooLarge` beyond that).
pub fn images_to_pdf(
    inputs: &[PathBuf],
    output: &Path,
    cancel: &CancelToken,
    progress: &dyn Fn(f32),
) -> Result<()> {
    if inputs.is_empty() {
        return Err(Error::Failed("no input images".into()));
    }
    check_total_size(inputs)?;
    let mut b = Builder::new();
    for (i, p) in inputs.iter().enumerate() {
        check_cancel(cancel)?;
        b.add_image(p)?;
        progress((i + 1) as f32 / (inputs.len() + 1) as f32);
    }
    check_cancel(cancel)?;
    b.finish(output)?;
    progress(1.0);
    Ok(())
}

/// PDFs and/or raster images (in order) -> one PDF.
///
/// Known limit: all merged objects stay in memory until the output is written (lopdf), so
/// the inputs may total at most 1 GB (`Error::TooLarge` beyond that).
pub fn merge(
    inputs: &[PathBuf],
    output: &Path,
    cancel: &CancelToken,
    progress: &dyn Fn(f32),
) -> Result<()> {
    if inputs.is_empty() {
        return Err(Error::Failed("no inputs to merge".into()));
    }
    check_total_size(inputs)?;
    let mut b = Builder::new();
    for (i, p) in inputs.iter().enumerate() {
        check_cancel(cancel)?;
        let is_pdf = crate::formats::detect_path(p).is_some_and(|f| f.id == "pdf");
        if is_pdf {
            b.add_pdf(p)?;
        } else {
            b.add_image(p)?;
        }
        progress((i + 1) as f32 / (inputs.len() + 1) as f32);
    }
    check_cancel(cancel)?;
    b.finish(output)?;
    progress(1.0);
    Ok(())
}

/// One PDF -> `<stem>-<n>.pdf` per page inside existing directory `out_dir`. Returns files in page order.
pub fn split(
    input: &Path,
    out_dir: &Path,
    cancel: &CancelToken,
    progress: &dyn Fn(f32),
) -> Result<Vec<PathBuf>> {
    let mut src = load(input)?;
    let pages: Vec<u32> = src.get_pages().into_keys().collect();
    if pages.is_empty() {
        return Err(Error::Failed("PDF has no pages".into()));
    }
    let page_ids: Vec<ObjectId> = src.get_pages().into_values().collect();
    for id in &page_ids {
        materialize_page(&mut src, *id);
    }
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "page".into());
    let mut written: Vec<PathBuf> = Vec::with_capacity(pages.len());
    let r = (|| -> Result<()> {
        for (i, page_id) in page_ids.iter().enumerate() {
            check_cancel(cancel)?;
            let mut one = single_page_doc(&src, *page_id);
            let path = out_dir.join(format!("{stem}-{}.pdf", i + 1));
            written.push(path.clone());
            save_plain(&mut one, &path)?;
            progress((i + 1) as f32 / pages.len() as f32);
        }
        Ok(())
    })();
    if let Err(e) = r {
        for p in &written {
            let _ = std::fs::remove_file(p);
        }
        return Err(e);
    }
    Ok(written)
}

/// Rewrite with object streams + compressed streams, drop unused objects.
/// If the rewrite is not smaller than the original, the original bytes are copied instead.
pub fn compress(input: &Path, output: &Path) -> Result<()> {
    let mut doc = load(input)?;
    doc.prune_objects();
    doc.delete_zero_length_streams();
    doc.compress();
    let opts = SaveOptions::builder()
        .use_object_streams(true)
        .use_xref_streams(true)
        .build();
    let r = (|| -> Result<()> {
        let mut w = BufWriter::new(File::create(output)?);
        doc.save_with_options(&mut w, opts)?;
        w.flush()?;
        Ok(())
    })();
    if let Err(e) = r {
        let _ = std::fs::remove_file(output);
        return Err(e);
    }
    if std::fs::metadata(output)?.len() >= std::fs::metadata(input)?.len() {
        std::fs::copy(input, output)?;
    }
    Ok(())
}

/// Number of pages (used by the executor to decide single file vs folder).
pub fn page_count(input: &Path) -> Result<usize> {
    if !input.exists() {
        return Err(Error::NotFound(input.to_path_buf()));
    }
    let meta = Document::load_metadata(input).map_err(pdf_error)?;
    Ok(meta.page_count as usize)
}

static PDFIUM: OnceLock<pdfium_render::prelude::Pdfium> = OnceLock::new();

/// pdfium can be bound once per process; later calls reuse the first binding.
fn pdfium(dll: &Path) -> Result<&'static pdfium_render::prelude::Pdfium> {
    use pdfium_render::prelude::{Pdfium, PdfiumError};
    if let Some(p) = PDFIUM.get() {
        return Ok(p);
    }
    match Pdfium::bind_to_library(dll) {
        Ok(b) => {
            let _ = PDFIUM.set(Pdfium::new(b));
        }
        Err(PdfiumError::PdfiumLibraryBindingsAlreadyInitialized) => {}
        Err(_) => return Err(Error::MissingTool("pdfium".into())),
    }
    PDFIUM
        .get()
        .ok_or_else(|| Error::MissingTool("pdfium".into()))
}

/// Pixel size for a page of `w_pt` x `h_pt` points: 150 dpi, long side <= 4096, then the
/// `max_width`/`max_height` fit.
pub(crate) fn render_size(w_pt: f32, h_pt: f32, options: &ConvertOptions) -> (u32, u32) {
    let k = DPI / 72.0;
    let (mut w, mut h) = (w_pt * k, h_pt * k);
    let long = w.max(h);
    if long > RENDER_MAX_LONG_SIDE {
        let s = RENDER_MAX_LONG_SIDE / long;
        w *= s;
        h *= s;
    }
    raster::fit_dimensions(
        (w.round() as u32).max(1),
        (h.round() as u32).max(1),
        options,
    )
}

/// Render every page with pdfium loaded from `pdfium_dll`, at 150 dpi (longer side capped
/// at 4096 px, then `max_width`/`max_height` fit), encoded via `image::encode` to `to`
/// ("png" | "jpg"). `outputs[i]` is the destination for page i (len must equal page count).
pub fn render_pages(
    pdfium_dll: &Path,
    input: &Path,
    to: &str,
    outputs: &[PathBuf],
    options: &ConvertOptions,
    cancel: &CancelToken,
    progress: &dyn Fn(f32),
) -> Result<()> {
    use pdfium_render::prelude::PdfRenderConfig;
    check_cancel(cancel)?;
    if !input.exists() {
        return Err(Error::NotFound(input.to_path_buf()));
    }
    let pdfium = pdfium(pdfium_dll)?;
    let document = pdfium
        .load_pdf_from_file(input, None)
        .map_err(|e| Error::Failed(format!("could not open PDF: {e}")))?;
    let pages = document.pages();
    let count = pages.len() as usize;
    if count != outputs.len() {
        return Err(Error::Failed(format!(
            "page count mismatch: {count} pages, {} outputs",
            outputs.len()
        )));
    }
    let encode_opts = ConvertOptions {
        max_width: None,
        max_height: None,
        ..options.clone()
    };
    let mut done: Vec<&PathBuf> = Vec::new();
    let r = (|| -> Result<()> {
        for (i, page) in pages.iter().enumerate() {
            check_cancel(cancel)?;
            let (w, h) = render_size(page.width().value, page.height().value, options);
            let config = PdfRenderConfig::new().set_target_size(w as i32, h as i32);
            let img = page
                .render_with_config(&config)
                .and_then(|b| b.as_image())
                .map_err(|e| Error::Failed(format!("render failed: {e}")))?;
            raster::encode(&img, to, &outputs[i], &encode_opts)?;
            done.push(&outputs[i]);
            progress((i + 1) as f32 / count as f32);
        }
        Ok(())
    })();
    if r.is_err() {
        for p in done {
            let _ = std::fs::remove_file(p);
        }
    }
    r
}

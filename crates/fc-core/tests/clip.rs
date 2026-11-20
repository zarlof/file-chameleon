//! Public-API checks for the clipboard history (more detailed unit tests live in history.rs).

use fc_core::clip::{ClipKind, ClipStore};

fn png(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(w, h, image::Rgba([200, 10, 10, 255]));
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

#[test]
fn mixed_history_survives_reopen_and_clear_keeps_pins() {
    let dir = tempfile::tempdir().unwrap();
    let (text_id, img_id) = {
        let mut s = ClipStore::open(dir.path()).unwrap();
        let t = s.add_text("remember me", 10).unwrap().unwrap();
        let i = s.add_image_png(&png(40, 20), 40, 20, 20).unwrap().unwrap();
        (t.id, i.id)
    };
    let mut s = ClipStore::open(dir.path()).unwrap();
    assert_eq!(s.items().len(), 2);
    assert_eq!(s.items()[0].id, img_id);
    assert_eq!(s.items()[0].kind, ClipKind::Image);
    assert!(s.set_pinned(text_id, true).unwrap());
    s.clear_unpinned().unwrap();
    let s = ClipStore::open(dir.path()).unwrap();
    assert_eq!(s.items().len(), 1);
    assert_eq!(s.items()[0].id, text_id);
    assert!(s.items()[0].pinned);
}

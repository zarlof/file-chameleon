//! Public-API checks for the clipboard store (more detailed unit tests live in history.rs).

use fc_core::clip::{ClipKind, ClipQuery, ClipStore, Field, Retention, Shelf};

const DAY: u64 = 86_400_000;

fn png(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(w, h, image::Rgba([200, 10, 10, 255]));
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

fn live(s: &ClipStore) -> Vec<fc_core::clip::ClipItem> {
    s.query(&ClipQuery::default())
}

fn ret(text_days: u32, max_items: usize) -> Retention {
    Retention {
        text_days,
        links_days: text_days,
        images_days: text_days,
        max_items,
    }
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
    assert_eq!(live(&s).len(), 2);
    assert_eq!(live(&s)[0].id, img_id);
    assert_eq!(live(&s)[0].kind, ClipKind::Image);
    s.set_pinned(&[text_id], true).unwrap();
    s.add_text("junk", 30).unwrap();
    assert_eq!(s.clear_unsaved(40).unwrap(), 2);
    let s = ClipStore::open(dir.path()).unwrap();
    assert_eq!(live(&s).len(), 1);
    assert_eq!(live(&s)[0].id, text_id);
    assert!(live(&s)[0].pinned && live(&s)[0].saved());
    assert_eq!(s.stats().trash, 2);
}

#[test]
fn v1_index_migrates() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("history.json"),
        r#"{"items":[{"id":4,"kind":"text","text":"old note","image":null,"thumb":null,"width":null,"height":null,"site":null,"hash":"00ff","createdAt":5,"pinned":false}]}"#,
    )
    .unwrap();
    let mut s = ClipStore::open(dir.path()).unwrap();
    let it = &live(&s)[0];
    assert!(it.tags.is_empty() && !it.saved());
    assert_eq!(it.size, 8);
    assert!(it.trashed_at.is_none() && it.folder.is_none());
    assert!(s.add_text("next", 6).unwrap().unwrap().id > 4);
    let json = serde_json::to_value(&live(&s)[0]).unwrap();
    assert_eq!(json["saved"], false);
    assert!(json.get("createdAt").is_some() && json.get("trashedAt").is_some());
}

#[test]
fn serialized_keys_match_the_typescript_clip_item() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../src/lib/types.ts"
    ))
    .unwrap();
    let body = src
        .split("export interface ClipItem {")
        .nth(1)
        .and_then(|rest| rest.split("\n}").next())
        .unwrap();
    let mut ts: Vec<String> = body
        .lines()
        .filter(|l| l.starts_with("  ") && !l.trim_start().starts_with(['/', '*']))
        .filter_map(|l| l.trim().split([':', '?']).next().map(str::to_string))
        .collect();
    ts.sort();
    assert_eq!(ts.len(), 23, "{ts:?}");

    let (_d, mut s) = store();
    let item = s.add_text("x", 1).unwrap().unwrap();
    let json = serde_json::to_value(&item).unwrap();
    let mut rust: Vec<String> = json.as_object().unwrap().keys().cloned().collect();
    rust.sort();
    assert_eq!(rust, ts);
}

#[test]
fn blank_and_oversized_text_are_skipped() {
    let (_d, mut s) = store();
    assert!(s.add_text("  \n\t", 1).unwrap().is_none());
    assert!(
        s.add_text(&"x".repeat(1024 * 1024 + 1), 1)
            .unwrap()
            .is_none()
    );
    assert!(live(&s).is_empty());
    assert!(s.add_text(&"x".repeat(1024 * 1024), 1).unwrap().is_some());
}

#[test]
fn duplicate_refreshes_and_restores_from_trash() {
    let (_d, mut s) = store();
    let a = s.add_text("a", 1).unwrap().unwrap();
    s.add_text("b", 2).unwrap();
    s.trash(&[a.id], 3).unwrap();
    assert_eq!(live(&s).len(), 1);
    let again = s.add_text("a", 4).unwrap().unwrap();
    assert_eq!(again.id, a.id);
    assert_eq!(again.trashed_at, None);
    assert_eq!(live(&s)[0].id, a.id);
    assert_eq!(live(&s).len(), 2);
}

#[test]
fn query_filters_and_search() {
    let (_d, mut s) = store();
    let link = s.add_text("https://example.com/page", 1).unwrap().unwrap();
    let media = s
        .add_text("https://www.youtube.com/watch?v=abc", 2)
        .unwrap()
        .unwrap();
    let text = s.add_text("plain words", 3).unwrap().unwrap();
    let img = s.add_image_png(&png(8, 8), 8, 8, 4).unwrap().unwrap();
    assert_eq!(link.kind, ClipKind::Link);
    assert_eq!(media.kind, ClipKind::MediaLink);
    let q = |shelf| ClipQuery {
        shelf: Some(shelf),
        ..Default::default()
    };
    assert_eq!(s.query(&q(Shelf::Links)).len(), 2);
    assert_eq!(s.query(&q(Shelf::Texts)).len(), 1);
    assert_eq!(s.query(&q(Shelf::Images))[0].id, img.id);

    s.set_field(text.id, Field::Title, "İSTANBUL gezisi")
        .unwrap();
    for needle in ["istanbul", "ISTANBUL", "ıstanbul", "İstanbul"] {
        let hits = s.query(&ClipQuery {
            text: Some(needle.into()),
            ..Default::default()
        });
        assert_eq!(hits.len(), 1, "{needle}");
    }
    s.tag(&[link.id], &["Work".into()], &[]).unwrap();
    let by_tag = s.query(&ClipQuery {
        tag: Some("work".into()),
        ..Default::default()
    });
    assert_eq!(by_tag.len(), 1);
    let f = s.folder_create("Trips", None).unwrap();
    s.move_to(&[media.id], Some(f.id)).unwrap();
    let by_folder = s.query(&ClipQuery {
        folder: Some(f.id),
        ..Default::default()
    });
    assert_eq!(by_folder[0].id, media.id);
    let saved = s.query(&ClipQuery {
        saved: true,
        ..Default::default()
    });
    assert_eq!(saved.len(), 3);
    s.trash(&[img.id], 9).unwrap();
    assert_eq!(live(&s).len(), 3);
    let trash = s.query(&ClipQuery {
        trash: true,
        ..Default::default()
    });
    assert_eq!(trash.len(), 1);
    let limited = s.query(&ClipQuery {
        limit: Some(2),
        ..Default::default()
    });
    assert_eq!(limited.len(), 2);
}

#[test]
fn saved_items_survive_prune() {
    let (_d, mut s) = store();
    let pinned = s.add_text("pinned old", 0).unwrap().unwrap();
    let tagged = s.add_text("tagged old", 1).unwrap().unwrap();
    let foldered = s.add_text("foldered old", 2).unwrap().unwrap();
    s.add_text("plain old", 3).unwrap();
    s.set_pinned(&[pinned.id], true).unwrap();
    s.tag(&[tagged.id], &["x".into()], &[]).unwrap();
    let f = s.folder_create("F", None).unwrap();
    s.move_to(&[foldered.id], Some(f.id)).unwrap();
    let now = 60 * DAY;
    assert_eq!(s.prune(now, &ret(30, 100)).unwrap(), 1);
    assert_eq!(live(&s).len(), 3);
    // 0 days = forever
    s.add_text("plain old 2", 4).unwrap();
    assert_eq!(s.prune(now, &ret(0, 100)).unwrap(), 0);
    assert_eq!(live(&s).len(), 4);
    // max_items drops the oldest unsaved only
    s.add_text("n1", now).unwrap();
    s.add_text("n2", now + 1).unwrap();
    s.prune(now + 2, &ret(0, 2)).unwrap();
    let texts: Vec<_> = live(&s).iter().map(|i| i.text.clone().unwrap()).collect();
    assert_eq!(
        texts,
        ["n2", "n1", "foldered old", "tagged old", "pinned old"]
    );
}

#[test]
fn old_trash_is_deleted_for_good_by_prune() {
    let (_d, mut s) = store();
    let a = s.add_text("a", 0).unwrap().unwrap();
    s.trash(&[a.id], 0).unwrap();
    assert_eq!(s.prune(29 * DAY, &ret(0, 10)).unwrap(), 0);
    assert_eq!(s.stats().trash, 1);
    assert_eq!(s.prune(31 * DAY, &ret(0, 10)).unwrap(), 1);
    assert_eq!(s.stats().trash, 0);
    assert!(s.get(a.id).is_none());
}

#[test]
fn trash_restore_and_empty() {
    let (_d, mut s) = store();
    let img = s.add_image_png(&png(10, 10), 10, 10, 1).unwrap().unwrap();
    let (i, t) = (img.image.clone().unwrap(), img.thumb.clone().unwrap());
    s.trash(&[img.id], 5).unwrap();
    assert!(live(&s).is_empty());
    s.restore(&[img.id]).unwrap();
    assert_eq!(live(&s).len(), 1);
    s.trash(&[img.id], 6).unwrap();
    assert!(i.exists());
    assert_eq!(s.empty_trash().unwrap(), 1);
    assert!(!i.exists() && !t.exists());
    assert!(s.get(img.id).is_none());
}

#[test]
fn folders_and_tags() {
    let (_d, mut s) = store();
    let a = s.add_text("a", 1).unwrap().unwrap();
    let b = s.add_text("b", 2).unwrap().unwrap();
    let parent = s.folder_create("Parent", None).unwrap();
    let child = s.folder_create("Child", Some(parent.id)).unwrap();
    let grand = s.folder_create("Grand", Some(child.id)).unwrap();
    s.move_to(&[a.id], Some(parent.id)).unwrap();
    s.move_to(&[b.id], Some(child.id)).unwrap();
    s.folder_rename(child.id, "Kid").unwrap();
    let infos = s.folders();
    assert_eq!(infos.iter().find(|f| f.id == parent.id).unwrap().count, 1);
    assert_eq!(infos.iter().find(|f| f.id == child.id).unwrap().name, "Kid");
    s.folder_delete(parent.id).unwrap();
    assert_eq!(s.get(a.id).unwrap().folder, None);
    let infos = s.folders();
    assert_eq!(infos.len(), 2);
    assert_eq!(
        infos.iter().find(|f| f.id == child.id).unwrap().parent,
        None
    );
    assert_eq!(
        infos.iter().find(|f| f.id == grand.id).unwrap().parent,
        Some(child.id)
    );
    assert!(s.move_to(&[a.id], Some(999)).is_err());

    s.tag(
        &[a.id, b.id],
        &["Work".into(), " work ".into(), "Home".into()],
        &[],
    )
    .unwrap();
    assert_eq!(s.get(a.id).unwrap().tags, ["Work", "Home"]);
    s.tag(&[b.id], &["Work".into()], &["home".into()]).unwrap();
    let tags = s.tags();
    assert_eq!((tags[0].tag.as_str(), tags[0].count), ("Work", 2));
    assert_eq!((tags[1].tag.as_str(), tags[1].count), ("Home", 1));
}

#[test]
fn edit_and_revert_keep_original() {
    let (_d, mut s) = store();
    let img = s.add_image_png(&png(20, 20), 20, 20, 1).unwrap().unwrap();
    let orig = std::fs::read(img.image.as_ref().unwrap()).unwrap();
    let edited = s.save_edited(img.id, &png(10, 10)).unwrap();
    let ep = edited.edited.clone().unwrap();
    assert!(ep.exists() && edited.edited_thumb.as_ref().unwrap().exists());
    assert_eq!(std::fs::read(edited.image.as_ref().unwrap()).unwrap(), orig);
    let reverted = s.revert_edit(img.id).unwrap();
    assert!(reverted.edited.is_none() && reverted.edited_thumb.is_none());
    assert!(!ep.exists());
    assert_eq!(
        std::fs::read(reverted.image.as_ref().unwrap()).unwrap(),
        orig
    );
    let t = s.add_text("t", 2).unwrap().unwrap();
    assert!(s.save_edited(t.id, &png(2, 2)).is_err());
}

#[test]
fn ocr_queue_and_fields() {
    let (_d, mut s) = store();
    let img = s.add_image_png(&png(8, 8), 8, 8, 1).unwrap().unwrap();
    let t = s.add_text("hello", 2).unwrap().unwrap();
    assert_eq!(s.needs_ocr().len(), 1);
    s.set_ocr(img.id, "scanned words".into()).unwrap();
    assert!(s.needs_ocr().is_empty());
    let hits = s.query(&ClipQuery {
        text: Some("SCANNED".into()),
        ..Default::default()
    });
    assert_eq!(hits.len(), 1);
    s.set_link_title(t.id, "A page".into()).unwrap();
    let it = s.set_field(t.id, Field::Note, "  ").unwrap();
    assert!(it.note.is_none());
    let old_hash = s.get(t.id).unwrap().hash.clone();
    let it = s.set_field(t.id, Field::Text, "changed").unwrap();
    assert_ne!(it.hash, old_hash);
    assert!(s.set_field(img.id, Field::Text, "x").is_err());
    assert!(s.set_field(99, Field::Title, "x").is_err());
}

#[test]
fn export_writes_unique_files() {
    let (_d, mut s) = store();
    let a = s.add_text("first body", 1).unwrap().unwrap();
    let b = s.add_text("second body", 2).unwrap().unwrap();
    s.set_field(a.id, Field::Title, "Note").unwrap();
    s.set_field(b.id, Field::Title, "Note").unwrap();
    let img = s.add_image_png(&png(6, 6), 6, 6, 3).unwrap().unwrap();
    let out = tempfile::tempdir().unwrap();
    let n = s.export(&[a.id, b.id, img.id], out.path()).unwrap();
    assert_eq!(n, 3);
    let mut names: Vec<_> = std::fs::read_dir(out.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names.len(), 3);
    assert!(names.contains(&"Note.txt".to_string()));
    assert!(names.contains(&"Note (1).txt".to_string()));
    assert!(
        names
            .iter()
            .any(|n| n.starts_with("Görsel ") && n.ends_with(".png"))
    );
}

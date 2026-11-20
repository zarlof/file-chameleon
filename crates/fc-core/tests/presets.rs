//! Presets, groups and wheel profiles (`fc_core::presets`).

use fc_core::Error;
use fc_core::formats::Kind;
use fc_core::jobs::{ConvertOptions, ImageOp, Operation};
use fc_core::presets::{Face, Preset, Profile, WheelConfig, WheelSlot, applicable};
use serde_json::json;
use std::path::PathBuf;

fn paths(names: &[&str]) -> Vec<PathBuf> {
    names.iter().map(PathBuf::from).collect()
}

fn ids(slots: &[WheelSlot]) -> Vec<&str> {
    slots.iter().map(|s| s.id.as_str()).collect()
}

fn profile<'a>(cfg: &'a WheelConfig, id: &str) -> &'a Profile {
    cfg.profiles
        .iter()
        .find(|p| p.id == id)
        .unwrap_or_else(|| panic!("no profile {id}"))
}

fn profile_mut<'a>(cfg: &'a mut WheelConfig, id: &str) -> &'a mut Profile {
    cfg.profiles
        .iter_mut()
        .find(|p| p.id == id)
        .unwrap_or_else(|| panic!("no profile {id}"))
}

fn slot<'a>(slots: &'a [WheelSlot], id: &str) -> &'a WheelSlot {
    slots
        .iter()
        .find(|s| s.id == id)
        .unwrap_or_else(|| panic!("no slot {id} in {:?}", ids(slots)))
}

fn user_preset(id: &str, op: Operation, options: ConvertOptions) -> Preset {
    Preset {
        id: id.into(),
        name: "Mine".into(),
        builtin: false,
        op,
        options,
    }
}

#[test]
fn builtin_is_valid() {
    let cfg = WheelConfig::builtin();
    assert!(cfg.clone().validate().is_ok());
    for id in [
        "images",
        "vector",
        "video",
        "audio",
        "pdf",
        "text",
        "data",
        "subtitles",
        "archives",
        "mixed",
        "other",
    ] {
        assert!(profile(&cfg, id).builtin, "{id}");
    }
    for p in &cfg.profiles {
        assert!(p.front.len() <= 8, "{} front", p.id);
        assert!(p.back.len() <= 12, "{} back", p.id);
        for s in p.front.iter().chain(&p.back) {
            match s.strip_prefix("group:") {
                Some(g) => assert!(cfg.groups.iter().any(|x| x.id == g), "{s}"),
                None => assert!(cfg.preset(s).is_some(), "{} -> {s}", p.id),
            }
        }
    }
    assert_eq!(
        profile(&cfg, "images").front,
        [
            "to-png",
            "to-webp",
            "to-pdf",
            "to-jpg-70",
            "to-ico",
            "to-bmp",
            "to-tiff",
            "to-gif"
        ]
    );
    assert_eq!(profile(&cfg, "images").back.last().unwrap(), "group:files");
    let files = cfg.groups.iter().find(|g| g.id == "files").unwrap();
    assert_eq!(files.name, "");
    assert_eq!(files.presets, ["zip", "copy-to", "move-to"]);
    assert!(profile(&cfg, "mixed").mixed);
    assert!(profile(&cfg, "other").extensions.is_empty());
    assert!(cfg.presets.iter().all(|p| p.builtin && p.name.is_empty()));
}

#[test]
fn serde_matches_the_ts_mirror() {
    let cfg = WheelConfig::builtin();
    let v = serde_json::to_value(&cfg).unwrap();
    let preset = |id: &str| {
        v["presets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == id)
            .unwrap()
            .clone()
    };
    assert_eq!(
        preset("move-to")["op"],
        json!({"type": "copyTo", "dir": null, "moveFiles": true})
    );
    assert_eq!(
        preset("crop-16-9")["op"],
        json!({"type": "cropAspect", "w": 16, "h": 9})
    );
    assert_eq!(
        preset("rotate-left")["op"],
        json!({"type": "edit", "ops": [{"type": "rotateLeft"}], "to": null})
    );
    assert_eq!(preset("to-jpg-70")["options"]["quality"], 70);
    assert_eq!(preset("zip")["op"], json!({"type": "zip"}));
    assert_eq!(preset("annotate")["op"], json!({"type": "annotate"}));
    let p = &v["profiles"][0];
    for key in [
        "id",
        "name",
        "builtin",
        "extensions",
        "mixed",
        "front",
        "back",
    ] {
        assert!(p.get(key).is_some(), "profile.{key}");
    }
    assert!(v["version"].is_number() && v["groups"].is_array());

    let op: Operation = serde_json::from_value(json!({
        "type": "edit",
        "ops": [{"type": "pixelate", "block": 8}, {"type": "blur", "sigma": 2.5},
                {"type": "scale", "percent": 50}, {"type": "flipH"}, {"type": "rotate180"}],
        "to": "webp"
    }))
    .unwrap();
    assert_eq!(
        op,
        Operation::Edit {
            ops: vec![
                ImageOp::Pixelate { block: 8 },
                ImageOp::Blur { sigma: 2.5 },
                ImageOp::Scale { percent: 50 },
                ImageOp::FlipH,
                ImageOp::Rotate180,
            ],
            to: Some("webp".into()),
        }
    );

    let slots = cfg.slots(profile(&cfg, "images"), Face::Front, &paths(&["a.png"]));
    let sv = serde_json::to_value(&slots[0]).unwrap();
    for key in [
        "id",
        "name",
        "format",
        "detail",
        "icon",
        "kind",
        "needsTools",
        "children",
    ] {
        assert!(sv.get(key).is_some(), "slot.{key}");
    }
    assert_eq!(json!(Kind::Subtitle), json!("subtitle"));
    assert_eq!(json!(Face::Back), json!("back"));
}

#[test]
fn resolves_profiles() {
    let cfg = WheelConfig::builtin();
    let id = |names: &[&str]| cfg.profile_for(&paths(names)).id.clone();
    assert_eq!(id(&["a.JPG", "b.png"]), "images");
    assert_eq!(id(&["x.mp4"]), "video");
    assert_eq!(id(&["a.jpg", "b.mp4"]), "mixed");
    assert_eq!(id(&["a.docx"]), "other");
    assert_eq!(id(&["a.jpg", "b.docx"]), "mixed");
    assert_eq!(id(&["noext"]), "other");
    assert_eq!(id(&[]), "other");
    assert_eq!(id(&["a.srt", "b.ass"]), "subtitles");
    assert_eq!(id(&["a.zip"]), "archives");
    assert_eq!(id(&["n.md", "t.txt"]), "text");
    assert_eq!(id(&["d.tsv"]), "data");
    assert_eq!(id(&["r.m4r"]), "audio");
}

#[test]
fn resolves_mixed_and_unknown_selections() {
    let cfg = WheelConfig::builtin();
    let sel = paths(&["a.jpg", "b.docx", "c.mp4"]);
    let prof = cfg.profile_for(&sel);
    assert_eq!(prof.id, "mixed");
    let front = cfg.slots(prof, Face::Front, &sel);
    assert!(!front.is_empty());
    let v = ids(&front);
    assert!(v.contains(&"zip") && v.contains(&"copy-to") && v.contains(&"move-to"));
    assert!(!v.contains(&"to-png") && !v.contains(&"to-pdf") && !v.contains(&"merge-pdf"));
    assert!(cfg.slots(prof, Face::Back, &sel).is_empty());

    // A mixed profile removed by the user: fall back to "other", never an empty wheel.
    let mut no_mixed = WheelConfig::builtin();
    no_mixed.profiles.retain(|p| p.id != "mixed");
    let prof = no_mixed.profile_for(&sel);
    assert_eq!(prof.id, "other");
    assert!(ids(&no_mixed.slots(prof, Face::Front, &sel)).contains(&"zip"));

    // Even a config without any profile answers.
    let mut empty = WheelConfig::builtin();
    empty.profiles.clear();
    assert_eq!(empty.profile_for(&sel).id, "other");
}

#[test]
fn slots_hide_inapplicable_and_carry_details() {
    let cfg = WheelConfig::builtin();
    let images = profile(&cfg, "images");

    let png = cfg.slots(images, Face::Front, &paths(&["a.png"]));
    let jpg = slot(&png, "to-jpg-70");
    assert_eq!(jpg.format.as_deref(), Some("JPG"));
    assert_eq!(jpg.detail.as_deref(), Some("70%"));
    assert_eq!(jpg.kind, Some(Kind::Image));
    assert_eq!(jpg.icon, "image");
    assert!(jpg.needs_tools.is_empty() && jpg.children.is_empty());
    assert!(!ids(&png).contains(&"to-png"), "same format is hidden");
    assert_eq!(slot(&png, "to-pdf").kind, Some(Kind::Pdf));

    let gif = cfg.slots(images, Face::Front, &paths(&["a.gif"]));
    assert!(!ids(&gif).contains(&"to-gif"));
    assert!(ids(&gif).contains(&"to-png"));

    let back = cfg.slots(images, Face::Back, &paths(&["a.png"]));
    assert_eq!(slot(&back, "resize-50").detail.as_deref(), Some("50%"));
    assert_eq!(slot(&back, "resize-50").icon, "resize");
    assert_eq!(slot(&back, "fit-1920").detail.as_deref(), Some("1920 px"));
    assert_eq!(slot(&back, "pixelate").detail, None);
    assert_eq!(slot(&back, "blur").detail, None);
    assert_eq!(slot(&back, "rotate-left").icon, "rotateLeft");
    assert_eq!(slot(&back, "annotate").icon, "annotate");
    assert!(
        !ids(&back).contains(&"merge-pdf"),
        "merging needs two files"
    );
    let group = slot(&back, "group:files");
    assert_eq!(group.icon, "group");
    assert_eq!(group.kind, None);
    assert_eq!(ids(&group.children), ["zip", "copy-to", "move-to"]);
    assert_eq!(slot(&group.children, "move-to").icon, "move");

    let two = cfg.slots(images, Face::Back, &paths(&["a.png", "b.jpg"]));
    assert!(ids(&two).contains(&"merge-pdf"));
    assert!(
        !ids(&two).contains(&"annotate"),
        "the editor opens one image"
    );

    // A group whose members all are hidden disappears.
    let mut cfg2 = WheelConfig::builtin();
    cfg2.groups.push(fc_core::presets::Group {
        id: "pdfs".into(),
        name: "PDF".into(),
        presets: vec!["split-pdf".into()],
    });
    profile_mut(&mut cfg2, "images")
        .back
        .push("group:pdfs".into());
    let back2 = cfg2.slots(profile(&cfg2, "images"), Face::Back, &paths(&["a.png"]));
    assert!(!ids(&back2).contains(&"group:pdfs"));

    let pdf = cfg.slots(profile(&cfg, "pdf"), Face::Front, &paths(&["doc.pdf"]));
    assert_eq!(slot(&pdf, "to-png").needs_tools, ["pdfium"]);

    let video = cfg.slots(profile(&cfg, "video"), Face::Back, &paths(&["v.mp4"]));
    let crop = slot(&video, "crop-16-9");
    assert_eq!(crop.detail.as_deref(), Some("16:9"));
    assert_eq!(crop.kind, Some(Kind::Video));
    assert_eq!(crop.needs_tools, ["ffmpeg"]);
    assert_eq!(slot(&video, "speed-2x").detail.as_deref(), Some("2×"));
    assert_eq!(slot(&video, "speed-half").detail.as_deref(), Some("0.5×"));
    assert_eq!(slot(&video, "to-m4r").icon, "ringtone");
    assert_eq!(slot(&video, "zip").kind, None);

    let subs = cfg.slots(profile(&cfg, "subtitles"), Face::Front, &paths(&["a.srt"]));
    assert_eq!(ids(&subs), ["to-vtt", "to-txt"]);
    assert!(subs.iter().all(|s| s.needs_tools.is_empty()));
    let ass = cfg.slots(profile(&cfg, "subtitles"), Face::Front, &paths(&["a.ass"]));
    assert_eq!(slot(&ass, "to-srt").needs_tools, ["ffmpeg"]);

    let zip = cfg.slots(profile(&cfg, "archives"), Face::Front, &paths(&["a.zip"]));
    assert_eq!(ids(&zip), ["extract", "copy-to", "move-to"]);
}

#[test]
fn load_drops_dangling_refs_and_merges_builtins() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wheel.json");
    let file = json!({
        "version": 1,
        "presets": [
            {"id": "my-webp", "name": "Small WebP", "builtin": false,
             "op": {"type": "convert", "to": "webp"}, "options": {"quality": 50}},
            {"id": "future", "name": "From a newer version", "builtin": false,
             "op": {"type": "teleport"}, "options": {}},
            {"id": "gone-builtin", "name": "", "builtin": true,
             "op": {"type": "mute"}, "options": {}}
        ],
        "groups": [
            {"id": "g1", "name": "Mine", "presets": ["to-png", "ghost", "my-webp"]}
        ],
        "profiles": [
            {"id": "images", "name": "", "builtin": true, "extensions": ["png", "JPG", ".webp"],
             "mixed": false, "front": ["to-png", "ghost"], "back": ["group:g1", "group:nope"]},
            {"id": "docs", "name": "Word", "builtin": false, "extensions": ["docx"],
             "mixed": false, "front": ["zip", "copy-to", "zip"], "back": []},
            "garbage"
        ]
    });
    std::fs::write(&path, serde_json::to_vec(&file).unwrap()).unwrap();
    let cfg = WheelConfig::load(&path);

    let images = profile(&cfg, "images");
    assert_eq!(images.front, ["to-png"]);
    assert_eq!(images.back, ["group:g1"]);
    assert_eq!(images.extensions, ["png", "jpg", "webp"]);
    let g1 = cfg.groups.iter().find(|g| g.id == "g1").unwrap();
    assert_eq!(g1.presets, ["to-png", "my-webp"]);
    assert!(
        cfg.groups.iter().any(|g| g.id == "files"),
        "built-in group merged"
    );
    let docs = profile(&cfg, "docs");
    assert!(!docs.builtin);
    assert_eq!(docs.front, ["zip", "copy-to"]);
    assert_eq!(
        profile(&cfg, "video"),
        profile(&WheelConfig::builtin(), "video")
    );
    assert!(cfg.preset("my-webp").is_some());
    assert!(cfg.preset("future").is_none());
    assert!(cfg.preset("gone-builtin").is_none());
    assert!(cfg.preset("to-m4r").is_some(), "built-in presets merged");
    // The user's profile order is kept; missing built-ins come after.
    assert_eq!(cfg.profiles[0].id, "images");
    assert_eq!(cfg.profiles[1].id, "docs");
    assert_eq!(cfg.profile_for(&paths(&["report.docx"])).id, "docs");
    let cfg = cfg.validate().expect("a loaded config is valid");

    // Save + load round-trips.
    let saved = dir.path().join("sub").join("wheel.json");
    cfg.save(&saved).unwrap();
    assert_eq!(WheelConfig::load(&saved), cfg);

    // Missing or corrupt files give the built-ins.
    assert_eq!(
        WheelConfig::load(&dir.path().join("none.json")),
        WheelConfig::builtin()
    );
    std::fs::write(&path, b"{ not json").unwrap();
    assert_eq!(WheelConfig::load(&path), WheelConfig::builtin());
    std::fs::write(&path, b"[1, 2]").unwrap();
    assert_eq!(WheelConfig::load(&path), WheelConfig::builtin());
}

#[test]
fn load_enforces_limits() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wheel.json");
    let many: Vec<&str> = vec![
        "to-png",
        "to-jpg",
        "to-webp",
        "to-gif",
        "to-bmp",
        "to-tiff",
        "to-ico",
        "to-pdf",
        "grayscale",
        "blur",
        "trim",
        "pixelate",
        "flip-h",
        "flip-v",
    ];
    let file = json!({
        "version": 1,
        "presets": [],
        "groups": [{"id": "big", "name": "Big", "presets": many}],
        "profiles": [{"id": "images", "name": "", "builtin": true, "extensions": ["png"],
                      "mixed": false, "front": many, "back": many}]
    });
    std::fs::write(&path, serde_json::to_vec(&file).unwrap()).unwrap();
    let cfg = WheelConfig::load(&path);
    let images = profile(&cfg, "images");
    assert_eq!(images.front.len(), 8);
    assert_eq!(images.back.len(), 12);
    assert_eq!(
        cfg.groups
            .iter()
            .find(|g| g.id == "big")
            .unwrap()
            .presets
            .len(),
        6
    );
    assert!(cfg.validate().is_ok());
}

#[test]
fn validate_rejects_limits() {
    let invalid = |cfg: WheelConfig| matches!(cfg.validate(), Err(Error::Invalid(_)));

    let mut c = WheelConfig::builtin();
    profile_mut(&mut c, "images").front.push("to-jpg".into());
    assert_eq!(profile(&c, "images").front.len(), 9);
    assert!(invalid(c));

    let mut c = WheelConfig::builtin();
    let video_back = profile(&c, "video").back.clone();
    profile_mut(&mut c, "video").back = video_back
        .into_iter()
        .chain(["to-mp4".to_string()])
        .collect();
    assert!(invalid(c), "back of 13");

    let mut c = WheelConfig::builtin();
    let dup = c.presets[0].clone();
    c.presets.push(dup);
    assert!(invalid(c), "duplicate preset id");

    let mut c = WheelConfig::builtin();
    c.presets.push(user_preset(
        "q0",
        Operation::Convert { to: "jpg".into() },
        ConvertOptions {
            quality: Some(0),
            ..Default::default()
        },
    ));
    assert!(invalid(c), "quality 0");

    for (op, why) in [
        (
            Operation::Edit {
                ops: vec![ImageOp::Scale { percent: 0 }],
                to: None,
            },
            "scale 0",
        ),
        (
            Operation::Edit {
                ops: vec![ImageOp::Scale { percent: 401 }],
                to: None,
            },
            "scale 401",
        ),
        (
            Operation::Edit {
                ops: vec![ImageOp::Pixelate { block: 1 }],
                to: None,
            },
            "pixelate 1",
        ),
        (
            Operation::Edit {
                ops: vec![ImageOp::Blur { sigma: 0.1 }],
                to: None,
            },
            "blur 0.1",
        ),
        (
            Operation::Edit {
                ops: vec![],
                to: Some("mp4".into()),
            },
            "edit to video",
        ),
        (Operation::Convert { to: "nope".into() }, "unknown format"),
        (Operation::CropAspect { w: 0, h: 9 }, "crop 0"),
        (Operation::Speed { factor: 9.0 }, "speed 9"),
    ] {
        let mut c = WheelConfig::builtin();
        c.presets
            .push(user_preset("bad", op, ConvertOptions::default()));
        assert!(invalid(c), "{why}");
    }

    let mut c = WheelConfig::builtin();
    c.groups[0].presets = vec![
        "to-png", "to-jpg", "to-webp", "to-gif", "to-bmp", "to-tiff", "to-ico",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    assert!(invalid(c), "group of 7");

    let mut c = WheelConfig::builtin();
    profile_mut(&mut c, "images").front[0] = "ghost".into();
    assert!(invalid(c), "dangling slot");

    let mut c = WheelConfig::builtin();
    profile_mut(&mut c, "images").front[0] = "to-webp".into();
    assert!(invalid(c), "slot twice on one face");

    let mut c = WheelConfig::builtin();
    c.groups[0].presets.push("ghost".into());
    assert!(invalid(c), "dangling group member");

    let mut c = WheelConfig::builtin();
    let p = profile(&c, "video").clone();
    c.profiles.push(p);
    assert!(invalid(c), "duplicate profile id");

    let mut c = WheelConfig::builtin();
    c.presets.push(user_preset(
        "ok",
        Operation::Edit {
            ops: vec![
                ImageOp::Blur { sigma: 0.5 },
                ImageOp::Scale { percent: 400 },
            ],
            to: Some("png".into()),
        },
        ConvertOptions {
            quality: Some(100),
            ..Default::default()
        },
    ));
    assert!(c.validate().is_ok());
}

#[test]
fn reset_restores_builtin_profiles() {
    let mut c = WheelConfig::builtin();
    let p = profile_mut(&mut c, "images");
    p.front.clear();
    p.extensions = vec!["png".into()];
    p.mixed = true;
    c.groups.retain(|g| g.id != "files");
    assert!(c.reset_profile("images"));
    assert_eq!(
        profile(&c, "images"),
        profile(&WheelConfig::builtin(), "images")
    );
    assert!(
        c.groups.iter().any(|g| g.id == "files"),
        "groups the default arrangement uses come back"
    );
    assert!(c.clone().validate().is_ok());

    c.profiles.push(Profile {
        id: "docs".into(),
        name: "Docs".into(),
        builtin: false,
        extensions: vec!["docx".into()],
        mixed: false,
        front: vec!["zip".into()],
        back: vec![],
    });
    assert!(!c.reset_profile("docs"));
    assert!(!c.reset_profile("missing"));
}

#[test]
fn requests_build_jobs() {
    let cfg = WheelConfig::builtin();
    let reqs = cfg
        .requests("to-jpg-70", &paths(&["a.png", "b.png"]))
        .unwrap();
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].op, Operation::Convert { to: "jpg".into() });
    assert_eq!(reqs[0].options.quality, Some(70));
    assert_eq!(reqs[0].inputs, paths(&["a.png", "b.png"]));

    assert!(matches!(
        cfg.requests("copy-to", &paths(&["a.png"])),
        Err(Error::Invalid(_))
    ));
    assert!(matches!(
        cfg.requests("annotate", &paths(&["a.png"])),
        Err(Error::Invalid(_))
    ));
    assert!(matches!(
        cfg.requests("ghost", &paths(&["a.png"])),
        Err(Error::Invalid(_))
    ));

    let zip = cfg.requests("zip", &paths(&["a.png", "b.txt"])).unwrap();
    assert_eq!(zip.len(), 1);
    assert_eq!(zip[0].op, Operation::Zip);
    assert_eq!(zip[0].inputs.len(), 2);

    let fit = cfg.requests("fit-1920", &paths(&["a.png"])).unwrap();
    assert_eq!(fit[0].options.max_width, Some(1920));
    assert_eq!(fit[0].options.max_height, Some(1920));
    assert_eq!(
        cfg.requests("speed-half", &paths(&["v.mp4"])).unwrap()[0].op,
        Operation::Speed { factor: 0.5 }
    );

    // Files already in the target format are left alone.
    let png = cfg.requests("to-png", &paths(&["a.png", "b.jpg"])).unwrap();
    assert_eq!(png[0].inputs, paths(&["b.jpg"]));
    assert!(
        cfg.requests("to-png", &paths(&["a.png"]))
            .unwrap()
            .is_empty()
    );
    assert!(cfg.requests("zip", &[]).unwrap().is_empty());
}

#[test]
fn applicable_ids_for_extensions() {
    let cfg = WheelConfig::builtin();
    let mp4 = cfg.applicable_ids(&["mp4".to_string()]);
    for id in ["crop-16-9", "to-gif", "to-m4r", "mute", "zip", "copy-to"] {
        assert!(mp4.contains(&id.to_string()), "{id}");
    }
    for id in ["grayscale", "to-mp4", "extract", "annotate"] {
        assert!(!mp4.contains(&id.to_string()), "{id}");
    }
    let pdf = cfg.applicable_ids(&["pdf".to_string()]);
    assert!(pdf.contains(&"merge-pdf".to_string()));
    assert!(pdf.contains(&"split-pdf".to_string()));
    let png = cfg.applicable_ids(&["png".to_string()]);
    assert!(png.contains(&"annotate".to_string()) && png.contains(&"merge-pdf".to_string()));
    let any = cfg.applicable_ids(&["png".to_string(), "mp4".to_string()]);
    assert!(any.contains(&"grayscale".to_string()) && any.contains(&"mute".to_string()));
    assert!(
        cfg.applicable_ids(&["docx".to_string()])
            .contains(&"zip".to_string())
    );
    // No extensions (the "other" and mixed profiles) = an unknown file: the file operations.
    assert_eq!(cfg.applicable_ids(&[]), ["zip", "copy-to", "move-to"]);
    assert_eq!(
        cfg.applicable_ids(&[" ".to_string()]),
        ["zip", "copy-to", "move-to"]
    );

    let extract = cfg.preset("extract").unwrap();
    assert!(applicable(extract, &paths(&["a.ZIP"])));
    assert!(!applicable(extract, &paths(&["a.png"])));
    assert!(!applicable(cfg.preset("zip").unwrap(), &[]));
}

#[test]
fn every_known_extension_gets_a_front_face() {
    let cfg = WheelConfig::builtin();
    let mut exts: Vec<String> = fc_core::formats::FORMATS
        .iter()
        .flat_map(|f| f.extensions.iter().map(|e| e.to_string()))
        .collect();
    exts.push("docx".into());
    for ext in exts {
        for name in [format!("a.{ext}"), format!("A.{}", ext.to_uppercase())] {
            let sel = paths(&[&name]);
            let prof = cfg.profile_for(&sel);
            assert!(
                !cfg.slots(prof, Face::Front, &sel).is_empty(),
                "{name}: empty front in profile {}",
                prof.id
            );
        }
    }
    let html = cfg.slots(profile(&cfg, "text"), Face::Front, &paths(&["page.html"]));
    assert_eq!(ids(&html), ["zip", "copy-to", "move-to"]);
}

#[test]
fn folders_only_get_file_operations() {
    let cfg = WheelConfig::builtin();
    let d = tempfile::tempdir().unwrap();
    // Named like an image on purpose: a folder is never converted.
    let folder = d.path().join("pics.png");
    std::fs::create_dir_all(&folder).unwrap();
    let sel = vec![folder.clone()];
    let prof = cfg.profile_for(&sel);
    assert_eq!(prof.id, "other");
    assert_eq!(
        ids(&cfg.slots(prof, Face::Front, &sel)),
        ["zip", "copy-to", "move-to"]
    );
    for p in &cfg.presets {
        let file_op = matches!(p.op, Operation::Zip | Operation::CopyTo { .. });
        assert_eq!(applicable(p, &sel), file_op, "{}", p.id);
    }
    // A folder and an image: mixed, file operations only.
    let image = d.path().join("a.png");
    std::fs::write(&image, b"x").unwrap();
    let both = vec![folder, image];
    let prof = cfg.profile_for(&both);
    assert_eq!(prof.id, "mixed");
    assert_eq!(
        ids(&cfg.slots(prof, Face::Front, &both)),
        ["zip", "copy-to", "move-to"]
    );
}

#[test]
fn builtin_presets_are_read_only() {
    let read_only = |cfg: WheelConfig| match cfg.validate() {
        Err(Error::Invalid(msg)) => msg.contains("read-only"),
        _ => false,
    };
    let edit = |id: &str, f: &dyn Fn(&mut Preset)| {
        let mut cfg = WheelConfig::builtin();
        f(cfg.presets.iter_mut().find(|p| p.id == id).unwrap());
        cfg
    };
    assert!(read_only(
        edit("to-jpg-70", &|p| p.options.quality = Some(80))
    ));
    assert!(read_only(edit("grayscale", &|p| p.name = "Gray".into())));
    assert!(read_only(edit("zip", &|p| p.op = Operation::Extract)));
    // A user preset with a built-in id is no way around it.
    assert!(read_only(edit("to-png", &|p| {
        p.builtin = false;
        p.op = Operation::Convert { to: "webp".into() };
    })));
    // Unchanged built-ins (even with a wrong flag) are fine; the flag is normalized.
    let cfg = edit("to-png", &|p| p.builtin = false).validate().unwrap();
    assert!(cfg.preset("to-png").unwrap().builtin);
}

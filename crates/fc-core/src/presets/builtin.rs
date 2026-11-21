//! Built-in presets, groups and profiles (spec §3). Ids are stable: the UI localizes them
//! (`presets.builtin.<id>`, `presets.profile.<id>`, `presets.groupName.<id>`), so names are "".

use super::{Group, Preset, Profile};
use crate::formats;
use crate::jobs::{CompressLevel, ConvertOptions, ImageOp, Operation};

/// `to-<id>` convert presets, in presets-page order. `to-jpg-70` follows `to-jpg`.
const CONVERT: &[&str] = &[
    "png", "jpg", "webp", "gif", "bmp", "tiff", "ico", "pdf", // images & documents
    "mp4", "webm", "mkv", "mov", "avi", // video
    "mp3", "m4a", "wav", "flac", "ogg", "opus", "m4r", // audio
    "csv", "tsv", "json", "yaml", "toml", "html", "txt", // text & data
    "srt", "vtt", // subtitles
];

fn preset(id: &str, op: Operation) -> Preset {
    Preset {
        id: id.to_string(),
        name: String::new(),
        builtin: true,
        op,
        options: ConvertOptions::default(),
    }
}

fn convert(to: &str) -> Operation {
    Operation::Convert { to: to.to_string() }
}

fn edit(id: &str, op: ImageOp) -> Preset {
    preset(
        id,
        Operation::Edit {
            ops: vec![op],
            to: None,
        },
    )
}

pub(super) fn presets() -> Vec<Preset> {
    let mut v = Vec::with_capacity(64);
    for to in CONVERT {
        v.push(preset(&format!("to-{to}"), convert(to)));
        if *to == "jpg" {
            v.push(Preset {
                options: ConvertOptions {
                    quality: Some(70),
                    ..ConvertOptions::default()
                },
                ..preset("to-jpg-70", convert("jpg"))
            });
        }
    }
    v.extend([
        edit("grayscale", ImageOp::Grayscale),
        edit("rotate-left", ImageOp::RotateLeft),
        edit("rotate-right", ImageOp::RotateRight),
        edit("rotate-180", ImageOp::Rotate180),
        edit("flip-h", ImageOp::FlipH),
        edit("flip-v", ImageOp::FlipV),
        edit("trim", ImageOp::Trim),
        edit("pixelate", ImageOp::Pixelate { block: 12 }),
        edit("blur", ImageOp::Blur { sigma: 6.0 }),
        edit("resize-50", ImageOp::Scale { percent: 50 }),
        Preset {
            options: ConvertOptions {
                max_width: Some(1920),
                max_height: Some(1920),
                ..ConvertOptions::default()
            },
            ..preset(
                "fit-1920",
                Operation::Edit {
                    ops: Vec::new(),
                    to: None,
                },
            )
        },
        preset("annotate", Operation::Annotate),
        preset(
            "compress",
            Operation::Compress {
                level: CompressLevel::Balanced,
            },
        ),
        preset("mute", Operation::Mute),
        preset("speed-2x", Operation::Speed { factor: 2.0 }),
        preset("speed-half", Operation::Speed { factor: 0.5 }),
        preset("crop-16-9", Operation::CropAspect { w: 16, h: 9 }),
        preset("crop-9-16", Operation::CropAspect { w: 9, h: 16 }),
        preset("crop-1-1", Operation::CropAspect { w: 1, h: 1 }),
        preset("crop-4-3", Operation::CropAspect { w: 4, h: 3 }),
        preset("merge-pdf", Operation::MergePdf),
        preset("split-pdf", Operation::SplitPdf),
        preset("zip", Operation::Zip),
        preset("extract", Operation::Extract),
        preset(
            "copy-to",
            Operation::CopyTo {
                dir: None,
                move_files: false,
            },
        ),
        preset(
            "move-to",
            Operation::CopyTo {
                dir: None,
                move_files: true,
            },
        ),
        preset("copy-path", Operation::CopyPath),
        preset("open-terminal", Operation::OpenTerminal),
        preset("float", Operation::Float),
    ]);
    v
}

pub(super) fn groups() -> Vec<Group> {
    vec![Group {
        id: "files".into(),
        name: String::new(),
        presets: strings(&[
            "zip",
            "copy-to",
            "move-to",
            "copy-path",
            "open-terminal",
            "float",
        ]),
    }]
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_string()).collect()
}

/// Every extension of the given format ids, in `FORMATS` order.
fn extensions(ids: &[&str]) -> Vec<String> {
    formats::FORMATS
        .iter()
        .filter(|f| ids.contains(&f.id))
        .flat_map(|f| f.extensions.iter().map(|e| (*e).to_string()))
        .collect()
}

/// Extensions of every format of `kind`.
fn extensions_of(kind: formats::Kind) -> Vec<String> {
    let ids: Vec<&str> = formats::FORMATS
        .iter()
        .filter(|f| f.kind == kind)
        .map(|f| f.id)
        .collect();
    extensions(&ids)
}

fn profile(id: &str, extensions: Vec<String>, front: &[&str], back: &[&str]) -> Profile {
    Profile {
        id: id.to_string(),
        name: String::new(),
        builtin: true,
        extensions,
        mixed: false,
        front: strings(front),
        back: strings(back),
    }
}

/// The fallback for files no profile claims: ZIP / copy / move.
pub(super) fn other() -> Profile {
    profile(
        "other",
        Vec::new(),
        &[
            "zip",
            "copy-to",
            "move-to",
            "copy-path",
            "open-terminal",
            "float",
        ],
        &[],
    )
}

const FILES: [&str; 3] = ["zip", "copy-to", "move-to"];

pub(super) fn profiles() -> Vec<Profile> {
    use formats::Kind;
    vec![
        profile(
            "images",
            extensions_of(Kind::Image),
            &[
                "to-png",
                "to-webp",
                "to-pdf",
                "to-jpg-70",
                "to-ico",
                "to-bmp",
                "to-tiff",
                "to-gif",
            ],
            &[
                "rotate-left",
                "rotate-right",
                "flip-h",
                "grayscale",
                "trim",
                "pixelate",
                "blur",
                "resize-50",
                "fit-1920",
                "annotate",
                "merge-pdf",
                "group:files",
            ],
        ),
        profile(
            "vector",
            extensions_of(Kind::Vector),
            &["to-png", "to-jpg", "to-webp", "to-ico"],
            &FILES,
        ),
        profile(
            "video",
            extensions_of(Kind::Video),
            &[
                "to-mp4", "to-webm", "to-gif", "to-mp3", "compress", "mute", "to-mkv", "to-mov",
            ],
            &[
                "crop-16-9",
                "crop-9-16",
                "crop-1-1",
                "crop-4-3",
                "speed-2x",
                "speed-half",
                "to-m4a",
                "to-wav",
                "to-m4r",
                "zip",
                "copy-to",
                "move-to",
            ],
        ),
        profile(
            "audio",
            extensions_of(Kind::Audio),
            &[
                "to-mp3", "to-m4a", "to-wav", "to-flac", "to-ogg", "to-opus", "to-m4r", "compress",
            ],
            &["speed-2x", "speed-half", "zip", "copy-to", "move-to"],
        ),
        profile(
            "pdf",
            extensions_of(Kind::Pdf),
            &[
                "to-png",
                "to-jpg",
                "merge-pdf",
                "split-pdf",
                "compress",
                "zip",
            ],
            &["copy-to", "move-to"],
        ),
        // md and txt only convert to HTML (no route to plain text); HTML itself has no
        // conversion, so the file actions sit on the front too.
        profile(
            "text",
            extensions(&["md", "txt", "html"]),
            &["to-html", "zip", "copy-to", "move-to"],
            &[],
        ),
        profile(
            "data",
            extensions(&["csv", "tsv", "json", "yaml", "toml"]),
            &["to-csv", "to-tsv", "to-json", "to-yaml", "to-toml"],
            &FILES,
        ),
        profile(
            "subtitles",
            extensions_of(Kind::Subtitle),
            &["to-srt", "to-vtt", "to-txt"],
            &FILES,
        ),
        profile(
            "archives",
            extensions_of(Kind::Archive),
            &["extract", "copy-to", "move-to"],
            &[],
        ),
        Profile {
            mixed: true,
            ..profile(
                "mixed",
                Vec::new(),
                &["to-pdf", "merge-pdf", "zip", "copy-to", "move-to"],
                &[],
            )
        },
        other(),
    ]
}

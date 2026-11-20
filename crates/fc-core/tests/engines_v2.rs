//! v2 operations and formats: image edits, aspect crop, zip/extract, copy/move, subtitles,
//! TSV, m4r and base64. In-process unless noted; ffmpeg tests skip when ffmpeg is not on PATH
//! (like `tests/ffmpeg.rs`).

use fc_core::engine_mgr::which;
use fc_core::engines::{DefaultExecutor, Executor, ToolLocator, data, subtitle};
use fc_core::jobs::{ConvertOptions, ImageOp, Operation};
use fc_core::{CancelToken, Error, Result, b64, proc};
use image::{GenericImageView, Rgba, RgbaImage};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

struct NoTools;
impl ToolLocator for NoTools {
    fn locate(&self, _tool: &str, _file: &str) -> Option<PathBuf> {
        None
    }
}

/// Tools from PATH (ffmpeg.exe, ffprobe.exe).
struct PathTools;
impl ToolLocator for PathTools {
    fn locate(&self, _tool: &str, file: &str) -> Option<PathBuf> {
        which(file)
    }
}

fn run_with(
    tools: Arc<dyn ToolLocator>,
    inputs: &[PathBuf],
    op: Operation,
) -> Result<Vec<PathBuf>> {
    DefaultExecutor { tools }.run(
        inputs,
        &op,
        &ConvertOptions::default(),
        &CancelToken::new(),
        &|_| {},
    )
}

fn run(inputs: &[PathBuf], op: Operation) -> Result<Vec<PathBuf>> {
    run_with(Arc::new(NoTools), inputs, op)
}

fn run_one(input: &Path, op: Operation) -> Result<Vec<PathBuf>> {
    run(&[input.to_path_buf()], op)
}

fn edit(ops: Vec<ImageOp>) -> Operation {
    Operation::Edit { ops, to: None }
}

fn save(dir: &Path, name: &str, img: &RgbaImage) -> PathBuf {
    let p = dir.join(name);
    img.save(&p).unwrap();
    p
}

/// 4x2: every pixel distinct and coloured.
fn sample_4x2() -> RgbaImage {
    RgbaImage::from_fn(4, 2, |x, y| {
        Rgba([(x * 60) as u8, (y * 120 + 10) as u8, 200, 255])
    })
}

fn only(outputs: Vec<PathBuf>) -> PathBuf {
    assert_eq!(outputs.len(), 1, "{outputs:?}");
    outputs.into_iter().next().unwrap()
}

#[test]
fn edit_ops_change_pixels() {
    let d = tempfile::tempdir().unwrap();
    let src = save(d.path(), "a.png", &sample_4x2());
    let original = image::open(&src).unwrap().to_rgba8();

    let rotated = only(run_one(&src, edit(vec![ImageOp::RotateLeft])).unwrap());
    assert_eq!(rotated.file_name().unwrap(), "a (edited).png");
    let r = image::open(&rotated).unwrap();
    assert_eq!(r.dimensions(), (2, 4));
    // Rotating left moves the top-right pixel to the top-left.
    assert_eq!(r.to_rgba8().get_pixel(0, 0), original.get_pixel(3, 0));

    let flipped = image::open(only(run_one(&src, edit(vec![ImageOp::FlipH])).unwrap()))
        .unwrap()
        .to_rgba8();
    assert_eq!(flipped.get_pixel(0, 0), original.get_pixel(3, 0));
    assert_eq!(flipped.get_pixel(3, 0), original.get_pixel(0, 0));

    let gray = image::open(only(run_one(&src, edit(vec![ImageOp::Grayscale])).unwrap()))
        .unwrap()
        .to_rgba8();
    assert!(gray.pixels().all(|p| p[0] == p[1] && p[1] == p[2]));

    let half = only(run_one(&src, edit(vec![ImageOp::Scale { percent: 50 }])).unwrap());
    assert_eq!(image::open(&half).unwrap().dimensions(), (2, 1));

    // Several steps in order, written as another format.
    let combo = only(
        run(
            std::slice::from_ref(&src),
            Operation::Edit {
                ops: vec![
                    ImageOp::Rotate180,
                    ImageOp::FlipV,
                    ImageOp::Scale { percent: 200 },
                ],
                to: Some("webp".into()),
            },
        )
        .unwrap(),
    );
    assert_eq!(combo.extension().unwrap(), "webp");
    assert_eq!(image::open(&combo).unwrap().dimensions(), (8, 4));

    let blurred = only(run_one(&src, edit(vec![ImageOp::Blur { sigma: 2.0 }])).unwrap());
    assert_eq!(image::open(&blurred).unwrap().dimensions(), (4, 2));
    let pix = only(run_one(&src, edit(vec![ImageOp::Pixelate { block: 2 }])).unwrap());
    let p = image::open(&pix).unwrap().to_rgba8();
    assert_eq!(p.dimensions(), (4, 2));
    assert_eq!(
        p.get_pixel(0, 0),
        p.get_pixel(1, 1),
        "one colour per 2x2 block"
    );

    // Max size from the options applies after the edits (fit-1920 style).
    let big = save(d.path(), "big.png", &RgbaImage::new(300, 100));
    let fitted = DefaultExecutor {
        tools: Arc::new(NoTools),
    }
    .run(
        &[big],
        &edit(vec![]),
        &ConvertOptions {
            max_width: Some(150),
            max_height: Some(150),
            ..ConvertOptions::default()
        },
        &CancelToken::new(),
        &|_| {},
    )
    .unwrap();
    assert_eq!(image::open(only(fitted)).unwrap().dimensions(), (150, 50));

    assert!(src.is_file(), "source untouched");
    assert!(matches!(
        run(&[src], edit(vec![ImageOp::Pixelate { block: 0 }])),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn trim_removes_transparent_border() {
    let d = tempfile::tempdir().unwrap();
    let mut img = RgbaImage::new(10, 10);
    for y in 5..8 {
        for x in 4..6 {
            img.put_pixel(x, y, Rgba([255, 0, 0, 255]));
        }
    }
    let src = save(d.path(), "t.png", &img);
    let out = only(run(&[src], edit(vec![ImageOp::Trim])).unwrap());
    let trimmed = image::open(&out).unwrap();
    assert_eq!(trimmed.dimensions(), (2, 3));
    assert_eq!(trimmed.to_rgba8().get_pixel(0, 0), &Rgba([255, 0, 0, 255]));

    let opaque = save(d.path(), "o.png", &sample_4x2());
    let out = only(run(&[opaque], edit(vec![ImageOp::Trim])).unwrap());
    assert_eq!(image::open(&out).unwrap().dimensions(), (4, 2));

    let empty = save(d.path(), "e.png", &RgbaImage::new(5, 5));
    let out = only(run(&[empty], edit(vec![ImageOp::Trim])).unwrap());
    assert_eq!(
        image::open(&out).unwrap().dimensions(),
        (5, 5),
        "nothing to keep: unchanged"
    );
}

#[test]
fn crop_aspect_image() {
    let d = tempfile::tempdir().unwrap();
    let src = save(d.path(), "wide.png", &RgbaImage::new(400, 200));
    let out = only(run_one(&src, Operation::CropAspect { w: 1, h: 1 }).unwrap());
    assert_eq!(out.file_name().unwrap(), "wide (cropped).png");
    assert_eq!(image::open(&out).unwrap().dimensions(), (200, 200));
    let out = only(run(&[src], Operation::CropAspect { w: 16, h: 9 }).unwrap());
    assert_eq!(image::open(&out).unwrap().dimensions(), (356, 200));
}

fn zip_names(path: &Path) -> Vec<String> {
    let mut z = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
    (0..z.len())
        .map(|i| z.by_index(i).unwrap().name().unwrap().to_string())
        .collect()
}

#[test]
fn zip_and_extract_roundtrip() {
    let d = tempfile::tempdir().unwrap();
    let (x, y) = (d.path().join("x"), d.path().join("y"));
    fs::create_dir_all(&x).unwrap();
    fs::create_dir_all(&y).unwrap();
    let a1 = x.join("a.txt");
    let a2 = y.join("a.txt");
    fs::write(&a1, b"first file").unwrap();
    fs::write(&a2, vec![7u8; 100_000]).unwrap();

    let archive = only(run(&[a1.clone(), a2.clone()], Operation::Zip).unwrap());
    assert_eq!(archive, x.join("a.zip"), "next to the first input");
    assert_eq!(zip_names(&archive), ["a.txt", "a (1).txt"]);
    let mut z = zip::ZipArchive::new(fs::File::open(&archive).unwrap()).unwrap();
    assert_eq!(
        z.by_index(1).unwrap().compression(),
        zip::CompressionMethod::Deflated
    );

    let folder = only(run_one(&archive, Operation::Extract).unwrap());
    assert_eq!(folder, x.join("a"));
    assert_eq!(fs::read(folder.join("a.txt")).unwrap(), b"first file");
    assert_eq!(
        fs::read(folder.join("a (1).txt")).unwrap(),
        vec![7u8; 100_000]
    );

    // A second extract never overwrites: a new unique folder.
    let again = only(run(&[archive], Operation::Extract).unwrap());
    assert_eq!(again, x.join("a (1)"));
}

fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let mut w = zip::ZipWriter::new(fs::File::create(path).unwrap());
    for (name, bytes) in entries {
        w.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        w.write_all(bytes).unwrap();
    }
    w.finish().unwrap();
}

#[test]
fn extract_keeps_folders_inside_the_archive() {
    let d = tempfile::tempdir().unwrap();
    let archive = d.path().join("pack.zip");
    write_zip(
        &archive,
        &[("docs/readme.md", b"# hi"), ("docs/sub/b.bin", b"\x00\x01")],
    );
    let folder = only(run(&[archive], Operation::Extract).unwrap());
    assert_eq!(
        fs::read(folder.join("docs").join("readme.md")).unwrap(),
        b"# hi"
    );
    assert_eq!(
        fs::read(folder.join("docs").join("sub").join("b.bin")).unwrap(),
        b"\x00\x01"
    );
}

#[test]
fn extract_rejects_zip_slip() {
    for evil in [
        "../evil.txt",
        "ok/../../evil.txt",
        "/evil.txt",
        "C:/evil.txt",
    ] {
        let d = tempfile::tempdir().unwrap();
        let inner = d.path().join("inner");
        fs::create_dir_all(&inner).unwrap();
        let archive = inner.join("bad.zip");
        write_zip(&archive, &[("fine.txt", b"ok"), (evil, b"pwned")]);
        let err = run(&[archive], Operation::Extract).unwrap_err();
        assert!(matches!(err, Error::Invalid(_)), "{evil}: {err}");
        assert!(!d.path().join("evil.txt").exists(), "{evil}");
        assert!(
            !inner.join("bad").exists(),
            "{evil}: partial folder removed"
        );
        assert_eq!(fs::read_dir(&inner).unwrap().count(), 1, "{evil}");
    }
}

#[test]
fn copy_and_move() {
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("name.ext");
    fs::write(&src, b"payload").unwrap();
    let dest = d.path().join("dest");
    fs::create_dir_all(&dest).unwrap();
    fs::write(dest.join("name.ext"), b"already here").unwrap();

    let copy = |dir: &Path, move_files: bool| {
        run(
            std::slice::from_ref(&src),
            Operation::CopyTo {
                dir: Some(dir.to_path_buf()),
                move_files,
            },
        )
    };
    let copied = only(copy(&dest, false).unwrap());
    assert_eq!(copied, dest.join("name (1).ext"));
    assert_eq!(fs::read(&copied).unwrap(), b"payload");
    assert_eq!(fs::read(dest.join("name.ext")).unwrap(), b"already here");
    assert!(src.is_file());

    // Moving within the same folder is skipped without an error.
    let same = only(copy(d.path(), true).unwrap());
    assert_eq!(same, src);
    assert!(src.is_file());

    // A missing target folder is created; moving removes the source.
    let fresh = d.path().join("new").join("deeper");
    let moved = only(copy(&fresh, true).unwrap());
    assert_eq!(moved, fresh.join("name.ext"));
    assert_eq!(fs::read(&moved).unwrap(), b"payload");
    assert!(!src.exists());

    assert!(matches!(
        run(
            &[moved],
            Operation::CopyTo {
                dir: None,
                move_files: false
            }
        ),
        Err(Error::Invalid(_))
    ));
}

/// `photos/a.txt`, `photos/sub/b.txt`, empty `photos/empty/`.
fn photo_folder(parent: &Path) -> PathBuf {
    let photos = parent.join("photos");
    fs::create_dir_all(photos.join("sub")).unwrap();
    fs::create_dir_all(photos.join("empty")).unwrap();
    fs::write(photos.join("a.txt"), b"top").unwrap();
    fs::write(photos.join("sub").join("b.txt"), b"nested").unwrap();
    photos
}

fn assert_photo_tree(dir: &Path) {
    assert_eq!(fs::read(dir.join("a.txt")).unwrap(), b"top");
    assert_eq!(fs::read(dir.join("sub").join("b.txt")).unwrap(), b"nested");
    assert!(dir.join("empty").is_dir());
}

#[test]
fn zip_copy_and_move_folders() {
    let d = tempfile::tempdir().unwrap();
    let photos = photo_folder(d.path());
    let note = d.path().join("note.txt");
    fs::write(&note, b"n").unwrap();

    // ZIP: the folder name is the top entry, nested paths and empty folders are kept.
    let archive = only(run(&[photos.clone(), note.clone()], Operation::Zip).unwrap());
    assert_eq!(archive, d.path().join("photos.zip"));
    assert_eq!(
        zip_names(&archive),
        [
            "photos/",
            "photos/a.txt",
            "photos/empty/",
            "photos/sub/",
            "photos/sub/b.txt",
            "note.txt"
        ]
    );
    let album = d.path().join("my.album");
    fs::create_dir_all(&album).unwrap();
    let album_zip = only(run_one(&album, Operation::Zip).unwrap());
    assert_eq!(album_zip, d.path().join("my.album.zip"));
    assert_eq!(zip_names(&album_zip), ["my.album/"]);
    let extracted = only(run_one(&archive, Operation::Extract).unwrap());
    assert_eq!(
        extracted,
        d.path().join("photos (1)"),
        "never over the source"
    );
    assert_photo_tree(&extracted.join("photos"));

    // Copy: recursive, a unique name when the folder exists there already.
    let dest = d.path().join("dest");
    fs::create_dir_all(dest.join("photos")).unwrap();
    let to = |dir: &Path, move_files: bool| Operation::CopyTo {
        dir: Some(dir.to_path_buf()),
        move_files,
    };
    let copied = only(run_one(&photos, to(&dest, false)).unwrap());
    assert_eq!(copied, dest.join("photos (1)"));
    assert_photo_tree(&copied);
    assert_photo_tree(&photos);
    assert!(matches!(
        run_one(&photos, to(&photos.join("sub"), false)),
        Err(Error::Invalid(_))
    ));
    assert!(matches!(
        run_one(&photos, to(&photos, true)),
        Err(Error::Invalid(_))
    ));

    // Move: same parent is a no-op; elsewhere the folder moves with everything in it.
    assert_eq!(only(run_one(&photos, to(d.path(), true)).unwrap()), photos);
    let moved = only(run_one(&photos, to(&dest, true)).unwrap());
    assert_eq!(moved, dest.join("photos (2)"));
    assert_photo_tree(&moved);
    assert!(!photos.exists());

    // Other operations still want files.
    assert!(matches!(
        run_one(&moved, Operation::Convert { to: "png".into() }),
        Err(Error::NotFound(_))
    ));
}

const SRT: &str = "1\n00:00:01,000 --> 00:00:02,500\n{\\an8}<i>Hi</i>\n";

fn convert_text(dir: &Path, name: &str, content: &str, from: &str, to: &str) -> String {
    let input = dir.join(name);
    fs::write(&input, content).unwrap();
    let out = dir.join(format!("{name}.{to}"));
    subtitle::convert(&input, from, to, &out).unwrap();
    fs::read_to_string(out).unwrap()
}

#[test]
fn subtitles_convert() {
    let d = tempfile::tempdir().unwrap();
    let vtt = convert_text(d.path(), "a.srt", SRT, "srt", "vtt");
    assert!(vtt.starts_with("WEBVTT"), "{vtt}");
    assert!(vtt.contains("00:00:01.000 --> 00:00:02.500"), "{vtt}");
    assert!(
        vtt.contains("<i>Hi</i>") && !vtt.contains("{\\an8}"),
        "{vtt}"
    );

    assert_eq!(convert_text(d.path(), "b.srt", SRT, "srt", "txt"), "Hi\n");

    let srt = convert_text(d.path(), "c.vtt", &vtt, "vtt", "srt");
    assert!(
        srt.starts_with("1\n00:00:01,000 --> 00:00:02,500\n"),
        "{srt}"
    );

    // WebVTT extras: header text, NOTE blocks, cue ids, cue settings, short timestamps, CRLF.
    let rich = "\u{feff}WEBVTT - demo\r\n\r\nNOTE a comment\r\n\r\nintro\r\n00:01.000 --> 00:03.250 align:start position:10%\r\n<v Ann>Hello &amp; welcome</v>\r\nsecond line\r\n\r\n01:00:00.000 --> 01:00:01.000\r\nBye\r\n";
    let srt = convert_text(d.path(), "d.vtt", rich, "vtt", "srt");
    assert_eq!(
        srt,
        "1\n00:00:01,000 --> 00:00:03,250\nHello & welcome\nsecond line\n\n2\n01:00:00,000 --> 01:00:01,000\nBye\n"
    );
    assert_eq!(
        convert_text(d.path(), "e.vtt", rich, "vtt", "txt"),
        "Hello & welcome\nsecond line\n\nBye\n"
    );

    let none = d.path().join("none.srt");
    fs::write(&none, "just text").unwrap();
    assert!(subtitle::convert(&none, "srt", "vtt", &d.path().join("none.vtt")).is_err());

    // Through the executor (route srt -> vtt).
    let input = d.path().join("movie.srt");
    fs::write(&input, SRT).unwrap();
    let out = only(run(&[input], Operation::Convert { to: "vtt".into() }).unwrap());
    assert_eq!(out.file_name().unwrap(), "movie.vtt");
}

/// "Gün ışığı: Şİ Ğğ “tırnak”" in Windows-1254 ("ANSI" on Turkish Windows).
const CP1254_LINE: &[u8] = b"G\xFCn \xFD\xFE\xFD\xF0\xFD: \xDE\xDD \xD0\xF0 \x93t\xFDrnak\x94";
const TURKISH_LINE: &str = "Gün ışığı: Şİ Ğğ “tırnak”";

fn convert_bytes(dir: &Path, name: &str, bytes: &[u8], from: &str, to: &str) -> String {
    let input = dir.join(name);
    fs::write(&input, bytes).unwrap();
    let out = dir.join(format!("{name}.{to}"));
    subtitle::convert(&input, from, to, &out).unwrap();
    fs::read_to_string(out).expect("output is UTF-8")
}

#[test]
fn subtitles_in_legacy_and_utf16_encodings() {
    let d = tempfile::tempdir().unwrap();
    let mut ansi = b"1\r\n00:00:01,000 --> 00:00:02,000\r\n".to_vec();
    ansi.extend_from_slice(CP1254_LINE);
    ansi.extend_from_slice(b"\r\n");
    assert!(std::str::from_utf8(&ansi).is_err(), "really not UTF-8");
    assert_eq!(
        convert_bytes(d.path(), "ansi.srt", &ansi, "srt", "txt"),
        format!("{TURKISH_LINE}\n")
    );
    let vtt = convert_bytes(d.path(), "ansi2.srt", &ansi, "srt", "vtt");
    assert!(vtt.contains(TURKISH_LINE), "{vtt}");

    // Notepad's "Unicode": UTF-16 LE with a BOM.
    let text = format!("1\r\n00:00:03,000 --> 00:00:04,500\r\n{TURKISH_LINE}\r\n");
    let mut utf16 = vec![0xFF, 0xFE];
    for unit in text.encode_utf16() {
        utf16.extend_from_slice(&unit.to_le_bytes());
    }
    let vtt = convert_bytes(d.path(), "wide.srt", &utf16, "srt", "vtt");
    assert!(vtt.contains("00:00:03.000 --> 00:00:04.500"), "{vtt}");
    assert!(vtt.contains(TURKISH_LINE), "{vtt}");

    // UTF-16 BE with a BOM, and UTF-8 with a BOM.
    let mut be = vec![0xFE, 0xFF];
    for unit in text.encode_utf16() {
        be.extend_from_slice(&unit.to_be_bytes());
    }
    assert!(convert_bytes(d.path(), "be.srt", &be, "srt", "txt").contains(TURKISH_LINE));
    let mut bom = vec![0xEF, 0xBB, 0xBF];
    bom.extend_from_slice(text.as_bytes());
    assert_eq!(
        convert_bytes(d.path(), "bom.srt", &bom, "srt", "txt"),
        format!("{TURKISH_LINE}\n")
    );
}

#[test]
fn tsv_roundtrip() {
    let d = tempfile::tempdir().unwrap();
    let tsv = "name\tnote\tage\nAda\tsays hi, twice\t36\nLinus\t\t\n";
    let input = d.path().join("t.tsv");
    fs::write(&input, tsv).unwrap();
    let csv = d.path().join("t.csv");
    data::convert(&input, "tsv", "csv", &csv).unwrap();
    let csv_text = fs::read_to_string(&csv).unwrap();
    assert!(csv_text.starts_with("name,note,age\n"), "{csv_text}");
    let back = d.path().join("back.tsv");
    data::convert(&csv, "csv", "tsv", &back).unwrap();
    assert_eq!(fs::read_to_string(&back).unwrap(), tsv);

    let json = d.path().join("t.json");
    data::convert(&input, "tsv", "json", &json).unwrap();
    let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(&json).unwrap()).unwrap();
    assert_eq!(v[0]["note"], "says hi, twice");
    let tsv2 = d.path().join("j.tsv");
    data::convert(&json, "json", "tsv", &tsv2).unwrap();
    assert!(fs::read_to_string(&tsv2).unwrap().contains("Ada\t"));
}

#[test]
fn b64_roundtrip() {
    let bytes = b"\x00\xffhi";
    let url = format!("data:image/png;base64,{}", b64::encode(bytes));
    assert_eq!(b64::decode(&url).unwrap(), bytes);
    assert_eq!(b64::decode(&b64::encode(bytes)).unwrap(), bytes);
    assert!(matches!(
        b64::decode("data:x;base64,@@"),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn unknown_and_unsupported_inputs_fail_cleanly() {
    let d = tempfile::tempdir().unwrap();
    let txt = d.path().join("n.txt");
    fs::write(&txt, b"x").unwrap();
    assert!(run_one(&txt, edit(vec![ImageOp::Grayscale])).is_err());
    assert!(run_one(&txt, Operation::Extract).is_err());
    assert!(run_one(&txt, Operation::CropAspect { w: 1, h: 1 }).is_err());
    assert!(matches!(
        run_one(&txt, Operation::Annotate),
        Err(Error::Invalid(_))
    ));
    let garbage = d.path().join("g.zip");
    fs::write(&garbage, b"not a zip").unwrap();
    assert!(run(&[garbage], Operation::Extract).is_err());
    assert_eq!(fs::read_dir(d.path()).unwrap().count(), 2, "no leftovers");
}

// ---------------------------------------------------------------- ffmpeg (skipped without it)

fn ffmpeg_on_path() -> Option<PathBuf> {
    let found = which("ffmpeg.exe").filter(|_| which("ffprobe.exe").is_some());
    if found.is_none() {
        eprintln!("skipping: ffmpeg/ffprobe not on PATH");
    }
    found
}

fn ffmpeg(ffmpeg: &Path, args: &[&str]) {
    let mut cmd = proc::command(ffmpeg);
    cmd.args(["-hide_banner", "-loglevel", "error", "-y"])
        .args(args);
    let out = proc::run(cmd, &CancelToken::new()).unwrap();
    assert!(out.status.success(), "{}", out.stderr);
}

fn probe(path: &Path, entries: &str) -> String {
    let mut cmd = proc::command(which("ffprobe.exe").unwrap());
    cmd.args(["-v", "error", "-show_entries", entries, "-of", "csv=p=0"])
        .arg(path);
    proc::run(cmd, &CancelToken::new())
        .unwrap()
        .stdout
        .trim()
        .to_string()
}

#[test]
fn m4r_limits_duration() {
    let Some(ff) = ffmpeg_on_path() else { return };
    let d = tempfile::tempdir().unwrap();
    let wav = d.path().join("tone.wav");
    let wav_s = wav.to_string_lossy().into_owned();
    ffmpeg(
        &ff,
        &[
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=60",
            &wav_s,
        ],
    );
    let out = only(
        run_with(
            Arc::new(PathTools),
            &[wav],
            Operation::Convert { to: "m4r".into() },
        )
        .unwrap(),
    );
    assert_eq!(out.extension().unwrap(), "m4r");
    let secs: f64 = probe(&out, "format=duration").parse().unwrap();
    assert!(secs <= 40.5 && secs > 39.0, "{secs}");
    assert_eq!(probe(&out, "stream=codec_name"), "aac");
}

#[test]
fn ass_converts_with_ffmpeg() {
    let Some(_) = ffmpeg_on_path() else { return };
    let d = tempfile::tempdir().unwrap();
    let ass = d.path().join("s.ass");
    fs::write(
        &ass,
        "[Script Info]\nScriptType: v4.00+\n\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\nStyle: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.00,0:00:02.50,Default,,0,0,0,,Merhaba\n",
    )
    .unwrap();
    let tools: Arc<dyn ToolLocator> = Arc::new(PathTools);
    let srt = only(
        run_with(
            tools.clone(),
            std::slice::from_ref(&ass),
            Operation::Convert { to: "srt".into() },
        )
        .unwrap(),
    );
    let text = fs::read_to_string(&srt).unwrap();
    assert!(
        text.contains("00:00:01,000 --> 00:00:02,500") && text.contains("Merhaba"),
        "{text}"
    );
    let vtt = only(
        run_with(
            tools.clone(),
            std::slice::from_ref(&ass),
            Operation::Convert { to: "vtt".into() },
        )
        .unwrap(),
    );
    let mut s = String::new();
    fs::File::open(&vtt)
        .unwrap()
        .read_to_string(&mut s)
        .unwrap();
    assert!(s.starts_with("WEBVTT") && s.contains("Merhaba"), "{s}");

    // The same script saved as Windows-1254 ("ANSI"): read with -sub_charenc CP1254.
    let utf8 = fs::read_to_string(&ass).unwrap();
    let mut ansi = utf8.replace("Merhaba\n", "").into_bytes();
    ansi.extend_from_slice(CP1254_LINE);
    ansi.push(b'\n');
    let ass_ansi = d.path().join("ansi.ass");
    fs::write(&ass_ansi, &ansi).unwrap();
    let srt = only(run_with(tools, &[ass_ansi], Operation::Convert { to: "srt".into() }).unwrap());
    let text = fs::read_to_string(&srt).expect("UTF-8 output");
    assert!(text.contains(TURKISH_LINE), "{text}");
}

#[test]
fn crop_aspect_video() {
    let Some(ff) = ffmpeg_on_path() else { return };
    let d = tempfile::tempdir().unwrap();
    let mp4 = d.path().join("in.mp4");
    let mp4_s = mp4.to_string_lossy().into_owned();
    ffmpeg(
        &ff,
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=1:size=320x240:rate=10",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            &mp4_s,
        ],
    );
    let out = only(
        run_with(
            Arc::new(PathTools),
            &[mp4],
            Operation::CropAspect { w: 16, h: 9 },
        )
        .unwrap(),
    );
    assert_eq!(out.file_name().unwrap(), "in (cropped).mp4");
    assert_eq!(probe(&out, "stream=width,height"), "320,180");
}

use fc_core::engine_mgr::manifest::{Archive, FileMap};
use fc_core::engine_mgr::{
    EngineManager, Manifest, ToolSource, extract_archive, sha256_file, which_in,
};
use fc_core::engines::ToolLocator;
use fc_core::{CancelToken, Error};
use std::io::{Read, Write};
use std::net::TcpListener;

fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut zw = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, data) in entries {
            zw.start_file(*name, opts).unwrap();
            zw.write_all(data).unwrap();
        }
        zw.finish().unwrap();
    }
    buf.into_inner()
}

fn targz_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    let mut tb = tar::Builder::new(gz);
    for (name, data) in entries {
        let mut h = tar::Header::new_gnu();
        h.set_size(data.len() as u64);
        h.set_mode(0o644);
        h.set_cksum();
        tb.append_data(&mut h, name, *data).unwrap();
    }
    tb.into_inner().unwrap().finish().unwrap()
}

fn map(from: &str, to: &str) -> FileMap {
    FileMap {
        from: from.into(),
        to: to.into(),
    }
}

#[test]
fn which_in_finds_files_on_a_given_path() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    std::fs::write(b.path().join("tool.exe"), b"x").unwrap();
    let path = std::env::join_paths([a.path(), b.path()]).unwrap();
    assert_eq!(which_in("tool.exe", &path), Some(b.path().join("tool.exe")));
    assert_eq!(which_in("other.exe", &path), None);
}

#[test]
fn sha256_of_known_content() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("f");
    std::fs::write(&p, b"abc").unwrap();
    assert_eq!(
        sha256_file(&p).unwrap(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn zip_extraction_matches_by_suffix_and_only_mapped_files() {
    let d = tempfile::tempdir().unwrap();
    let archive = d.path().join("a.zip");
    std::fs::write(
        &archive,
        zip_bytes(&[
            ("ffmpeg-9.0.2-essentials_build/bin/ffmpeg.exe", b"FF"),
            ("ffmpeg-9.0.2-essentials_build/bin/ffprobe.exe", b"FP"),
            ("ffmpeg-9.0.2-essentials_build/doc/readme.txt", b"no"),
            ("ffmpeg-9.0.2-essentials_build/xbin/ffmpeg.exe", b"WRONG"),
        ]),
    )
    .unwrap();
    let out = d.path().join("out");
    std::fs::create_dir(&out).unwrap();
    extract_archive(
        Archive::Zip,
        &archive,
        &[
            map("bin/ffmpeg.exe", "ffmpeg.exe"),
            map("bin/ffprobe.exe", "ffprobe.exe"),
        ],
        &out,
    )
    .unwrap();
    assert_eq!(std::fs::read(out.join("ffmpeg.exe")).unwrap(), b"FF");
    assert_eq!(std::fs::read(out.join("ffprobe.exe")).unwrap(), b"FP");
    assert_eq!(std::fs::read_dir(&out).unwrap().count(), 2);
}

#[test]
fn targz_extraction_and_missing_entry() {
    let d = tempfile::tempdir().unwrap();
    let archive = d.path().join("a.tgz");
    std::fs::write(
        &archive,
        targz_bytes(&[("bin/pdfium.dll", b"DLL"), ("LICENSE", b"l")]),
    )
    .unwrap();
    let out = d.path().join("out");
    std::fs::create_dir(&out).unwrap();
    extract_archive(
        Archive::TarGz,
        &archive,
        &[map("bin/pdfium.dll", "pdfium.dll")],
        &out,
    )
    .unwrap();
    assert_eq!(std::fs::read(out.join("pdfium.dll")).unwrap(), b"DLL");
    let missing = extract_archive(
        Archive::TarGz,
        &archive,
        &[map("bin/nope.dll", "nope.dll")],
        &out,
    );
    assert!(matches!(missing, Err(Error::Failed(_))));
}

#[test]
fn plain_file_archive_is_copied() {
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("dl");
    std::fs::write(&src, b"EXE").unwrap();
    let out = d.path().join("out");
    std::fs::create_dir(&out).unwrap();
    extract_archive(
        Archive::None,
        &src,
        &[map("yt-dlp.exe", "yt-dlp.exe")],
        &out,
    )
    .unwrap();
    assert_eq!(std::fs::read(out.join("yt-dlp.exe")).unwrap(), b"EXE");
}

/// Serve `body` for every request on a random local port; returns the base URL.
fn serve(body: Vec<u8>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { break };
            let mut req = [0u8; 2048];
            let _ = s.read(&mut req);
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = s.write_all(head.as_bytes());
            let _ = s.write_all(&body);
        }
    });
    format!("http://127.0.0.1:{port}/pkg.zip")
}

fn manifest_for(url: &str, sha: &str) -> Manifest {
    serde_json::from_value(serde_json::json!({
        "schema": 1,
        "engines": [{
            "id": "demo", "name": "Demo", "version": "1.2.3", "purpose": "test",
            "license": "MIT", "homepage": "https://example.com", "url": url,
            "sha256": sha, "size": 100, "archive": "zip",
            "files": [{ "from": "bin/demo.exe", "to": "demo.exe" }],
            "system": ["fc-definitely-not-installed.exe"]
        }]
    }))
    .unwrap()
}

#[test]
fn install_verifies_extracts_and_uninstalls() {
    let zip = zip_bytes(&[("top/bin/demo.exe", b"DEMO")]);
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("p.zip");
    std::fs::write(&archive, &zip).unwrap();
    let sha = sha256_file(&archive).unwrap();
    let url = serve(zip);

    let root = tmp.path().join("engines");
    let mgr = EngineManager::new(root.clone(), manifest_for(&url, &sha));
    assert_eq!(mgr.status()[0].source, ToolSource::Missing);
    assert!(mgr.locate("demo", "demo.exe").is_none());

    let last = std::sync::Mutex::new((0u64, 0u64));
    mgr.install(
        "demo",
        &|d, t| *last.lock().unwrap() = (d, t),
        &CancelToken::new(),
    )
    .unwrap();
    let (done, total) = *last.lock().unwrap();
    assert!(done > 0 && done == total);

    let st = &mgr.status()[0];
    assert_eq!(st.source, ToolSource::Managed);
    assert_eq!(st.version.as_deref(), Some("1.2.3"));
    assert_eq!(
        mgr.locate("demo", "demo.exe"),
        Some(root.join("demo").join("demo.exe"))
    );
    assert!(root.join("demo").join("installed.json").is_file());
    assert!(!root.join(".staging").join("demo.download").exists());

    // Reinstall over an existing install works.
    mgr.install("demo", &|_, _| {}, &CancelToken::new())
        .unwrap();

    mgr.uninstall("demo").unwrap();
    assert!(!root.join("demo").exists());
    assert_eq!(mgr.status()[0].source, ToolSource::Missing);
}

fn installed_demo(tmp: &tempfile::TempDir) -> (EngineManager, std::path::PathBuf) {
    let zip = zip_bytes(&[("bin/demo.exe", b"DEMO")]);
    let archive = tmp.path().join("p.zip");
    std::fs::write(&archive, &zip).unwrap();
    let sha = sha256_file(&archive).unwrap();
    let url = serve(zip);
    let root = tmp.path().join("engines");
    let mgr = EngineManager::new(root.clone(), manifest_for(&url, &sha));
    mgr.install("demo", &|_, _| {}, &CancelToken::new())
        .unwrap();
    (mgr, root)
}

#[test]
fn reinstall_with_a_file_held_open_swaps_and_cleans_up_later() {
    let tmp = tempfile::tempdir().unwrap();
    let (mgr, root) = installed_demo(&tmp);
    let held = std::fs::File::open(root.join("demo").join("demo.exe")).unwrap();
    mgr.install("demo", &|_, _| {}, &CancelToken::new())
        .unwrap();
    // New install is in place and complete.
    assert!(root.join("demo").join("installed.json").is_file());
    assert_eq!(
        std::fs::read(root.join("demo").join("demo.exe")).unwrap(),
        b"DEMO"
    );
    drop(held);
    // Any `.old-*` leftover is removed by the next install.
    mgr.install("demo", &|_, _| {}, &CancelToken::new())
        .unwrap();
    let old: Vec<_> = std::fs::read_dir(&root)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".old-"))
        .collect();
    assert!(old.is_empty(), "{old:?}");
}

#[cfg(windows)]
#[test]
fn reinstall_while_the_managed_exe_is_running() {
    // Use a real executable (a copy of ping.exe) as the "tool" and keep it running.
    let ping = std::fs::read(r"C:\Windows\System32\PING.EXE").unwrap();
    let zip = zip_bytes(&[("bin/demo.exe", &ping)]);
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("p.zip");
    std::fs::write(&archive, &zip).unwrap();
    let sha = sha256_file(&archive).unwrap();
    let url = serve(zip);
    let root = tmp.path().join("engines");
    let mgr = EngineManager::new(root.clone(), manifest_for(&url, &sha));
    mgr.install("demo", &|_, _| {}, &CancelToken::new())
        .unwrap();

    let mut running = std::process::Command::new(root.join("demo").join("demo.exe"))
        .args(["-n", "20", "127.0.0.1"])
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    let r = mgr.install("demo", &|_, _| {}, &CancelToken::new());
    let _ = running.kill();
    let _ = running.wait();
    r.unwrap();
    assert!(root.join("demo").join("installed.json").is_file());
    assert!(root.join("demo").join("demo.exe").is_file());
    // The next install/startup sweeps leftovers.
    let _ = EngineManager::new(root.clone(), mgr.manifest().clone());
    let left: Vec<_> = std::fs::read_dir(root.join("demo"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".old-"))
        .collect();
    assert!(left.is_empty(), "{left:?}");
}

#[cfg(windows)]
#[test]
fn reinstall_blocked_by_a_locked_file_keeps_the_old_install_intact() {
    use std::os::windows::fs::OpenOptionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let (mgr, root) = installed_demo(&tmp);
    // Share read/write only: no delete/rename, like a running executable.
    let _held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(root.join("demo").join("demo.exe"))
        .unwrap();
    let r = mgr.install("demo", &|_, _| {}, &CancelToken::new());
    // Whatever the outcome, the tool must still be fully installed.
    let dir = root.join("demo");
    assert!(dir.join("installed.json").is_file(), "{r:?}");
    assert!(dir.join("demo.exe").is_file(), "{r:?}");
    assert_eq!(mgr.status()[0].source, ToolSource::Managed);
}

#[test]
fn checksum_mismatch_installs_nothing_and_cleans_staging() {
    let zip = zip_bytes(&[("bin/demo.exe", b"DEMO")]);
    let url = serve(zip);
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("engines");
    let mgr = EngineManager::new(root.clone(), manifest_for(&url, &"0".repeat(64)));
    let r = mgr.install("demo", &|_, _| {}, &CancelToken::new());
    assert!(matches!(r, Err(Error::Checksum(_))), "{r:?}");
    assert!(!root.join("demo").exists());
    let staging = root.join(".staging");
    assert!(!staging.exists() || std::fs::read_dir(staging).unwrap().count() == 0);
}

#[test]
fn cancelled_install_cleans_up() {
    let url = serve(zip_bytes(&[("bin/demo.exe", b"DEMO")]));
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("engines");
    let mgr = EngineManager::new(root.clone(), manifest_for(&url, &"0".repeat(64)));
    let cancel = CancelToken::new();
    cancel.cancel();
    let r = mgr.install("demo", &|_, _| {}, &cancel);
    assert!(matches!(r, Err(Error::Cancelled)), "{r:?}");
    assert!(!root.join("demo").exists());
}

#[test]
fn unknown_tool_and_unreachable_host() {
    let tmp = tempfile::tempdir().unwrap();
    let mgr = EngineManager::new(
        tmp.path().join("e"),
        manifest_for("http://127.0.0.1:1/x.zip", &"0".repeat(64)),
    );
    assert!(matches!(
        mgr.install("nope", &|_, _| {}, &CancelToken::new()),
        Err(Error::Unsupported(_))
    ));
    assert!(matches!(
        mgr.install("demo", &|_, _| {}, &CancelToken::new()),
        Err(Error::Network(_))
    ));
    assert!(mgr.locate("nope", "x.exe").is_none());
}

#[test]
fn builtin_manifest_status_lists_every_tool() {
    let tmp = tempfile::tempdir().unwrap();
    let mgr = EngineManager::new(tmp.path().join("e"), Manifest::builtin());
    let ids: Vec<_> = mgr.status().into_iter().map(|s| s.id).collect();
    assert_eq!(ids, ["ffmpeg", "yt-dlp", "deno", "pdfium"]);
}

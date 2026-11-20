//! Scanner behaviour with explicit clocks (no threads, no sleeping).

use fc_core::watch::{Scanner, WatchSpec, is_temp_name};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const SETTLE: Duration = Duration::from_secs(5);

fn spec(dir: &Path) -> WatchSpec {
    WatchSpec {
        id: "w".into(),
        dir: dir.to_path_buf(),
        recursive: false,
        exclude: Vec::new(),
        extensions: None,
    }
}

fn fired(sc: &mut Scanner, now: Instant) -> Vec<PathBuf> {
    sc.tick(now).into_iter().map(|(_, p)| p).collect()
}

#[test]
fn existing_files_never_fire() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("old.txt"), "x").unwrap();
    let t0 = Instant::now();
    let mut sc = Scanner::new(SETTLE);
    sc.set_specs(vec![spec(d.path())], t0);
    assert!(fired(&mut sc, t0).is_empty());
    assert!(fired(&mut sc, t0 + Duration::from_secs(60)).is_empty());
}

#[test]
fn waits_until_file_is_stable() {
    let d = tempfile::tempdir().unwrap();
    let t0 = Instant::now();
    let mut sc = Scanner::new(SETTLE);
    sc.set_specs(vec![spec(d.path())], t0);
    let f = d.path().join("a.txt");
    std::fs::write(&f, "one").unwrap();
    assert!(fired(&mut sc, t0).is_empty());
    std::fs::write(&f, "one two").unwrap();
    assert!(fired(&mut sc, t0 + Duration::from_secs(3)).is_empty());
    assert!(fired(&mut sc, t0 + Duration::from_secs(6)).is_empty());
    assert_eq!(
        fired(&mut sc, t0 + Duration::from_secs(9)),
        std::slice::from_ref(&f)
    );
    assert!(fired(&mut sc, t0 + Duration::from_secs(12)).is_empty());
    // A later change fires again.
    std::fs::write(&f, "one two three").unwrap();
    assert!(fired(&mut sc, t0 + Duration::from_secs(15)).is_empty());
    assert_eq!(fired(&mut sc, t0 + Duration::from_secs(21)), [f]);
}

#[test]
fn ignores_temp_names_and_excluded_dirs() {
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("converted");
    std::fs::create_dir(&out).unwrap();
    let t0 = Instant::now();
    let mut sc = Scanner::new(SETTLE);
    let mut s = spec(d.path());
    s.recursive = true;
    s.exclude = vec![out.clone()];
    sc.set_specs(vec![s], t0);
    std::fs::write(d.path().join("x.crdownload"), "1").unwrap();
    std::fs::write(d.path().join("~$doc.docx"), "1").unwrap();
    std::fs::write(out.join("result.png"), "1").unwrap();
    let ok = d.path().join("real.png");
    std::fs::write(&ok, "1").unwrap();
    fired(&mut sc, t0);
    assert_eq!(fired(&mut sc, t0 + Duration::from_secs(6)), [ok]);
    assert!(is_temp_name("a.PART") && is_temp_name(".~lock.x#") && !is_temp_name("a.png"));
}

#[test]
fn ignored_paths_never_fire() {
    let d = tempfile::tempdir().unwrap();
    let t0 = Instant::now();
    let mut sc = Scanner::new(SETTLE);
    sc.set_specs(vec![spec(d.path())], t0);
    let f = d.path().join("mine.png");
    std::fs::write(&f, "1").unwrap();
    sc.ignore(f);
    fired(&mut sc, t0);
    assert!(fired(&mut sc, t0 + Duration::from_secs(10)).is_empty());
}

#[test]
fn recursive_flag() {
    let t0 = Instant::now();
    for (rec, expect) in [(false, 1usize), (true, 2)] {
        let d = tempfile::tempdir().unwrap();
        let sub = d.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        let mut sc = Scanner::new(SETTLE);
        let mut s = spec(d.path());
        s.recursive = rec;
        sc.set_specs(vec![s], t0);
        std::fs::write(d.path().join("top.txt"), "1").unwrap();
        std::fs::write(sub.join("deep.txt"), "1").unwrap();
        fired(&mut sc, t0);
        let got = fired(&mut sc, t0 + Duration::from_secs(6));
        assert_eq!(got.len(), expect, "recursive={rec}: {got:?}");
    }
}

#[test]
fn extension_filter() {
    let d = tempfile::tempdir().unwrap();
    let t0 = Instant::now();
    let mut sc = Scanner::new(SETTLE);
    let mut s = spec(d.path());
    s.extensions = Some(vec!["PNG".into(), "jpg".into()]);
    sc.set_specs(vec![s], t0);
    let png = d.path().join("a.png");
    std::fs::write(&png, "1").unwrap();
    std::fs::write(d.path().join("b.txt"), "1").unwrap();
    fired(&mut sc, t0);
    assert_eq!(fired(&mut sc, t0 + Duration::from_secs(6)), [png]);
}

#[test]
fn missing_folder_reports_error() {
    let d = tempfile::tempdir().unwrap();
    let gone = d.path().join("nope");
    let t0 = Instant::now();
    let mut sc = Scanner::new(SETTLE);
    sc.set_specs(vec![spec(&gone)], t0);
    assert!(fired(&mut sc, t0).is_empty());
    let errs = sc.errors();
    assert_eq!(errs.len(), 1);
    assert_eq!(errs[0].0, "w");
    // Folder appears later: the error clears.
    std::fs::create_dir(&gone).unwrap();
    fired(&mut sc, t0 + Duration::from_secs(1));
    assert!(sc.errors().is_empty());
}

//! Job naming, queue and executor routing. Uses fake executors; no real engines are called.

use fc_core::engines::{DefaultExecutor, Executor, ToolLocator};
use fc_core::jobs::naming::{commit, output_path, temp_path};
use fc_core::jobs::queue::JobQueue;
use fc_core::jobs::{
    CompressLevel, ConvertOptions, JobId, JobRequest, JobSnapshot, JobState, Operation,
};
use fc_core::{CancelToken, Error, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ---------- naming ----------

fn touch(p: &Path) {
    fs::write(p, b"x").unwrap();
}

#[test]
fn output_next_to_source_and_collisions() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("photo.png");
    touch(&input);
    let first = output_path(&input, "webp", None, None);
    assert_eq!(first, dir.path().join("photo.webp"));
    touch(&first);
    let second = output_path(&input, "webp", None, None);
    assert_eq!(second, dir.path().join("photo (1).webp"));
    touch(&second);
    assert_eq!(
        output_path(&input, "webp", None, None),
        dir.path().join("photo (2).webp")
    );
}

#[test]
fn output_never_returns_input() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.png");
    touch(&input);
    assert_eq!(
        output_path(&input, "png", None, None),
        dir.path().join("a (1).png")
    );
}

#[test]
fn output_dir_and_suffix() {
    let dir = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    let input = dir.path().join("clip.mp4");
    touch(&input);
    assert_eq!(
        output_path(&input, "mp4", Some(out.path()), Some(" (compressed)")),
        out.path().join("clip (compressed).mp4")
    );
}

#[test]
fn reserved_device_names_get_underscore() {
    let dir = tempfile::tempdir().unwrap();
    for (stem, want) in [
        ("CON", "CON_"),
        ("nul", "nul_"),
        ("Com3", "Com3_"),
        ("lpt9", "lpt9_"),
    ] {
        let input = dir.path().join(format!("{stem}.txt"));
        assert_eq!(
            output_path(&input, "html", None, None),
            dir.path().join(format!("{want}.html")),
            "{stem}"
        );
    }
    // Not reserved.
    let input = dir.path().join("COM10.txt");
    assert_eq!(
        output_path(&input, "html", None, None),
        dir.path().join("COM10.html")
    );
}

#[test]
fn temp_path_keeps_extension_in_same_dir() {
    let p = Path::new("C:/out/a.webm");
    assert_eq!(temp_path(p), PathBuf::from("C:/out/a.fcpart.webm"));
}

#[test]
fn commit_moves_and_avoids_existing() {
    let dir = tempfile::tempdir().unwrap();
    let final_path = dir.path().join("a.webp");
    let t = temp_path(&final_path);
    fs::write(&t, b"one").unwrap();
    let got = commit(&t, &final_path).unwrap();
    assert_eq!(got, final_path);
    assert!(!t.exists());
    assert_eq!(fs::read(&final_path).unwrap(), b"one");

    fs::write(&t, b"two").unwrap();
    let got = commit(&t, &final_path).unwrap();
    assert_eq!(got, dir.path().join("a (1).webp"));
    assert_eq!(
        fs::read(&final_path).unwrap(),
        b"one",
        "existing file untouched"
    );
    assert_eq!(fs::read(&got).unwrap(), b"two");
}

#[test]
fn commit_race_gives_distinct_files() {
    let dir = tempfile::tempdir().unwrap();
    let final_path = dir.path().join("race.png");
    let mut handles = Vec::new();
    for i in 0..8 {
        let dir = dir.path().to_path_buf();
        let final_path = final_path.clone();
        handles.push(std::thread::spawn(move || {
            let t = dir.join(format!("t{i}.tmp"));
            fs::write(&t, format!("{i}")).unwrap();
            commit(&t, &final_path).unwrap()
        }));
    }
    let mut got: Vec<PathBuf> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    got.sort();
    got.dedup();
    assert_eq!(got.len(), 8, "every commit must land on its own name");
    for p in &got {
        assert!(p.exists());
    }
}

// ---------- queue ----------

type Log = Arc<Mutex<Vec<JobSnapshot>>>;

fn recording_queue(exec: Arc<dyn Executor>) -> (JobQueue, Log) {
    let log: Log = Arc::default();
    let l = log.clone();
    let q = JobQueue::new(exec, Arc::new(move |s| l.lock().unwrap().push(s.clone())));
    (q, log)
}

fn wait_until(what: &str, mut cond: impl FnMut() -> bool) {
    let start = Instant::now();
    while !cond() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "timed out: {what}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn state_of(q: &JobQueue, id: JobId) -> JobState {
    q.snapshot().iter().find(|s| s.id == id).unwrap().state
}

fn convert(inputs: &[&str], to: &str) -> JobRequest {
    JobRequest {
        inputs: inputs.iter().map(PathBuf::from).collect(),
        op: Operation::Convert { to: to.into() },
        options: ConvertOptions::default(),
    }
}

/// Runs instantly, records the order of inputs.
struct Recorder(Mutex<Vec<PathBuf>>);
impl Executor for Recorder {
    fn run(
        &self,
        inputs: &[PathBuf],
        _op: &Operation,
        _o: &ConvertOptions,
        _c: &CancelToken,
        progress: &dyn Fn(f32),
    ) -> Result<Vec<PathBuf>> {
        self.0.lock().unwrap().push(inputs[0].clone());
        progress(0.5);
        Ok(vec![inputs[0].with_extension("out")])
    }
}

#[test]
fn single_pool_runs_in_submission_order() {
    let rec = Arc::new(Recorder(Mutex::default()));
    let (q, log) = recording_queue(rec.clone());
    // mp4 -> webm is an ffmpeg route: one worker, strict FIFO.
    let ids = q.submit(convert(&["a.mp4", "b.mp4", "c.mp4", "d.mp4"], "webm"));
    assert_eq!(ids.len(), 4);
    wait_until("all done", || {
        ids.iter().all(|i| state_of(&q, *i) == JobState::Done)
    });
    let order: Vec<_> = rec
        .0
        .lock()
        .unwrap()
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    assert_eq!(order, ["a.mp4", "b.mp4", "c.mp4", "d.mp4"]);

    // Every job went Queued -> Running -> Done, in that order.
    let log = log.lock().unwrap();
    for id in &ids {
        let states: Vec<JobState> = log
            .iter()
            .filter(|s| s.id == *id)
            .map(|s| s.state)
            .collect();
        assert_eq!(states.first(), Some(&JobState::Queued));
        assert_eq!(states.last(), Some(&JobState::Done));
        assert!(states.contains(&JobState::Running));
    }
    let done = q.snapshot();
    // Finished: newest first.
    assert_eq!(done[0].id, ids[3]);
    assert_eq!(done[0].progress, 1.0);
    assert_eq!(done[0].outputs, vec![PathBuf::from("d.out")]);
}

#[test]
fn merge_is_one_job_other_ops_are_per_input() {
    let rec = Arc::new(Recorder(Mutex::default()));
    let (q, _log) = recording_queue(rec);
    let merge = q.submit(JobRequest {
        inputs: vec!["a.pdf".into(), "b.png".into(), "c.pdf".into()],
        op: Operation::MergePdf,
        options: ConvertOptions::default(),
    });
    assert_eq!(merge.len(), 1);
    let split = q.submit(JobRequest {
        inputs: vec!["a.pdf".into(), "b.pdf".into()],
        op: Operation::SplitPdf,
        options: ConvertOptions::default(),
    });
    assert_eq!(split.len(), 2);
    assert!(q.submit(convert(&[], "png")).is_empty());
}

#[test]
fn zip_is_one_job_and_shell_only_ops_are_never_queued() {
    let rec = Arc::new(Recorder(Mutex::default()));
    let (q, log) = recording_queue(rec);
    let req = |op: Operation| JobRequest {
        inputs: vec!["a.png".into(), "b.txt".into()],
        op,
        options: ConvertOptions::default(),
    };
    assert_eq!(q.submit(req(Operation::Zip)).len(), 1);
    let dir = Some(PathBuf::from("out"));
    let copies = q.submit(req(Operation::CopyTo {
        dir,
        move_files: false,
    }));
    assert_eq!(copies.len(), 2);
    for op in [
        Operation::Annotate,
        Operation::CopyTo {
            dir: None,
            move_files: true,
        },
    ] {
        assert!(matches!(op.queueable(), Err(Error::Invalid(_))));
        assert!(q.submit(req(op)).is_empty());
    }
    wait_until("all done", || {
        q.snapshot().iter().all(|s| s.state.is_finished())
    });
    assert_eq!(q.snapshot().len(), 3);
    assert!(log.lock().unwrap().iter().all(|s| !matches!(
        s.op,
        Operation::Annotate | Operation::CopyTo { dir: None, .. }
    )));
}

/// Loops until cancelled.
struct Spinner;
impl Executor for Spinner {
    fn run(
        &self,
        _i: &[PathBuf],
        _op: &Operation,
        _o: &ConvertOptions,
        cancel: &CancelToken,
        _p: &dyn Fn(f32),
    ) -> Result<Vec<PathBuf>> {
        while !cancel.is_cancelled() {
            std::thread::sleep(Duration::from_millis(5));
        }
        Err(Error::Cancelled)
    }
}

#[test]
fn cancel_queued_and_running() {
    let (q, log) = recording_queue(Arc::new(Spinner));
    let ids = q.submit(convert(&["a.mp4", "b.mp4"], "webm"));
    wait_until("first running", || {
        state_of(&q, ids[0]) == JobState::Running
    });
    assert_eq!(state_of(&q, ids[1]), JobState::Queued);

    // Queued: cancelled immediately, never runs.
    assert!(q.cancel(ids[1]));
    assert_eq!(state_of(&q, ids[1]), JobState::Cancelled);
    assert!(!q.cancel(ids[1]), "already finished");
    assert!(!q.cancel(9999), "unknown id");

    // Running: stays Running until the executor returns, then Cancelled.
    assert!(q.cancel(ids[0]));
    wait_until("running cancelled", || {
        state_of(&q, ids[0]) == JobState::Cancelled
    });
    let log = log.lock().unwrap();
    assert!(
        !log.iter()
            .any(|s| s.id == ids[1] && s.state == JobState::Running)
    );
    assert!(
        log.iter()
            .any(|s| s.id == ids[1] && s.state == JobState::Cancelled)
    );
}

struct Chatty;
impl Executor for Chatty {
    fn run(
        &self,
        i: &[PathBuf],
        _op: &Operation,
        _o: &ConvertOptions,
        _c: &CancelToken,
        progress: &dyn Fn(f32),
    ) -> Result<Vec<PathBuf>> {
        for k in 0..=1000 {
            progress(k as f32 / 1000.0);
        }
        Ok(i.to_vec())
    }
}

#[test]
fn progress_is_throttled_but_final_state_is_emitted() {
    let (q, log) = recording_queue(Arc::new(Chatty));
    let ids = q.submit(convert(&["a.png"], "jpg"));
    wait_until("done", || state_of(&q, ids[0]) == JobState::Done);
    let log = log.lock().unwrap();
    let running_updates = log.iter().filter(|s| s.state == JobState::Running).count();
    assert!(
        running_updates < 20,
        "1001 progress calls should be throttled, got {running_updates}"
    );
    let last = log.last().unwrap();
    assert_eq!(last.state, JobState::Done);
    assert_eq!(last.progress, 1.0);
    assert!(last.finished_at.is_some());
    // The explicit 1.0 progress call is always emitted while still running.
    assert!(
        log.iter()
            .any(|s| s.state == JobState::Running && s.progress == 1.0)
    );
}

struct Failing(Error);
impl Executor for Failing {
    fn run(
        &self,
        _i: &[PathBuf],
        _op: &Operation,
        _o: &ConvertOptions,
        _c: &CancelToken,
        _p: &dyn Fn(f32),
    ) -> Result<Vec<PathBuf>> {
        Err(match &self.0 {
            Error::MissingTool(t) => Error::MissingTool(t.clone()),
            _ => Error::Failed("boom".into()),
        })
    }
}

#[test]
fn failure_carries_code_and_message() {
    let (q, _l) = recording_queue(Arc::new(Failing(Error::MissingTool("ffmpeg".into()))));
    let id = q.submit(convert(&["a.png"], "jpg"))[0];
    wait_until("failed", || state_of(&q, id) == JobState::Failed);
    let s = &q.snapshot()[0];
    assert_eq!(s.error_code.as_deref(), Some("missingTool"));
    assert_eq!(s.error.as_deref(), Some("missing tool: ffmpeg"));
}

struct Panicky;
impl Executor for Panicky {
    fn run(
        &self,
        _i: &[PathBuf],
        _op: &Operation,
        _o: &ConvertOptions,
        _c: &CancelToken,
        _p: &dyn Fn(f32),
    ) -> Result<Vec<PathBuf>> {
        panic!("executor bug");
    }
}

#[test]
fn panicking_executor_fails_the_job_and_queue_keeps_working() {
    let (q, _l) = recording_queue(Arc::new(Panicky));
    let ids = q.submit(convert(&["a.mp4", "b.mp4"], "webm"));
    wait_until("both failed", || {
        ids.iter().all(|i| state_of(&q, *i) == JobState::Failed)
    });
}

#[test]
fn keeps_at_most_100_finished_and_clear_works() {
    let rec = Arc::new(Recorder(Mutex::default()));
    let (q, _l) = recording_queue(rec.clone());
    let names: Vec<String> = (0..130).map(|i| format!("f{i}.png")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let ids = q.submit(convert(&refs, "jpg"));
    wait_until("all finished", || {
        let snap = q.snapshot();
        rec.0.lock().unwrap().len() == 130 && snap.iter().all(|s| s.state.is_finished())
    });
    let snap = q.snapshot();
    assert_eq!(snap.len(), 100);
    // 30 oldest finished jobs were dropped; the rest are listed newest first.
    assert_eq!(
        ids.iter()
            .filter(|i| snap.iter().all(|s| s.id != **i))
            .count(),
        30
    );
    let times: Vec<u64> = snap.iter().map(|s| s.finished_at.unwrap()).collect();
    assert!(times.windows(2).all(|w| w[0] >= w[1]));
    q.clear_finished();
    assert!(q.snapshot().is_empty());
}

// ---------- executor routing ----------

struct NoTools;
impl ToolLocator for NoTools {
    fn locate(&self, _t: &str, _f: &str) -> Option<PathBuf> {
        None
    }
}

fn run_default(inputs: &[PathBuf], op: Operation) -> Result<Vec<PathBuf>> {
    let ex = DefaultExecutor {
        tools: Arc::new(NoTools),
    };
    ex.run(
        inputs,
        &op,
        &ConvertOptions::default(),
        &CancelToken::new(),
        &|_| {},
    )
}

fn dir_entries(dir: &Path) -> usize {
    fs::read_dir(dir).unwrap().count()
}

#[test]
fn mp4_to_webm_without_ffmpeg_is_missing_tool_and_leaves_no_files() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("v.mp4");
    touch(&input);
    let err = run_default(&[input], Operation::Convert { to: "webm".into() }).unwrap_err();
    assert!(
        matches!(&err, Error::MissingTool(t) if t == "ffmpeg"),
        "{err:?}"
    );
    assert_eq!(dir_entries(dir.path()), 1);
}

#[test]
fn pdf_to_png_without_pdfium_is_missing_tool() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("d.pdf");
    touch(&input);
    let err = run_default(&[input], Operation::Convert { to: "png".into() }).unwrap_err();
    assert!(
        matches!(&err, Error::MissingTool(t) if t == "pdfium"),
        "{err:?}"
    );
}

#[test]
fn media_operations_without_ffmpeg_are_missing_tool() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("v.mp4");
    touch(&input);
    for op in [
        Operation::Compress {
            level: CompressLevel::Light,
        },
        Operation::Trim {
            start_ms: 0,
            end_ms: 10,
        },
        Operation::Mute,
        Operation::Speed { factor: 2.0 },
        Operation::Crop {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        },
    ] {
        let err = run_default(std::slice::from_ref(&input), op).unwrap_err();
        assert!(
            matches!(&err, Error::MissingTool(t) if t == "ffmpeg"),
            "{err:?}"
        );
    }
    assert_eq!(dir_entries(dir.path()), 1);
}

#[test]
fn bad_inputs_are_reported_precisely() {
    let dir = tempfile::tempdir().unwrap();
    // Missing file.
    let err = run_default(
        &[dir.path().join("nope.png")],
        Operation::Convert { to: "jpg".into() },
    )
    .unwrap_err();
    assert!(matches!(err, Error::NotFound(_)));

    // Unknown extension.
    let unknown = dir.path().join("x.xyz");
    touch(&unknown);
    let err = run_default(&[unknown], Operation::Convert { to: "jpg".into() }).unwrap_err();
    assert!(matches!(err, Error::Unsupported(_)));

    // No route.
    let png = dir.path().join("a.png");
    touch(&png);
    let err = run_default(
        std::slice::from_ref(&png),
        Operation::Convert { to: "mp4".into() },
    )
    .unwrap_err();
    assert!(matches!(err, Error::Unsupported(_)));

    // Split needs a pdf; trim needs media; compress of data is unsupported.
    let err = run_default(std::slice::from_ref(&png), Operation::SplitPdf).unwrap_err();
    assert!(matches!(err, Error::Unsupported(_)));
    let err = run_default(
        &[png],
        Operation::Trim {
            start_ms: 0,
            end_ms: 1,
        },
    )
    .unwrap_err();
    assert!(matches!(err, Error::Unsupported(_)));
    // Nothing was created next to the inputs.
    assert_eq!(dir_entries(dir.path()), 2);
}

#[test]
fn cancelled_token_stops_before_any_work() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.png");
    touch(&input);
    let ex = DefaultExecutor {
        tools: Arc::new(NoTools),
    };
    let cancel = CancelToken::new();
    cancel.cancel();
    let err = ex
        .run(
            &[input],
            &Operation::Convert { to: "jpg".into() },
            &ConvertOptions::default(),
            &cancel,
            &|_| {},
        )
        .unwrap_err();
    assert!(matches!(err, Error::Cancelled));
    assert_eq!(dir_entries(dir.path()), 1);
}

#[test]
fn output_name_replaces_the_input_stem_and_never_adds_folders() {
    let dir = tempfile::tempdir().unwrap();
    let src_dir = dir.path().join("clipboard").join("img");
    let out_dir = dir.path().join("Downloads");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&out_dir).unwrap();
    let input = src_dir.join("17.png");
    image::RgbaImage::from_pixel(4, 3, image::Rgba([10, 20, 30, 255]))
        .save(&input)
        .unwrap();
    let ex = DefaultExecutor {
        tools: Arc::new(NoTools),
    };
    let run = |name: &str| {
        let options = ConvertOptions {
            output_dir: Some(out_dir.clone()),
            output_name: Some(name.into()),
            ..ConvertOptions::default()
        };
        ex.run(
            std::slice::from_ref(&input),
            &Operation::Convert { to: "jpg".into() },
            &options,
            &CancelToken::new(),
            &|_| {},
        )
        .unwrap()
    };
    let stamp = "Clipboard 2026-11-20 10.11.12";
    assert_eq!(run(stamp), vec![out_dir.join(format!("{stamp}.jpg"))]);
    assert_eq!(run(stamp), vec![out_dir.join(format!("{stamp} (1).jpg"))]);
    // Folder parts in the name are ignored.
    assert_eq!(run("../../evil"), vec![out_dir.join("evil.jpg")]);
    // The source folder is left alone.
    assert_eq!(dir_entries(&src_dir), 1);
}

// ---------- fix round 1 ----------

#[test]
fn reserved_name_is_fixed_after_first_dot_segment() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("con.tar.gz");
    assert_eq!(
        output_path(&input, "html", None, None),
        dir.path().join("con_.tar.html")
    );
    let made = fc_core::jobs::naming::unique_dir(dir.path(), "nul.v2").unwrap();
    assert_eq!(made, dir.path().join("nul_.v2"));
}

/// Tracks how many jobs of each input extension run at the same time.
#[derive(Default)]
struct Concurrency {
    now: Mutex<std::collections::HashMap<String, usize>>,
    max: Mutex<std::collections::HashMap<String, usize>>,
}
impl Executor for Concurrency {
    fn run(
        &self,
        inputs: &[PathBuf],
        _op: &Operation,
        _o: &ConvertOptions,
        _c: &CancelToken,
        _p: &dyn Fn(f32),
    ) -> Result<Vec<PathBuf>> {
        let ext = inputs[0]
            .extension()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        {
            let mut now = self.now.lock().unwrap();
            let n = now.entry(ext.clone()).or_default();
            *n += 1;
            let mut max = self.max.lock().unwrap();
            let m = max.entry(ext.clone()).or_default();
            *m = (*m).max(*n);
        }
        std::thread::sleep(Duration::from_millis(60));
        *self.now.lock().unwrap().get_mut(&ext).unwrap() -= 1;
        Ok(vec![])
    }
}

#[test]
fn mixed_request_keeps_ffmpeg_jobs_serial() {
    let ex = Arc::new(Concurrency::default());
    let (q, _l) = recording_queue(ex.clone());
    let ids = q.submit(convert(
        &["a.png", "b.mp4", "c.mp4", "d.mp4", "e.mp4"],
        "gif",
    ));
    wait_until("done", || {
        ids.iter().all(|i| state_of(&q, *i) == JobState::Done)
    });
    assert_eq!(
        ex.max.lock().unwrap()["mp4"],
        1,
        "ffmpeg route must never overlap"
    );
}

/// Blocks until released.
struct Gate(std::sync::atomic::AtomicBool);
impl Executor for Gate {
    fn run(
        &self,
        _i: &[PathBuf],
        _op: &Operation,
        _o: &ConvertOptions,
        _c: &CancelToken,
        _p: &dyn Fn(f32),
    ) -> Result<Vec<PathBuf>> {
        while !self.0.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(vec![])
    }
}

#[test]
fn second_submit_gets_a_worker_while_first_still_runs() {
    if std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        < 4
    {
        eprintln!("skipped: cpu pool has a single slot on this machine");
        return;
    }
    let gate = Arc::new(Gate(Default::default()));
    let (q, _l) = recording_queue(gate.clone());
    let a = q.submit(convert(&["a.png"], "jpg"))[0];
    wait_until("A running", || state_of(&q, a) == JobState::Running);
    let b = q.submit(convert(&["b.png"], "jpg"))[0];
    wait_until("B running while A runs", || {
        state_of(&q, b) == JobState::Running
    });
    assert_eq!(state_of(&q, a), JobState::Running);
    gate.0.store(true, std::sync::atomic::Ordering::SeqCst);
    wait_until("both done", || {
        state_of(&q, a) == JobState::Done && state_of(&q, b) == JobState::Done
    });
}

#[test]
fn panicking_listener_does_not_kill_workers() {
    let rec = Arc::new(Recorder(Mutex::default()));
    let first = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let f = first.clone();
    let q = JobQueue::new(
        rec,
        Arc::new(move |_s| {
            if f.swap(false, std::sync::atomic::Ordering::SeqCst) {
                panic!("listener bug");
            }
        }),
    );
    // Single pool: if the worker died, the later jobs would never run.
    let ids = q.submit(convert(&["a.mp4", "b.mp4", "c.mp4"], "webm"));
    wait_until("all done", || {
        ids.iter().all(|i| state_of(&q, *i) == JobState::Done)
    });
    let late = q.submit(convert(&["d.mp4"], "webm"))[0];
    wait_until("late job", || state_of(&q, late) == JobState::Done);
}

#[test]
fn file_actions_are_not_queueable() {
    for op in [
        Operation::CopyPath,
        Operation::OpenTerminal,
        Operation::Float,
    ] {
        assert!(matches!(op.queueable(), Err(Error::Invalid(_))), "{op:?}");
    }
}

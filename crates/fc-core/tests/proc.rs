use fc_core::{CancelToken, Error, proc};
use std::time::{Duration, Instant};

#[cfg(windows)]
#[test]
fn captures_and_streams_stdout() {
    let mut cmd = proc::command("cmd");
    cmd.args(["/C", "echo hello"]);
    let mut lines = Vec::new();
    let out = proc::run_streaming(
        cmd,
        &CancelToken::new(),
        &mut |l| lines.push(l.to_string()),
        &mut |_| {},
    )
    .unwrap();
    assert!(out.status.success());
    assert_eq!(lines, vec!["hello"]);
    assert_eq!(out.stdout.trim(), "hello");
}

#[cfg(windows)]
#[test]
fn captures_stderr_separately() {
    let mut cmd = proc::command("cmd");
    cmd.args(["/C", "echo oops 1>&2"]);
    let out = proc::run(cmd, &CancelToken::new()).unwrap();
    assert_eq!(out.stderr.trim(), "oops");
    assert!(out.stdout.is_empty());
}

#[cfg(windows)]
#[test]
fn cancel_kills_process_quickly() {
    let mut cmd = proc::command("ping");
    cmd.args(["-n", "30", "127.0.0.1"]);
    let cancel = CancelToken::new();
    let c2 = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        c2.cancel();
    });
    let t = Instant::now();
    let r = proc::run(cmd, &cancel);
    assert!(matches!(r, Err(Error::Cancelled)));
    assert!(t.elapsed() < Duration::from_secs(2), "{:?}", t.elapsed());
}

#[cfg(windows)]
#[test]
fn cancelled_child_is_really_gone() {
    // Spawn through the same path, cancel, then verify the pid no longer exists.
    let mut cmd = proc::command("ping");
    cmd.args(["-n", "31", "127.0.0.1"]);
    let cancel = CancelToken::new();
    let c2 = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        c2.cancel();
    });
    let _ = proc::run(cmd, &cancel);
    // `-n 31` is unique to this test: no ping with that count may remain.
    let out = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "(Get-CimInstance Win32_Process -Filter \"Name='PING.EXE'\" | Where-Object { $_.CommandLine -like '*-n 31 *' } | Measure-Object).Count",
        ])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "0");
}

#[test]
fn pre_cancelled_does_not_spawn() {
    let c = CancelToken::new();
    c.cancel();
    let r = proc::run(proc::command("definitely-not-a-program"), &c);
    assert!(matches!(r, Err(Error::Cancelled)));
}

#[test]
fn missing_program_is_io_error() {
    let r = proc::run(
        proc::command("definitely-not-a-program"),
        &CancelToken::new(),
    );
    assert!(matches!(r, Err(Error::Io(_))));
}

#[cfg(windows)]
#[test]
fn large_output_does_not_deadlock() {
    let mut cmd = proc::command("cmd");
    cmd.args([
        "/C",
        "for /L %i in (1,1,20000) do @echo line-%i-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
    ]);
    let mut n = 0usize;
    let out = proc::run_streaming(cmd, &CancelToken::new(), &mut |_| n += 1, &mut |_| {}).unwrap();
    assert_eq!(n, 20000);
    assert!(out.stdout.len() <= 4 * 1024 * 1024);
}

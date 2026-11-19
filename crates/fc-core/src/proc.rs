//! Child processes: no console window, killed with the app (Windows Job Object with
//! KILL_ON_JOB_CLOSE, created once and shared), cancellable, line-streamed output.

use crate::{CancelToken, Error, Result};
use std::io::{BufRead, BufReader, Read};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Cap on captured text per stream.
const CAPTURE_CAP: usize = 4 * 1024 * 1024;
/// How long to keep draining pipes after the child exited (a grandchild may hold them open).
const DRAIN_GRACE: Duration = Duration::from_millis(500);

/// `Command` with `CREATE_NO_WINDOW` on Windows and stdin set to null.
pub fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut cmd = Command::new(program);
    cmd.stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Output of a finished process (both streams fully captured, each capped at 4 MB).
pub struct Output {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

#[cfg(windows)]
mod job {
    use std::ffi::c_void;
    use std::process::Child;
    use std::sync::OnceLock;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };

    /// Raw handle value; the job lives for the whole process and is never closed explicitly,
    /// so the OS closes it (and kills the children) when the app exits.
    static JOB: OnceLock<Option<isize>> = OnceLock::new();

    fn create() -> Option<isize> {
        // SAFETY: plain Win32 calls with valid, correctly sized arguments.
        unsafe {
            let handle = CreateJobObjectW(None, None).ok()?;
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
            .ok()?;
            Some(handle.0 as isize)
        }
    }

    /// Best effort: failure to join the job never fails the spawn.
    pub fn assign(child: &Child) {
        use std::os::windows::io::AsRawHandle;
        if let Some(job) = JOB.get_or_init(create) {
            // SAFETY: both handles are valid for the duration of the call.
            unsafe {
                let _ = AssignProcessToJobObject(
                    HANDLE(*job as *mut c_void),
                    HANDLE(child.as_raw_handle()),
                );
            }
        }
    }
}

fn read_lines<R: Read>(reader: R, tx: mpsc::Sender<(bool, String)>, is_err: bool) {
    let mut reader = BufReader::new(reader);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let line = String::from_utf8_lossy(&buf);
                let line = line.trim_end_matches(['\n', '\r']).to_string();
                if tx.send((is_err, line)).is_err() {
                    break;
                }
            }
        }
    }
}

fn capture(into: &mut String, line: &str) {
    if into.len() + line.len() < CAPTURE_CAP {
        into.push_str(line);
        into.push('\n');
    }
}

/// Spawn `cmd` (stdout/stderr piped), assign it to the shared kill-on-close job, stream every
/// stdout line to `on_stdout` and every stderr line to `on_stderr` (lossy UTF-8, trimmed of
/// `\r`), and poll `cancel` at least every 100 ms. On cancel: kill the process (and its tree via
/// the job object), wait, return `Err(Error::Cancelled)`. Never deadlocks on full pipes.
pub fn run_streaming(
    mut cmd: Command,
    cancel: &CancelToken,
    on_stdout: &mut dyn FnMut(&str),
    on_stderr: &mut dyn FnMut(&str),
) -> Result<Output> {
    if cancel.is_cancelled() {
        return Err(Error::Cancelled);
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn()?;
    #[cfg(windows)]
    job::assign(&child);

    let (tx, rx) = mpsc::channel::<(bool, String)>();
    if let Some(out) = child.stdout.take() {
        let tx = tx.clone();
        std::thread::spawn(move || read_lines(out, tx, false));
    }
    if let Some(err) = child.stderr.take() {
        let tx = tx.clone();
        std::thread::spawn(move || read_lines(err, tx, true));
    }
    drop(tx);

    let (mut stdout, mut stderr) = (String::new(), String::new());
    let mut status: Option<ExitStatus> = None;
    let mut exited_at: Option<Instant> = None;
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok((false, line)) => {
                capture(&mut stdout, &line);
                on_stdout(&line);
            }
            Ok((true, line)) => {
                capture(&mut stderr, &line);
                on_stderr(&line);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if cancel.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::Cancelled);
        }
        if status.is_none() {
            if let Some(s) = child.try_wait()? {
                status = Some(s);
                exited_at = Some(Instant::now());
            }
        } else if exited_at.is_some_and(|t| t.elapsed() > DRAIN_GRACE) {
            break;
        }
    }
    let status = match status {
        Some(s) => s,
        None => child.wait()?,
    };
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

/// Convenience: run to completion, capture output, no streaming callbacks.
pub fn run(cmd: Command, cancel: &CancelToken) -> Result<Output> {
    run_streaming(cmd, cancel, &mut |_| {}, &mut |_| {})
}

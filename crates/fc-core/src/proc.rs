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
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject,
    };

    /// App-wide job (raw handle value). Never closed explicitly: the OS closes it, and kills
    /// every child, when the app exits.
    static APP_JOB: OnceLock<Option<isize>> = OnceLock::new();

    fn create() -> Option<isize> {
        // SAFETY: plain Win32 calls with valid, correctly sized arguments.
        unsafe {
            let handle = CreateJobObjectW(None, None).ok()?;
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let set = SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            if set.is_err() {
                let _ = CloseHandle(handle);
                return None;
            }
            Some(handle.0 as isize)
        }
    }

    fn assign_to(job: isize, child: &Child) -> bool {
        use std::os::windows::io::AsRawHandle;
        // SAFETY: both handles are valid for the duration of the call.
        unsafe {
            AssignProcessToJobObject(HANDLE(job as *mut c_void), HANDLE(child.as_raw_handle()))
                .is_ok()
        }
    }

    /// A per-child job nested in the app-wide one (Windows 8+), so a cancel can terminate the
    /// whole process tree of this child while the app-wide kill-on-close stays in force.
    /// Dropping it closes the handle, which also kills anything the child left behind.
    pub struct ChildJob(Option<isize>);

    impl ChildJob {
        /// Best effort: failure to join a job never fails the spawn.
        pub fn assign(child: &Child) -> Self {
            if let Some(app) = APP_JOB.get_or_init(create) {
                assign_to(*app, child);
            }
            let own = create();
            match own {
                Some(job) if assign_to(job, child) => Self(Some(job)),
                Some(job) => {
                    // SAFETY: handle was created above and is not used elsewhere.
                    unsafe {
                        let _ = CloseHandle(HANDLE(job as *mut c_void));
                    }
                    Self(None)
                }
                None => Self(None),
            }
        }

        pub fn terminate(&self) {
            if let Some(job) = self.0 {
                // SAFETY: the handle is owned by `self` and still open.
                unsafe {
                    let _ = TerminateJobObject(HANDLE(job as *mut c_void), 1);
                }
            }
        }
    }

    impl Drop for ChildJob {
        fn drop(&mut self) {
            if let Some(job) = self.0.take() {
                // SAFETY: the handle is owned by `self`.
                unsafe {
                    let _ = CloseHandle(HANDLE(job as *mut c_void));
                }
            }
        }
    }
}

#[cfg(not(windows))]
mod job {
    use std::process::Child;

    pub struct ChildJob;

    impl ChildJob {
        pub fn assign(_child: &Child) -> Self {
            Self
        }
        pub fn terminate(&self) {}
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
    let job = job::ChildJob::assign(&child);

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

    // Kill the whole tree (job) and the direct child, then reap it.
    let kill = |child: &mut std::process::Child| {
        job.terminate();
        let _ = child.kill();
        let _ = child.wait();
    };

    let (mut stdout, mut stderr) = (String::new(), String::new());
    let mut status: Option<ExitStatus> = None;
    let mut exited_at: Option<Instant> = None;
    loop {
        let mut timed_out = false;
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
            Err(mpsc::RecvTimeoutError::Timeout) => timed_out = true,
        }
        if cancel.is_cancelled() {
            kill(&mut child);
            return Err(Error::Cancelled);
        }
        if status.is_none() {
            match child.try_wait() {
                Ok(Some(s)) => {
                    status = Some(s);
                    exited_at = Some(Instant::now());
                }
                Ok(None) => {}
                Err(e) => {
                    kill(&mut child);
                    return Err(e.into());
                }
            }
        } else if timed_out && exited_at.is_some_and(|t| t.elapsed() > DRAIN_GRACE) {
            break;
        }
    }
    // Pipes are closed; the process may still be running (it closed its output early).
    // Keep honoring cancel while waiting for it.
    let status = loop {
        if let Some(s) = status {
            break s;
        }
        if cancel.is_cancelled() {
            kill(&mut child);
            return Err(Error::Cancelled);
        }
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
            Err(e) => {
                kill(&mut child);
                return Err(e.into());
            }
        }
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

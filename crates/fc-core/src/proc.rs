//! Child processes: no console window, killed with the app (Windows Job Object with
//! KILL_ON_JOB_CLOSE, created once and shared), cancellable, line-streamed output.
//! OWNER: Task C.

use crate::{CancelToken, Result};
use std::process::{Command, ExitStatus};

/// `Command` with `CREATE_NO_WINDOW` on Windows and stdin set to null.
pub fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let _ = program;
    todo!("Task C")
}

/// Output of a finished process (both streams fully captured, each capped at 4 MB).
pub struct Output {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

/// Spawn `cmd` (stdout/stderr piped), assign it to the shared kill-on-close job, stream every
/// stdout line to `on_stdout` and every stderr line to `on_stderr` (lossy UTF-8, trimmed of
/// `\r`), and poll `cancel` at least every 100 ms. On cancel: kill the process (and its tree via
/// the job object), wait, return `Err(Error::Cancelled)`. Never deadlocks on full pipes.
pub fn run_streaming(
    cmd: Command,
    cancel: &CancelToken,
    on_stdout: &mut dyn FnMut(&str),
    on_stderr: &mut dyn FnMut(&str),
) -> Result<Output> {
    let _ = (cmd, cancel, on_stdout, on_stderr);
    todo!("Task C")
}

/// Convenience: run to completion, capture output, no streaming callbacks.
pub fn run(cmd: Command, cancel: &CancelToken) -> Result<Output> {
    run_streaming(cmd, cancel, &mut |_| {}, &mut |_| {})
}

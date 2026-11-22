//! Optional diagnostics: when the `FC_DIAG` environment variable names a file, the app appends
//! timestamped lines to it (what the global hooks see while the wheel is open, where the wheel
//! opens, who hides the line). Off, and free, otherwise. Writing happens on its own thread, so a
//! hook callback only formats a short string and sends it.

use std::io::Write;
use std::sync::OnceLock;
use std::sync::mpsc::{Sender, channel};

static SINK: OnceLock<Option<Sender<String>>> = OnceLock::new();

fn sink() -> Option<&'static Sender<String>> {
    SINK.get_or_init(|| {
        let path = std::env::var_os("FC_DIAG")?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .ok()?;
        let (tx, rx) = channel::<String>();
        std::thread::Builder::new()
            .name("fc-diag".into())
            .spawn(move || {
                for line in rx {
                    let _ = writeln!(file, "{} {line}", fc_core::now_ms());
                    let _ = file.flush();
                }
            })
            .ok()?;
        Some(tx)
    })
    .as_ref()
}

/// Log a line (built only when the log is on).
pub fn log(line: impl FnOnce() -> String) {
    if let Some(tx) = sink() {
        let _ = tx.send(line());
    }
}

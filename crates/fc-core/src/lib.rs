//! File Chameleon core. Pure Rust, no Tauri, so it is fast to test and portable.
//!
//! Module ownership (each module is independently testable):
//! - [`formats`]    format table, conversion graph and route planner
//! - [`jobs`]       job model, output naming, the job queue
//! - [`engines`]    conversion engines (image, svg, ffmpeg, pdf, data) behind one trait
//! - [`engine_mgr`] on-demand external tools (ffmpeg, yt-dlp, deno, pdfium)
//! - [`download`]   media downloader (yt-dlp + direct HTTP)
//! - [`clip`]       clipboard classification and history store
//! - [`settings`]   persisted user settings
//! - [`presets`]    presets, groups and wheel profiles
//! - [`files`]      the clothesline files rope store
//! - [`watch`]      polling folder watcher
//! - [`proc`]       child-process helpers (no console window, kill-on-close, cancel)
//! - [`b64`]        base64 helpers (data URLs crossing IPC)

pub mod b64;
pub mod cancel;
pub mod clip;
pub mod download;
pub mod engine_mgr;
pub mod engines;
pub mod error;
pub mod files;
pub mod formats;
pub mod jobs;
pub mod presets;
pub mod proc;
pub mod settings;
pub mod watch;

pub use cancel::CancelToken;
pub use error::{Error, Result};

/// Milliseconds since the Unix epoch. Used for every timestamp crossing IPC.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

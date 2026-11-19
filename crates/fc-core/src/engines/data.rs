//! Text & data conversions. OWNER: Task B.
//! - md -> html: pulldown-cmark (tables, strikethrough, tasklists), wrapped in a minimal
//!   standalone HTML5 page with `<meta charset="utf-8">`, the input stem as `<title>`,
//!   and a small embedded readable stylesheet (system font, max-width 760px, light/dark).
//! - txt -> html: HTML-escaped inside `<pre>` in the same page shell.
//! - csv/json/yaml/toml cross conversion through `serde_json::Value`:
//!   csv -> array of objects keyed by header row (all values strings);
//!   to csv requires an array of flat objects (union of keys as header, nested values
//!   JSON-encoded); to toml requires a top-level object (arrays get wrapped as `{ items = [...] }`).
//!
//! Inputs over 64 MB -> `Error::TooLarge`.

use crate::Result;
use std::path::Path;

pub const MAX_BYTES: u64 = 64 * 1024 * 1024;

pub fn convert(input: &Path, from: &str, to: &str, output: &Path) -> Result<()> {
    let _ = (input, from, to, output);
    todo!("Task B")
}

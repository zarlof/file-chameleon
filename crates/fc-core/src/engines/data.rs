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
//!
//! Note: `serde_json` is built without `preserve_order`, so object keys come out sorted
//! (CSV column order is alphabetical after a round trip through JSON/YAML/TOML).

use crate::{Error, Result};
use serde_json::{Map, Value};
use std::path::Path;

pub const MAX_BYTES: u64 = 64 * 1024 * 1024;
/// Rows x columns allowed when a CSV is expanded into a JSON-style tree.
const MAX_CSV_CELLS: usize = 1_000_000;

const PAGE_CSS: &str = "\
:root{color-scheme:light dark}\
body{font:16px/1.6 system-ui,-apple-system,'Segoe UI',sans-serif;max-width:760px;margin:2rem auto;padding:0 1rem;color:#1d1d1f;background:#fff}\
pre,code{font-family:ui-monospace,'Cascadia Mono',Consolas,monospace;font-size:.9em}\
pre{background:#f5f5f7;padding:.8rem 1rem;border-radius:8px;overflow:auto}\
code{background:#f5f5f7;padding:.1em .3em;border-radius:4px}pre code{background:none;padding:0}\
table{border-collapse:collapse}th,td{border:1px solid #d2d2d7;padding:.35rem .7rem}\
blockquote{margin:1rem 0;padding-left:1rem;border-left:3px solid #d2d2d7;color:#6e6e73}\
img{max-width:100%}a{color:#0071e3}\
@media (prefers-color-scheme:dark){\
body{color:#f5f5f7;background:#1c1c1e}pre,code{background:#2c2c2e}th,td{border-color:#48484a}\
blockquote{border-color:#48484a;color:#98989d}a{color:#2997ff}}";

fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + s.len() / 8);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

fn page(title: &str, body: &str) -> String {
    format!(
        "<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{}</title>\n<style>{}</style>\n</head>\n<body>\n{}\n</body>\n</html>\n",
        escape_html(title),
        PAGE_CSS,
        body
    )
}

fn read_text(path: &Path) -> Result<String> {
    let meta = std::fs::metadata(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Error::NotFound(path.to_path_buf())
        } else {
            Error::Io(e)
        }
    })?;
    if meta.len() > MAX_BYTES {
        return Err(Error::TooLarge("input over 64 MB".into()));
    }
    let bytes = std::fs::read(path)?;
    let text = String::from_utf8(bytes)
        .map_err(|_| Error::Failed("file is not valid UTF-8 text".into()))?;
    Ok(match text.strip_prefix('\u{feff}') {
        Some(rest) => rest.to_owned(),
        None => text,
    })
}

fn csv_to_value(text: &str) -> Result<Value> {
    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(text.as_bytes());
    let headers: Vec<String> = rdr
        .headers()
        .map_err(|e| Error::Failed(format!("invalid CSV: {e}")))?
        .iter()
        .map(str::to_owned)
        .collect();
    for (i, h) in headers.iter().enumerate() {
        if headers[..i].contains(h) {
            return Err(Error::Failed(format!("duplicate CSV column: {h}")));
        }
    }
    let mut rows = Vec::new();
    let mut cells = 0usize;
    for rec in rdr.records() {
        let rec = rec.map_err(|e| Error::Failed(format!("invalid CSV: {e}")))?;
        cells += rec.len().max(headers.len());
        if cells > MAX_CSV_CELLS {
            return Err(Error::TooLarge("CSV has more than 1,000,000 cells".into()));
        }
        let mut obj = Map::new();
        for (i, field) in rec.iter().enumerate() {
            let key = headers
                .get(i)
                .cloned()
                .unwrap_or_else(|| format!("column{}", i + 1));
            obj.insert(key, Value::String(field.to_owned()));
        }
        rows.push(Value::Object(obj));
    }
    Ok(Value::Array(rows))
}

fn toml_to_json(v: toml::Value) -> Value {
    match v {
        toml::Value::String(s) => Value::String(s),
        toml::Value::Integer(i) => Value::from(i),
        toml::Value::Float(f) => serde_json::Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(f.to_string())),
        toml::Value::Boolean(b) => Value::Bool(b),
        toml::Value::Datetime(d) => Value::String(d.to_string()),
        toml::Value::Array(a) => Value::Array(a.into_iter().map(toml_to_json).collect()),
        toml::Value::Table(t) => {
            Value::Object(t.into_iter().map(|(k, v)| (k, toml_to_json(v))).collect())
        }
    }
}

/// JSON -> TOML value. TOML has no null: null object members are dropped, null array
/// items become empty strings.
fn json_to_toml(v: &Value) -> Option<toml::Value> {
    Some(match v {
        Value::Null => return None,
        Value::Bool(b) => toml::Value::Boolean(*b),
        Value::Number(n) => match n.as_i64() {
            Some(i) => toml::Value::Integer(i),
            None => toml::Value::Float(n.as_f64()?),
        },
        Value::String(s) => toml::Value::String(s.clone()),
        Value::Array(a) => toml::Value::Array(
            a.iter()
                .map(|x| json_to_toml(x).unwrap_or_else(|| toml::Value::String(String::new())))
                .collect(),
        ),
        Value::Object(o) => toml::Value::Table(
            o.iter()
                .filter_map(|(k, x)| json_to_toml(x).map(|t| (k.clone(), t)))
                .collect(),
        ),
    })
}

fn parse(text: &str, from: &str) -> Result<Value> {
    match from {
        "csv" => csv_to_value(text),
        "json" => {
            serde_json::from_str(text).map_err(|e| Error::Failed(format!("invalid JSON: {e}")))
        }
        "yaml" => {
            serde_yaml::from_str(text).map_err(|e| Error::Failed(format!("invalid YAML: {e}")))
        }
        "toml" => toml::from_str::<toml::Value>(text)
            .map(toml_to_json)
            .map_err(|e| Error::Failed(format!("invalid TOML: {e}"))),
        other => Err(Error::Unsupported(format!("data input format: {other}"))),
    }
}

fn cell(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Bool(_) | Value::Number(_) => v.to_string(),
        nested => nested.to_string(),
    }
}

fn to_csv(v: &Value) -> Result<String> {
    let rows = v
        .as_array()
        .ok_or_else(|| Error::Failed("CSV needs an array of objects at the top level".into()))?;
    let mut header: Vec<&str> = Vec::new();
    for row in rows {
        let obj = row
            .as_object()
            .ok_or_else(|| Error::Failed("CSV needs an array of objects".into()))?;
        for k in obj.keys() {
            if !header.contains(&k.as_str()) {
                header.push(k);
            }
        }
    }
    let mut wtr = csv::Writer::from_writer(Vec::new());
    let csv_err = |e: csv::Error| Error::Failed(format!("CSV write failed: {e}"));
    if !header.is_empty() {
        wtr.write_record(&header).map_err(csv_err)?;
        for row in rows {
            // Checked above.
            let Some(obj) = row.as_object() else { continue };
            let rec: Vec<String> = header
                .iter()
                .map(|k| obj.get(*k).map(cell).unwrap_or_default())
                .collect();
            wtr.write_record(&rec).map_err(csv_err)?;
        }
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| Error::Failed(format!("CSV write failed: {e}")))?;
    String::from_utf8(bytes).map_err(|_| Error::Failed("CSV output is not UTF-8".into()))
}

fn to_toml(v: &Value) -> Result<String> {
    let table = match v {
        Value::Object(_) => v.clone(),
        Value::Array(_) => {
            let mut m = Map::new();
            m.insert("items".into(), v.clone());
            Value::Object(m)
        }
        _ => {
            return Err(Error::Failed(
                "TOML needs an object at the top level".into(),
            ));
        }
    };
    let t = json_to_toml(&table).ok_or_else(|| Error::Failed("nothing to write".into()))?;
    toml::to_string_pretty(&t).map_err(|e| Error::Failed(format!("TOML write failed: {e}")))
}

fn serialize(v: &Value, to: &str) -> Result<String> {
    match to {
        "json" => serde_json::to_string_pretty(v)
            .map(|mut s| {
                s.push('\n');
                s
            })
            .map_err(|e| Error::Failed(format!("JSON write failed: {e}"))),
        "yaml" => {
            serde_yaml::to_string(v).map_err(|e| Error::Failed(format!("YAML write failed: {e}")))
        }
        "csv" => to_csv(v),
        "toml" => to_toml(v),
        other => Err(Error::Unsupported(format!("data output format: {other}"))),
    }
}

pub fn convert(input: &Path, from: &str, to: &str, output: &Path) -> Result<()> {
    let text = read_text(input)?;
    let out = match (from, to) {
        ("md", "html") => {
            let opts = pulldown_cmark::Options::ENABLE_TABLES
                | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
                | pulldown_cmark::Options::ENABLE_TASKLISTS;
            let parser = pulldown_cmark::Parser::new_ext(&text, opts);
            let mut body = String::new();
            pulldown_cmark::html::push_html(&mut body, parser);
            page(&stem_of(input), &body)
        }
        ("txt", "html") => page(
            &stem_of(input),
            &format!("<pre>{}</pre>", escape_html(&text)),
        ),
        ("csv" | "json" | "yaml" | "toml", "csv" | "json" | "yaml" | "toml") if from != to => {
            serialize(&parse(&text, from)?, to)?
        }
        _ => {
            return Err(Error::Unsupported(format!(
                "data conversion {from} -> {to}"
            )));
        }
    };
    std::fs::write(output, out).inspect_err(|_| {
        let _ = std::fs::remove_file(output);
    })?;
    Ok(())
}

fn stem_of(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

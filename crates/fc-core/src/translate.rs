//! Screen translation helpers: group OCR lines into paragraphs and translate them with Google
//! Translate (the free web endpoint, no key). The shell captures the screen, runs OCR and shows
//! the result; everything here is pure except [`translate`], which makes the HTTP request.
//!
//! Direction: Turkish text goes to English, everything else to Turkish ([`Target::Auto`]), or
//! always to one language.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

/// One recognized line, in screen pixels of the captured image.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrLine {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// A paragraph: lines that sit together, read as one text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    /// The lines joined with spaces (a word broken with a hyphen at a line end is joined back).
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// Average line height (the page sizes the translated text from it).
    pub line_h: f64,
    /// How many lines it was made of.
    pub lines: usize,
}

impl Block {
    fn bottom(&self) -> f64 {
        self.y + self.h
    }
    fn right(&self) -> f64 {
        self.x + self.w
    }
}

/// Group lines into paragraphs. A line joins the block above it when it starts close to the
/// block's left edge (or overlaps it mostly sideways), the gap between them is small for the
/// line height, and the heights are alike. Lines without letters or digits are dropped.
pub fn group_blocks(lines: &[OcrLine]) -> Vec<Block> {
    let mut sorted: Vec<&OcrLine> = lines
        .iter()
        .filter(|l| l.h > 0.0 && l.w > 0.0 && l.text.chars().any(char::is_alphanumeric))
        .collect();
    sorted.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
    let mut blocks: Vec<Block> = Vec::new();
    for line in sorted {
        let text = line.text.trim();
        let fits = |b: &Block| {
            let gap = line.y - b.bottom();
            let alike = (line.h / b.line_h).max(b.line_h / line.h) < 1.45;
            let left = (line.x - b.x).abs() < b.line_h * 1.5;
            let overlap = (line.x + line.w).min(b.right()) - line.x.max(b.x);
            let sideways = overlap > 0.6 * line.w.min(b.w);
            alike && gap > -0.4 * line.h && gap < 0.9 * line.h && (left || sideways)
        };
        // The nearest block above that fits (the last one made, going back a little).
        match blocks.iter_mut().rev().take(12).find(|b| fits(b)) {
            Some(b) => {
                let right = b.right().max(line.x + line.w);
                let bottom = b.bottom().max(line.y + line.h);
                b.x = b.x.min(line.x);
                b.w = right - b.x;
                b.h = bottom - b.y;
                b.line_h = (b.line_h * b.lines as f64 + line.h) / (b.lines + 1) as f64;
                b.lines += 1;
                b.text = join_line(&b.text, text);
            }
            None => blocks.push(Block {
                text: text.to_string(),
                x: line.x,
                y: line.y,
                w: line.w,
                h: line.h,
                line_h: line.h,
                lines: 1,
            }),
        }
    }
    blocks
}

/// `a` and the next line `b` as one text: "exam-" + "ple" = "example", else a space.
fn join_line(a: &str, b: &str) -> String {
    match a.strip_suffix('-') {
        Some(head)
            if head.chars().last().is_some_and(char::is_alphabetic)
                && b.chars().next().is_some_and(char::is_lowercase) =>
        {
            format!("{head}{b}")
        }
        _ => format!("{a} {b}"),
    }
}

/// Which language to translate into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// Turkish text into English, everything else into Turkish.
    Auto,
    Turkish,
    English,
}

impl Target {
    /// The setting value: "tr", "en", anything else = auto.
    pub fn from_setting(s: &str) -> Self {
        match s {
            "tr" => Target::Turkish,
            "en" => Target::English,
            _ => Target::Auto,
        }
    }
}

/// The language code to ask for, given the source language seen last (`None` = not known yet).
pub fn target_code(target: Target, source: Option<&str>) -> &'static str {
    match target {
        Target::Turkish => "tr",
        Target::English => "en",
        Target::Auto if source.is_some_and(|s| s.starts_with("tr")) => "en",
        Target::Auto => "tr",
    }
}

/// Google's answer, one entry per text sent: `[["çeviri", "en"], ...]` (translation, detected
/// source language), or plain strings when the source language was given. `None` when it is not
/// that shape.
pub fn parse_google(body: &str) -> Option<Vec<(String, String)>> {
    let v: Value = serde_json::from_str(body).ok()?;
    v.as_array()?
        .iter()
        .map(|e| match e {
            Value::String(t) => Some((t.clone(), String::new())),
            Value::Array(a) => Some((
                a.first()?.as_str()?.to_string(),
                a.get(1).and_then(Value::as_str).unwrap_or("").to_string(),
            )),
            _ => None,
        })
        .collect()
}

/// The most text sent in one request (characters).
const MAX_CHARS: usize = 4000;

/// Texts packed into requests of at most [`MAX_CHARS`] (one text per line); a longer text goes
/// alone. Returns index ranges into `texts`.
pub fn batches(texts: &[String]) -> Vec<std::ops::Range<usize>> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut len = 0;
    for (i, t) in texts.iter().enumerate() {
        let n = t.chars().count() + 1;
        if i > start && len + n > MAX_CHARS {
            out.push(start..i);
            start = i;
            len = 0;
        }
        len += n;
    }
    if start < texts.len() {
        out.push(start..texts.len());
    }
    out
}

fn agent() -> ureq::Agent {
    use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
    let tls = TlsConfig::builder()
        .provider(TlsProvider::NativeTls)
        .root_certs(RootCerts::PlatformVerifier)
        .build();
    ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_global(Some(Duration::from_secs(8)))
        .http_status_as_error(false)
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) FileChameleon")
        .build()
        .into()
}

/// One request: every text of `texts` into `tl` (the endpoint Chrome's translate extension
/// uses; the plain web one refuses scripted requests). Returns (translation, detected source)
/// per text, in order.
fn google(agent: &ureq::Agent, texts: &[&str], tl: &str) -> Result<Vec<(String, String)>> {
    let form: Vec<(&str, &str)> = texts.iter().map(|t| ("q", *t)).collect();
    let mut resp = agent
        .post("https://clients5.google.com/translate_a/t")
        .query("client", "dict-chrome-ex")
        .query("sl", "auto")
        .query("tl", tl)
        .send_form(form)
        .map_err(|e| Error::Network(e.to_string()))?;
    let status = resp.status().as_u16();
    let body = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| Error::Network(e.to_string()))?;
    if status != 200 {
        return Err(Error::Network(format!("translate: HTTP {status}")));
    }
    parse_google(&body)
        .filter(|r| r.len() == texts.len())
        .ok_or_else(|| Error::Network("translate: unexpected answer".into()))
}

/// The other language of auto mode.
fn flip(tl: &str) -> &'static str {
    if tl == "tr" { "en" } else { "tr" }
}

/// Translate `texts` into the `target` language, same order and count. `source` carries the
/// source language seen last between calls (live mode), so auto mode asks for the right
/// language at once; in auto mode a text that already is in the asked language is asked again
/// in the other one (Turkish -> English).
pub fn translate(
    texts: &[String],
    target: Target,
    source: &mut Option<String>,
) -> Result<Vec<String>> {
    let agent = agent();
    let mut out = Vec::with_capacity(texts.len());
    for range in batches(texts) {
        let chunk: Vec<&str> = texts[range].iter().map(String::as_str).collect();
        let tl = target_code(target, source.as_deref());
        let mut got = google(&agent, &chunk, tl)?;
        if let Some((_, seen)) = got.iter().rev().find(|(_, s)| !s.is_empty()) {
            *source = Some(seen.clone());
        }
        if target == Target::Auto {
            let again: Vec<usize> = (0..got.len())
                .filter(|&i| !got[i].1.is_empty() && got[i].1.starts_with(tl))
                .collect();
            if !again.is_empty() {
                let texts: Vec<&str> = again.iter().map(|&i| chunk[i]).collect();
                for (i, r) in again.iter().zip(google(&agent, &texts, flip(tl))?) {
                    got[*i] = r;
                }
            }
        }
        out.extend(got.into_iter().map(|(t, _)| t.trim().to_string()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str, x: f64, y: f64, w: f64, h: f64) -> OcrLine {
        OcrLine {
            text: text.into(),
            x,
            y,
            w,
            h,
        }
    }

    #[test]
    fn lines_group_into_paragraphs() {
        let lines = vec![
            line("Hello there, this is a long", 100.0, 100.0, 400.0, 20.0),
            line("paragraph that goes on and", 100.0, 124.0, 380.0, 20.0),
            line("exam-", 100.0, 148.0, 60.0, 20.0),
            line("ple ends here.", 100.0, 172.0, 150.0, 20.0),
            // Far below: a new paragraph.
            line("Second paragraph", 100.0, 260.0, 200.0, 20.0),
            // Same height as the first lines but far to the right: a column of its own.
            line("Sidebar", 900.0, 100.0, 90.0, 20.0),
            // A big heading right above the second paragraph: its own block.
            line("Title", 100.0, 220.0, 120.0, 34.0),
            line("···", 300.0, 400.0, 30.0, 10.0),
        ];
        let b = group_blocks(&lines);
        let texts: Vec<&str> = b.iter().map(|b| b.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "Hello there, this is a long paragraph that goes on and example ends here.",
                "Sidebar",
                "Title",
                "Second paragraph"
            ]
        );
        assert_eq!(
            (b[0].x, b[0].y, b[0].w, b[0].h, b[0].lines),
            (100.0, 100.0, 400.0, 92.0, 4)
        );
        assert_eq!(b[0].line_h, 20.0);
    }

    #[test]
    fn hyphen_joins_only_broken_words() {
        assert_eq!(join_line("exam-", "ple"), "example");
        assert_eq!(join_line("well-", "Known"), "well- Known");
        assert_eq!(join_line("a -", "b"), "a - b");
        assert_eq!(join_line("hello", "world"), "hello world");
    }

    #[test]
    fn auto_target_flips_for_turkish() {
        assert_eq!(target_code(Target::Auto, None), "tr");
        assert_eq!(target_code(Target::Auto, Some("en")), "tr");
        assert_eq!(target_code(Target::Auto, Some("tr")), "en");
        assert_eq!(target_code(Target::Turkish, Some("tr")), "tr");
        assert_eq!(target_code(Target::English, None), "en");
        assert_eq!(Target::from_setting("en"), Target::English);
        assert_eq!(Target::from_setting("auto"), Target::Auto);
    }

    #[test]
    fn google_answer_parses() {
        let body = r#"[["Selam Dünya","en"],["Ayarları açın.","en"],["Merhaba","tr"]]"#;
        let r = parse_google(body).unwrap();
        assert_eq!(r[0], ("Selam Dünya".into(), "en".into()));
        assert_eq!(r[2].1, "tr");
        assert_eq!(
            parse_google(r#"["Merhaba"]"#).unwrap(),
            vec![("Merhaba".into(), String::new())]
        );
        assert!(parse_google("not json").is_none());
        assert!(parse_google(r#"{"a":1}"#).is_none());
    }

    #[test]
    fn batches_stay_under_the_limit() {
        let texts: Vec<String> = vec![
            "a".repeat(3000),
            "b".repeat(900),
            "c".repeat(200),
            "d".repeat(5000),
            "e".into(),
        ];
        assert_eq!(batches(&texts), vec![0..2, 2..3, 3..4, 4..5]);
        assert!(batches(&[]).is_empty());
    }
}

/// Live check against Google (network): `cargo test -p fc-core --lib translate_live -- --ignored`.
#[cfg(test)]
#[test]
#[ignore]
fn translate_live() {
    let texts = vec![
        "Open the settings and choose a language.".to_string(),
        "Ayarları açın.".to_string(),
    ];
    let mut src = None;
    let out = translate(&texts, Target::Auto, &mut src).unwrap();
    eprintln!("{out:?} {src:?}");
    assert_eq!(out.len(), 2);
    assert!(out[0].to_lowercase().contains("ayar"));
    assert!(out[1].to_lowercase().contains("settings"));
}

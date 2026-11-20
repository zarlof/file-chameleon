//! Subtitle conversions, in-process: SRT <-> WebVTT and SRT/WebVTT -> plain text.
//! (ASS/SSA input goes through ffmpeg.)
//!
//! - Cues are blocks separated by blank lines with a `start --> end` line; other blocks
//!   (the `WEBVTT` header, `NOTE`/`STYLE`/`REGION` blocks) are skipped, as are cue numbers and
//!   cue ids before the timing line.
//! - Timestamps: `HH:MM:SS,mmm` (SRT) <-> `HH:MM:SS.mmm` (VTT); VTT's short `MM:SS.mmm` is read.
//!   WebVTT cue settings (`align:start ...`) are dropped.
//! - Text: `{...}` override tags (`{\an8}`) are dropped everywhere; only `<i> <b> <u>` survive a
//!   conversion (VTT voice/class tags and SRT `<font>` do not exist on the other side); WebVTT
//!   entities are decoded for SRT and text. Plain text keeps the words only, one blank line
//!   between cues.

use super::data::read_text;
use crate::{Error, Result};
use std::path::Path;

struct Cue {
    start_ms: u64,
    end_ms: u64,
    lines: Vec<String>,
}

pub fn convert(input: &Path, from: &str, to: &str, output: &Path) -> Result<()> {
    if !matches!(from, "srt" | "vtt") || !matches!(to, "srt" | "vtt" | "txt") || from == to {
        return Err(Error::Unsupported(format!(
            "subtitle conversion {from} -> {to}"
        )));
    }
    let cues = parse(&read_text(input)?, from == "vtt");
    if cues.is_empty() {
        return Err(Error::Failed("no subtitle cues found".into()));
    }
    let out = match to {
        "srt" => write_srt(&cues),
        "vtt" => write_vtt(&cues),
        _ => write_txt(&cues),
    };
    std::fs::write(output, out).inspect_err(|_| {
        let _ = std::fs::remove_file(output);
    })?;
    Ok(())
}

fn parse(text: &str, vtt: bool) -> Vec<Cue> {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut cues = Vec::new();
    let mut block: Vec<&str> = Vec::new();
    for line in text.lines().chain([""]) {
        if !line.trim().is_empty() {
            block.push(line);
            continue;
        }
        if let Some(cue) = cue_of(&block, vtt) {
            cues.push(cue);
        }
        block.clear();
    }
    cues
}

fn cue_of(block: &[&str], vtt: bool) -> Option<Cue> {
    let first = block.first()?.trim_start();
    if ["WEBVTT", "NOTE", "STYLE", "REGION"]
        .iter()
        .any(|k| first.starts_with(k))
    {
        return None;
    }
    let at = block.iter().position(|l| l.contains("-->"))?;
    let (start, rest) = block[at].split_once("-->")?;
    let end = rest.split_whitespace().next()?;
    let lines = block[at + 1..].iter().map(|l| clean_line(l, vtt)).collect();
    Some(Cue {
        start_ms: parse_time(start.trim())?,
        end_ms: parse_time(end)?,
        lines,
    })
}

/// `HH:MM:SS,mmm`, `HH:MM:SS.mmm` or `MM:SS.mmm`.
fn parse_time(s: &str) -> Option<u64> {
    let (clock, frac) = s.split_once([',', '.']).unwrap_or((s, "0"));
    let parts: Vec<u64> = clock
        .split(':')
        .map(|p| p.trim().parse().ok())
        .collect::<Option<_>>()?;
    let (h, m, sec) = match parts[..] {
        [h, m, s] => (h, m, s),
        [m, s] => (0, m, s),
        _ => return None,
    };
    let digits: String = frac.chars().take(3).collect();
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let ms = digits.parse::<u64>().ok()? * 10u64.pow(3 - digits.len() as u32);
    Some(((h * 60 + m) * 60 + sec) * 1000 + ms)
}

fn format_time(ms: u64, sep: char) -> String {
    let (h, rest) = (ms / 3_600_000, ms % 3_600_000);
    format!(
        "{h:02}:{:02}:{:02}{sep}{:03}",
        rest / 60_000,
        rest % 60_000 / 1000,
        rest % 1000
    )
}

/// Drop `{...}` override tags and every `<...>` tag except `<i> <b> <u>` (and their closers);
/// decode WebVTT entities when reading VTT.
fn clean_line(line: &str, vtt: bool) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(c) = rest.chars().next() {
        let close = match c {
            '{' => Some('}'),
            '<' => Some('>'),
            _ => None,
        };
        if let Some(close) = close
            && let Some(end) = rest.find(close)
        {
            let tag = &rest[..=end];
            if c == '<' && is_basic_tag(tag) {
                out.push_str(&tag.to_ascii_lowercase());
            }
            rest = &rest[end + 1..];
            continue;
        }
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    if vtt { decode_entities(&out) } else { out }
}

fn is_basic_tag(tag: &str) -> bool {
    let name = tag
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim_start_matches('/')
        .to_ascii_lowercase();
    matches!(name.as_str(), "i" | "b" | "u")
}

fn decode_entities(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", "\u{a0}")
        .replace("&lrm;", "\u{200e}")
        .replace("&rlm;", "\u{200f}")
        .replace("&amp;", "&")
}

/// Text without any markup (for plain text).
fn strip_tags(line: &str) -> String {
    ["<i>", "</i>", "<b>", "</b>", "<u>", "</u>"]
        .iter()
        .fold(line.to_string(), |s, t| s.replace(t, ""))
}

fn timing(c: &Cue, sep: char) -> String {
    format!(
        "{} --> {}",
        format_time(c.start_ms, sep),
        format_time(c.end_ms, sep)
    )
}

fn write_srt(cues: &[Cue]) -> String {
    cues.iter()
        .enumerate()
        .map(|(i, c)| format!("{}\n{}\n{}\n", i + 1, timing(c, ','), c.lines.join("\n")))
        .collect::<Vec<_>>()
        .join("\n")
}

fn write_vtt(cues: &[Cue]) -> String {
    let body = cues
        .iter()
        .map(|c| format!("{}\n{}\n", timing(c, '.'), c.lines.join("\n")))
        .collect::<Vec<_>>()
        .join("\n");
    format!("WEBVTT\n\n{body}")
}

fn write_txt(cues: &[Cue]) -> String {
    let blocks: Vec<String> = cues
        .iter()
        .map(|c| {
            c.lines
                .iter()
                .map(|l| strip_tags(l).trim().to_string())
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|b| !b.is_empty())
        .collect();
    if blocks.is_empty() {
        String::new()
    } else {
        format!("{}\n", blocks.join("\n\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times() {
        assert_eq!(parse_time("00:00:01,000"), Some(1000));
        assert_eq!(parse_time("01:02:03.004"), Some(3_723_004));
        assert_eq!(parse_time("02:03.5"), Some(123_500));
        assert_eq!(parse_time("x:00"), None);
        assert_eq!(parse_time("00:00:01,abc"), None);
        assert_eq!(format_time(3_723_004, ','), "01:02:03,004");
        assert_eq!(format_time(100 * 3_600_000, '.'), "100:00:00.000");
    }

    #[test]
    fn tags() {
        assert_eq!(clean_line("{\\an8}<i>Hi</i>", false), "<i>Hi</i>");
        assert_eq!(
            clean_line("<font color=\"red\">a</font> <B>b</B>", false),
            "a <b>b</b>"
        );
        assert_eq!(clean_line("<c.yellow>x</c> &amp;lt;", true), "x &lt;");
        assert_eq!(clean_line("I <3 you", false), "I <3 you");
        assert_eq!(strip_tags("<i>Hi</i>"), "Hi");
    }

    #[test]
    fn blocks_without_timing_are_skipped() {
        let cues = parse(
            "WEBVTT\n\nNOTE x --> y\n\n1\n00:00:00.000 --> 00:00:01.000\nA\n",
            true,
        );
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].lines, ["A"]);
    }
}

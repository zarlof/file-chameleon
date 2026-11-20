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
//! - Encoding: UTF-8 (with or without BOM), UTF-16 LE/BE with a BOM ("Unicode" in Notepad),
//!   and anything else that is not valid UTF-8 is read as Windows-1254 ("ANSI" on Turkish
//!   Windows, a superset of Latin-1 for Western text). Output is always UTF-8.

use super::data::MAX_BYTES;
use crate::{Error, Result};
use std::path::Path;

/// How a subtitle file's bytes are encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextEncoding {
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    Utf8,
    /// No BOM and not valid UTF-8: legacy 8-bit text, read as Windows-1254.
    Cp1254,
}

pub(crate) fn sniff(bytes: &[u8]) -> TextEncoding {
    match bytes {
        [0xEF, 0xBB, 0xBF, ..] => TextEncoding::Utf8Bom,
        [0xFF, 0xFE, ..] => TextEncoding::Utf16Le,
        [0xFE, 0xFF, ..] => TextEncoding::Utf16Be,
        _ if std::str::from_utf8(bytes).is_ok() => TextEncoding::Utf8,
        _ => TextEncoding::Cp1254,
    }
}

/// Windows-1254 bytes 0x80..=0xFF: Latin-1 except the 0x80-0x9F row and six Turkish letters.
static CP1254_HIGH: [u16; 128] = cp1254_high();

const fn cp1254_high() -> [u16; 128] {
    // 0x80-0x9F; unassigned bytes keep their C1 code point, as Windows maps them.
    const ROW_80: [u16; 32] = [
        0x20AC, 0x0081, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, // 80-87
        0x02C6, 0x2030, 0x0160, 0x2039, 0x0152, 0x008D, 0x008E, 0x008F, // 88-8F
        0x0090, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, // 90-97
        0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x009D, 0x009E, 0x0178, // 98-9F
    ];
    let mut table = [0u16; 128];
    let mut i = 0;
    while i < 128 {
        table[i] = if i < 32 { ROW_80[i] } else { 0x80 + i as u16 };
        i += 1;
    }
    table[0xD0 - 0x80] = 0x011E; // Ğ
    table[0xDD - 0x80] = 0x0130; // İ
    table[0xDE - 0x80] = 0x015E; // Ş
    table[0xF0 - 0x80] = 0x011F; // ğ
    table[0xFD - 0x80] = 0x0131; // ı
    table[0xFE - 0x80] = 0x015F; // ş
    table
}

fn decode_cp1254(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| match b {
            0..=0x7F => char::from(b),
            _ => char::from_u32(u32::from(CP1254_HIGH[usize::from(b - 0x80)]))
                .unwrap_or(char::REPLACEMENT_CHARACTER),
        })
        .collect()
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> Result<String> {
    let (pairs, rest) = bytes.as_chunks::<2>();
    if !rest.is_empty() {
        return Err(Error::Failed("broken UTF-16 text".into()));
    }
    let units: Vec<u16> = pairs
        .iter()
        .map(|&p| {
            if little_endian {
                u16::from_le_bytes(p)
            } else {
                u16::from_be_bytes(p)
            }
        })
        .collect();
    String::from_utf16(&units).map_err(|_| Error::Failed("broken UTF-16 text".into()))
}

/// Read a subtitle file in any of the encodings listed in the module docs.
fn read_subtitle(path: &Path) -> Result<String> {
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
    match sniff(&bytes) {
        TextEncoding::Utf8Bom => String::from_utf8(bytes[3..].to_vec())
            .map_err(|_| Error::Failed("file is not valid UTF-8 text".into())),
        TextEncoding::Utf16Le => decode_utf16(&bytes[2..], true),
        TextEncoding::Utf16Be => decode_utf16(&bytes[2..], false),
        TextEncoding::Utf8 => String::from_utf8(bytes)
            .map_err(|_| Error::Failed("file is not valid UTF-8 text".into())),
        TextEncoding::Cp1254 => Ok(decode_cp1254(&bytes)),
    }
}

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
    let cues = parse(&read_subtitle(input)?, from == "vtt");
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
    // Absurd values (a 20-digit hour) are a parse error, not an overflow.
    h.checked_mul(60)?
        .checked_add(m)?
        .checked_mul(60)?
        .checked_add(sec)?
        .checked_mul(1000)?
        .checked_add(ms)
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
        assert_eq!(parse_time("18446744073709551615:00:00,000"), None);
        assert_eq!(parse_time("99999999999999999:00:00,000"), None);
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
    fn encodings() {
        assert_eq!(sniff(b"\xEF\xBB\xBFa"), TextEncoding::Utf8Bom);
        assert_eq!(sniff(b"\xFF\xFEa\x00"), TextEncoding::Utf16Le);
        assert_eq!(sniff(b"\xFE\xFF\x00a"), TextEncoding::Utf16Be);
        assert_eq!(sniff("Ş".as_bytes()), TextEncoding::Utf8);
        assert_eq!(sniff(b"\xDE"), TextEncoding::Cp1254);
        assert_eq!(
            decode_cp1254(b"\xD0\xDD\xDE\xF0\xFD\xFE \xFC\xE7\xF6 \x80\x93x\x94 \x81"),
            "ĞİŞğış üçö €“x” \u{81}"
        );
        assert_eq!(decode_utf16(&[0x5E, 0x01, b'a', 0], true).unwrap(), "Şa");
        assert_eq!(decode_utf16(&[0x01, 0x5E, 0, b'a'], false).unwrap(), "Şa");
        assert!(decode_utf16(&[0x00], true).is_err());
        assert!(decode_utf16(&[0x00, 0xD8], true).is_err(), "lone surrogate");
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

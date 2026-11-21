//! Link titles: fetch a page's `og:title` / `<title>`. Private hosts and links that look like
//! they carry a secret (token, session...) are never requested.

use std::io::Read;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;

const MAX_BODY: u64 = 256 * 1024;
const MAX_TITLE_CHARS: usize = 200;
const MAX_REDIRECTS: usize = 3;
const SECRET_KEYS: [&str; 10] = [
    "token",
    "key",
    "sig",
    "signature",
    "password",
    "pass",
    "secret",
    "auth",
    "code",
    "session",
];

fn private_v4(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || (o[0] == 100 && (64..128).contains(&o[1])) // carrier-grade NAT
}

fn private_v6(ip: Ipv6Addr) -> bool {
    let first = ip.segments()[0];
    ip.is_loopback()
        || ip.is_unspecified()
        || (first & 0xfe00) == 0xfc00 // unique local
        || (first & 0xffc0) == 0xfe80 // link local
        || ip.to_ipv4_mapped().is_some_and(private_v4)
}

/// True when the URL must not be requested: not http(s), carries credentials, points at
/// `localhost` / `*.local` / a single-label or numeric host / a private, loopback or link-local
/// address, or has a query key like `token`, `key`, `sig`, `password`, `auth`, `code`, `session`.
pub fn is_private_or_secret(url: &str) -> bool {
    let url = url.trim();
    let Some((scheme, rest)) = url.split_once("://") else {
        return true;
    };
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return true;
    }
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    if authority.contains('@') {
        return true;
    }
    let host = if let Some(v6) = authority.strip_prefix('[') {
        let Some((addr, _)) = v6.split_once(']') else {
            return true;
        };
        return addr.parse::<Ipv6Addr>().map_or(true, private_v6);
    } else {
        authority.split(':').next().unwrap_or("")
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty()
        || host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || !host.contains('.')
    {
        return true;
    }
    let numeric_label = |l: &str| {
        let hex = l
            .strip_prefix("0x")
            .or_else(|| l.strip_prefix("0X"))
            .is_some_and(|h| h.chars().all(|c| c.is_ascii_hexdigit()));
        hex || (!l.is_empty() && l.chars().all(|c| c.is_ascii_digit()))
    };
    if host.split('.').all(numeric_label) {
        // A strict dotted quad, or an inet_aton-style form (hex, octal, short) that some resolvers
        // accept: those we refuse to guess about.
        let public = host
            .parse::<IpAddr>()
            .is_ok_and(|ip| matches!(ip, IpAddr::V4(v4) if !private_v4(v4)));
        if !public {
            return true;
        }
    }
    let query = tail
        .strip_prefix('?')
        .or_else(|| tail.split_once('?').map(|(_, q)| q))
        .unwrap_or("");
    let query = query.split('#').next().unwrap_or("");
    query.split('&').any(|pair| {
        let key = pair.split('=').next().unwrap_or("").to_ascii_lowercase();
        SECRET_KEYS.contains(&key.as_str())
    })
}

fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let decoded = rest.find(';').filter(|e| *e <= 10).and_then(|e| {
            let ch = match &rest[1..e] {
                "amp" => '&',
                "lt" => '<',
                "gt" => '>',
                "quot" => '"',
                "apos" => '\'',
                "nbsp" => ' ',
                n if n.starts_with("#x") || n.starts_with("#X") => {
                    char::from_u32(u32::from_str_radix(&n[2..], 16).ok()?)?
                }
                n if n.starts_with('#') => char::from_u32(n[1..].parse().ok()?)?,
                _ => return None,
            };
            Some((ch, e))
        });
        match decoded {
            Some((ch, e)) => {
                out.push(ch);
                rest = &rest[e + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn clean(raw: &str) -> Option<String> {
    let text = decode_entities(raw)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let text: String = text.chars().take(MAX_TITLE_CHARS).collect();
    let text = text.trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// `(name, value)` attribute pairs of one tag body (the part after `<meta`).
fn attributes(tag: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let b = tag.as_bytes();
    let mut i = 0;
    while i < b.len() {
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b'/') {
            i += 1;
        }
        let start = i;
        while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'=' && b[i] != b'/' {
            i += 1;
        }
        let name = tag[start..i].to_ascii_lowercase();
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut value = String::new();
        if i < b.len() && b[i] == b'=' {
            i += 1;
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
                let q = b[i];
                let from = i + 1;
                i = from;
                while i < b.len() && b[i] != q {
                    i += 1;
                }
                value = tag[from..i].to_string();
                i += 1;
            } else {
                let from = i;
                while i < b.len() && !b[i].is_ascii_whitespace() {
                    i += 1;
                }
                value = tag[from..i].to_string();
            }
        }
        if !name.is_empty() {
            out.push((name, value));
        }
    }
    out
}

/// `og:title` if present, else the `<title>` text. Entities decoded, whitespace collapsed,
/// at most 200 characters.
pub fn extract_title(html: &str) -> Option<String> {
    // ASCII lowercasing keeps byte offsets, so positions found here index `html` too.
    let lower = html.to_ascii_lowercase();
    let mut from = 0;
    while let Some(at) = lower[from..].find("<meta") {
        let body_start = from + at + 5;
        let Some(len) = lower[body_start..].find('>') else {
            break;
        };
        let attrs = attributes(&html[body_start..body_start + len]);
        let get = |n: &str| attrs.iter().find(|(k, _)| k == n).map(|(_, v)| v.as_str());
        let is_og = ["property", "name"]
            .iter()
            .any(|k| get(k).is_some_and(|v| v.eq_ignore_ascii_case("og:title")));
        if is_og && let Some(t) = get("content").and_then(clean) {
            return Some(t);
        }
        from = body_start + len;
    }
    let open = lower.find("<title")?;
    let start = open + lower[open..].find('>')? + 1;
    let end = start + lower[start..].find("</title")?;
    clean(&html[start..end])
}

fn agent() -> ureq::Agent {
    use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
    let tls = TlsConfig::builder()
        .provider(TlsProvider::NativeTls)
        .root_certs(RootCerts::PlatformVerifier)
        .build();
    ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_global(Some(Duration::from_secs(5)))
        // Redirects are followed by hand so every hop is checked against the private-host rules.
        .max_redirects(0)
        .max_redirects_will_error(false)
        .http_status_as_error(false)
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) FileChameleon")
        .build()
        .into()
}

/// Title of the page at `url`, `None` when skipped (private or secret link), unreachable,
/// not HTML, or without a title. Blocking: at most about 5 seconds and 256 KB.
pub fn fetch_link_title(url: &str) -> Option<String> {
    let agent = agent();
    let mut current = url.trim().to_string();
    for _ in 0..=MAX_REDIRECTS {
        if is_private_or_secret(&current) {
            return None;
        }
        let mut resp = agent.get(&current).call().ok()?;
        let status = resp.status().as_u16();
        let header = |n: &str| {
            resp.headers()
                .get(n)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string)
        };
        if (300..400).contains(&status) {
            // Relative redirects are not followed: only absolute http(s) targets.
            current = header("location")?;
            continue;
        }
        if !(200..300).contains(&status) {
            return None;
        }
        if header("content-type").is_some_and(|c| {
            let c = c.to_ascii_lowercase();
            !c.contains("html") && !c.contains("xml")
        }) {
            return None;
        }
        let mut bytes = Vec::new();
        resp.body_mut()
            .as_reader()
            .take(MAX_BODY)
            .read_to_end(&mut bytes)
            .ok()?;
        return extract_title(&String::from_utf8_lossy(&bytes));
    }
    None
}

/// Largest YouTube thumbnail that is kept.
const MAX_THUMB: u64 = 200 * 1024;

fn valid_youtube_id(s: &str) -> Option<String> {
    let id: String = s.chars().take(11).collect();
    (id.len() == 11
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'))
    .then_some(id)
}

/// The 11-character video id of a YouTube link (`watch?v=`, `youtu.be/`, `shorts/`).
pub fn youtube_id(url: &str) -> Option<String> {
    let rest = url.trim().split_once("://")?.1;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    let host = authority.split(':').next()?.to_ascii_lowercase();
    let host = host
        .strip_prefix("www.")
        .or(host.strip_prefix("m."))
        .unwrap_or(&host);
    let (path, query) = {
        let tail = tail.split('#').next().unwrap_or("");
        tail.split_once('?').unwrap_or((tail, ""))
    };
    match host {
        "youtu.be" => valid_youtube_id(path.strip_prefix('/')?),
        "youtube.com" => {
            if let Some(id) = path.strip_prefix("/shorts/") {
                return valid_youtube_id(id);
            }
            if path != "/watch" {
                return None;
            }
            query
                .split('&')
                .find_map(|p| p.strip_prefix("v="))
                .and_then(valid_youtube_id)
        }
        _ => None,
    }
}

/// The medium thumbnail (JPEG, at most 200 KB) of YouTube video `id`. Blocking, a few seconds.
pub fn fetch_youtube_thumb(id: &str) -> Option<Vec<u8>> {
    let id = valid_youtube_id(id).filter(|i| i.len() == 11)?;
    let mut resp = agent()
        .get(&format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg"))
        .call()
        .ok()?;
    if !(200..300).contains(&resp.status().as_u16()) {
        return None;
    }
    let mut bytes = Vec::new();
    resp.body_mut()
        .as_reader()
        .take(MAX_THUMB + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (!bytes.is_empty() && bytes.len() as u64 <= MAX_THUMB).then_some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn youtube_ids() {
        let id = Some("dQw4w9WgXcQ".to_string());
        assert_eq!(
            youtube_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=1"),
            id
        );
        assert_eq!(youtube_id("https://youtu.be/dQw4w9WgXcQ"), id);
        assert_eq!(youtube_id("https://youtube.com/shorts/dQw4w9WgXcQ"), id);
        assert_eq!(youtube_id("https://example.com/?v=dQw4w9WgXcQ"), None);
        assert_eq!(youtube_id("https://youtube.com/watch?v=short"), None);
    }

    #[test]
    fn private_and_secret_urls_are_skipped() {
        for u in [
            "http://192.168.1.4/x",
            "http://localhost:3000",
            "https://a.com/?token=1",
            "https://a.com/x?a=1&Session=abc",
            "http://127.0.0.1/",
            "http://10.0.0.2:8080/",
            "http://169.254.1.1/",
            "http://[::1]/",
            "http://printer.local/",
            "http://nas/",
            "http://2130706433/",
            "http://0x7f.0.0.1/",
            "http://0xC0A80001/",
            "http://0x7f.1/",
            "http://017700000001.1/",
            "ftp://example.com/",
            "https://user:pw@example.com/",
            "not a url",
        ] {
            assert!(is_private_or_secret(u), "{u}");
        }
        for u in [
            "https://youtube.com/watch?v=1",
            "https://example.com/",
            "http://8.8.8.8/",
            "https://example.com/page?id=7&q=monkey",
        ] {
            assert!(!is_private_or_secret(u), "{u}");
        }
    }

    #[test]
    fn title_extraction() {
        let html = "<html><head><title> Plain \n title &amp; more </title>\
            <META property='og:title' content=\"OG &quot;Title&quot; &#38; co\"></head></html>";
        assert_eq!(extract_title(html).unwrap(), "OG \"Title\" & co");
        assert_eq!(extract_title("<TITLE>A &amp; B</TITLE>").unwrap(), "A & B");
        assert_eq!(extract_title("<p>nothing</p>"), None);
        assert_eq!(extract_title("<title>   </title>"), None);
        let long = format!("<title>{}</title>", "ü".repeat(500));
        assert_eq!(extract_title(&long).unwrap().chars().count(), 200);
    }
}

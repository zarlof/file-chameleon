//! Direct HTTP downloads (`ureq`, native TLS) and the `og:image` fallback for image posts.
//! OWNER: Task F.

use super::{DlProgress, MediaInfo, classify};
use crate::jobs::naming;
use crate::{CancelToken, Error, Result};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// A generic desktop browser identity: some CDNs refuse unknown agents.
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
(KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";
/// Largest HTML page read for `og:image`.
const HTML_CAP: u64 = 2 * 1024 * 1024;
/// Progress callbacks are rate limited to this.
const PROGRESS_EVERY: Duration = Duration::from_millis(100);

fn agent() -> ureq::Agent {
    use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
    let tls = TlsConfig::builder()
        .provider(TlsProvider::NativeTls)
        .root_certs(RootCerts::PlatformVerifier)
        .build();
    ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        // A stalled transfer must fail (and stay cancellable) instead of blocking in a read.
        .timeout_per_read(Some(Duration::from_secs(30)))
        .user_agent(USER_AGENT)
        .build()
        .into()
}

fn net(e: ureq::Error) -> Error {
    Error::Network(e.to_string())
}

fn header(resp: &ureq::http::Response<ureq::Body>, name: &str) -> Option<String> {
    resp.headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
}

/// HEAD (falling back to a GET whose body is never read) for size, type and file name.
pub fn probe_direct(url: &str) -> Result<MediaInfo> {
    let agent = agent();
    let resp = match agent.head(url).call() {
        Ok(r) => r,
        Err(_) => agent.get(url).call().map_err(net)?,
    };
    let content_type = header(&resp, "content-type");
    let disposition = header(&resp, "content-disposition");
    let size = resp.body().content_length();
    let name = file_name_for(url, disposition.as_deref(), content_type.as_deref());
    let kind = match media_kind(content_type.as_deref(), &name) {
        "image" => "image",
        "audio" => "audio",
        _ => "video",
    };
    let host = classify::classify_url(url).map(|l| (l.host, l.site));
    let site = host
        .map(|(h, s)| s.map(str::to_string).unwrap_or(h))
        .unwrap_or_default();
    Ok(MediaInfo {
        url: url.to_string(),
        title: name,
        site,
        kind: kind.into(),
        size_bytes: size,
        thumbnail: (kind == "image").then(|| url.to_string()),
        direct: true,
        image_urls: if kind == "image" {
            vec![url.to_string()]
        } else {
            vec![]
        },
        ..MediaInfo::default()
    })
}

/// Stream `url` into `out_dir` (via a temp file, committed under a free name).
/// The partial file is removed on error and cancel.
pub fn download_direct(
    url: &str,
    out_dir: &Path,
    progress: &mut dyn FnMut(DlProgress),
    cancel: &CancelToken,
) -> Result<PathBuf> {
    if cancel.is_cancelled() {
        return Err(Error::Cancelled);
    }
    std::fs::create_dir_all(out_dir)?;
    let mut resp = agent().get(url).call().map_err(net)?;
    let name = file_name_for(
        url,
        header(&resp, "content-disposition").as_deref(),
        header(&resp, "content-type").as_deref(),
    );
    let total = resp.body().content_length();
    let target = {
        let p = Path::new(&name);
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        naming::output_path(p, ext, Some(out_dir), None)
    };
    let temp = naming::temp_path(&target);
    let result = stream_to(
        &mut resp.body_mut().as_reader(),
        &temp,
        total,
        progress,
        cancel,
    );
    match result {
        Ok(()) => naming::commit(&temp, &target).map_err(|e| {
            let _ = std::fs::remove_file(&temp);
            Error::from(e)
        }),
        Err(e) => {
            let _ = std::fs::remove_file(&temp);
            Err(e)
        }
    }
}

fn stream_to(
    reader: &mut dyn Read,
    temp: &Path,
    total: Option<u64>,
    progress: &mut dyn FnMut(DlProgress),
    cancel: &CancelToken,
) -> Result<()> {
    let mut out = std::fs::File::create(temp)?;
    let mut buf = vec![0u8; 64 * 1024];
    let mut done = 0u64;
    let started = Instant::now();
    let mut last = started;
    let mut last_done = 0u64;
    loop {
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let n = reader
            .read(&mut buf)
            .map_err(|e| Error::Network(e.to_string()))?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n])?;
        done += n as u64;
        let now = Instant::now();
        if now.duration_since(last) >= PROGRESS_EVERY {
            let secs = now.duration_since(last).as_secs_f64();
            let speed = ((done - last_done) as f64 / secs) as u64;
            progress(DlProgress {
                fraction: fraction(done, total),
                speed_bps: Some(speed),
                eta_secs: eta(done, total, speed),
                processing: false,
            });
            last = now;
            last_done = done;
        }
    }
    out.flush()?;
    if let Some(t) = total
        && done < t
    {
        return Err(Error::Network("connection closed early".into()));
    }
    progress(DlProgress {
        fraction: Some(1.0),
        ..DlProgress::default()
    });
    Ok(())
}

/// 0.0..=1.0, `None` when the total is unknown or zero.
pub fn fraction(done: u64, total: Option<u64>) -> Option<f32> {
    let t = total.filter(|t| *t > 0)?;
    Some((done as f64 / t as f64).clamp(0.0, 1.0) as f32)
}

/// Seconds left at `speed` bytes/s.
pub fn eta(done: u64, total: Option<u64>, speed: u64) -> Option<u64> {
    let t = total?;
    (speed > 0 && t >= done).then(|| (t - done).div_ceil(speed))
}

/// "image" | "audio" | "video" from the Content-Type, else from the file extension.
fn media_kind(content_type: Option<&str>, name: &str) -> &'static str {
    let ct = content_type.unwrap_or("").to_ascii_lowercase();
    if ct.starts_with("image/") {
        return "image";
    }
    if ct.starts_with("audio/") {
        return "audio";
    }
    if ct.starts_with("video/") {
        return "video";
    }
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("jpg" | "jpeg" | "png" | "webp" | "gif" | "bmp" | "avif" | "heic") => "image",
        Some("mp3" | "m4a" | "ogg" | "opus" | "wav" | "flac") => "audio",
        _ => "video",
    }
}

fn ext_for_content_type(ct: &str) -> Option<&'static str> {
    let ct = ct.split(';').next()?.trim().to_ascii_lowercase();
    Some(match ct.as_str() {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/bmp" => "bmp",
        "image/avif" => "avif",
        "image/heic" => "heic",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "video/quicktime" => "mov",
        "video/x-matroska" => "mkv",
        "audio/mpeg" => "mp3",
        "audio/mp4" => "m4a",
        "audio/ogg" => "ogg",
        "audio/wav" | "audio/x-wav" => "wav",
        "audio/flac" => "flac",
        _ => return None,
    })
}

/// File name for a download: `Content-Disposition` first, then the last URL path segment,
/// then "download" plus an extension guessed from the Content-Type. Always safe on Windows.
pub fn file_name_for(url: &str, disposition: Option<&str>, content_type: Option<&str>) -> String {
    let raw = disposition
        .and_then(disposition_name)
        .or_else(|| url_name(url))
        .unwrap_or_else(|| "download".to_string());
    let mut name = sanitize(&raw);
    if name.is_empty() {
        name = "download".into();
    }
    if !name.contains('.')
        && let Some(ext) = content_type.and_then(ext_for_content_type)
    {
        name.push('.');
        name.push_str(ext);
    }
    name
}

fn url_name(url: &str) -> Option<String> {
    let no_frag = url.split('#').next()?;
    let no_query = no_frag.split('?').next()?;
    let after_scheme = no_query.split_once("://").map_or(no_query, |(_, r)| r);
    let (_, path) = after_scheme.split_once('/')?;
    let last = path.rsplit('/').next()?;
    (!last.is_empty()).then(|| percent_decode(last))
}

/// `filename*=UTF-8''%E2%82%AC.txt` (preferred) or `filename="a.txt"` / `filename=a.txt`.
fn disposition_name(value: &str) -> Option<String> {
    let mut plain = None;
    for part in value.split(';').skip(1) {
        let Some((key, val)) = part.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let val = val.trim();
        if key == "filename*" {
            let enc = val.splitn(3, '\'').nth(2).unwrap_or(val);
            return Some(percent_decode(enc));
        }
        if key == "filename" {
            plain = Some(val.trim_matches('"').to_string());
        }
    }
    plain.filter(|n| !n.is_empty())
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let (Some(h), Some(l)) = (hex_val(b[i + 1]), hex_val(b[i + 2]))
        {
            out.push(h * 16 + l);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(c: u8) -> Option<u8> {
    (c as char).to_digit(16).map(|d| d as u8)
}

/// Strip path parts and characters Windows does not allow; cap the length.
fn sanitize(raw: &str) -> String {
    let base = raw.rsplit(['/', '\\']).next().unwrap_or(raw);
    let cleaned: String = base
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect();
    let trimmed = cleaned.trim().trim_end_matches(['.', ' ']);
    if trimmed.chars().count() <= 150 {
        return trimmed.to_string();
    }
    // Keep the extension when shortening.
    match trimmed.rsplit_once('.') {
        Some((stem, ext)) if ext.len() <= 8 => {
            let stem: String = stem.chars().take(140).collect();
            format!("{stem}.{ext}")
        }
        _ => trimmed.chars().take(150).collect(),
    }
}

/// Fetch the page (at most 2 MB) and return its `og:image`, as an absolute http(s) URL.
pub fn og_image(url: &str) -> Option<String> {
    let mut resp = agent().get(url).call().ok()?;
    let mut bytes = Vec::new();
    resp.body_mut()
        .as_reader()
        .take(HTML_CAP)
        .read_to_end(&mut bytes)
        .ok()?;
    extract_og_image(&String::from_utf8_lossy(&bytes), url)
}

/// `og:image` (then `twitter:image`) from the `<meta>` tags of an HTML document.
pub fn extract_og_image(html: &str, base_url: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let mut fallback = None;
    let mut from = 0;
    while let Some(i) = lower[from..].find("<meta") {
        let start = from + i;
        let end = lower[start..].find('>').map_or(lower.len(), |e| start + e);
        let tag = &html[start..end];
        from = end.max(start + 5);
        let prop = attr(tag, "property").or_else(|| attr(tag, "name"));
        let Some(prop) = prop.map(|p| p.to_ascii_lowercase()) else {
            continue;
        };
        if !matches!(
            prop.as_str(),
            "og:image" | "og:image:url" | "og:image:secure_url" | "twitter:image"
        ) {
            continue;
        }
        let Some(content) = attr(tag, "content").map(|c| unescape(c.trim())) else {
            continue;
        };
        let Some(abs) = absolute(&content, base_url) else {
            continue;
        };
        if prop.starts_with("og:") {
            return Some(abs);
        }
        fallback.get_or_insert(abs);
    }
    fallback
}

/// Value of `name="..."` / `name='...'` / `name=bare` inside one tag.
fn attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut search = 0;
    while let Some(i) = lower[search..].find(name) {
        let at = search + i;
        search = at + name.len();
        let before_ok = at == 0 || lower.as_bytes()[at - 1].is_ascii_whitespace();
        let rest = tag[at + name.len()..].trim_start();
        if !before_ok || !rest.starts_with('=') {
            continue;
        }
        let rest = rest[1..].trim_start();
        let mut chars = rest.chars();
        return match chars.next()? {
            q @ ('"' | '\'') => {
                let inner = &rest[1..];
                Some(inner[..inner.find(q).unwrap_or(inner.len())].to_string())
            }
            _ => Some(
                rest.split(|c: char| c.is_whitespace() || c == '>' || c == '/')
                    .next()
                    .unwrap_or("")
                    .to_string(),
            ),
        };
    }
    None
}

fn unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&#x2F;", "/")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn absolute(link: &str, base: &str) -> Option<String> {
    if link.starts_with("https://") || link.starts_with("http://") {
        return Some(link.to_string());
    }
    let (scheme, rest) = base.split_once("://")?;
    if let Some(r) = link.strip_prefix("//") {
        return Some(format!("{scheme}://{r}"));
    }
    if link.starts_with('/') {
        let host = rest.split(['/', '?', '#']).next()?;
        return Some(format!("{scheme}://{host}{link}"));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;

    #[test]
    fn name_from_url() {
        assert_eq!(
            file_name_for("https://x.com/a/b/photo.jpg?w=1#f", None, None),
            "photo.jpg"
        );
        assert_eq!(
            file_name_for("https://x.com/a/My%20Clip%20%C3%B6.mp4", None, None),
            "My Clip ö.mp4"
        );
        assert_eq!(
            file_name_for("https://x.com/", None, Some("image/png")),
            "download.png"
        );
        assert_eq!(
            file_name_for("https://x.com/get", None, Some("video/mp4; charset=x")),
            "get.mp4"
        );
        assert_eq!(file_name_for("https://x.com", None, None), "download");
    }

    #[test]
    fn name_from_disposition() {
        let n = |cd| file_name_for("https://x.com/zzz", Some(cd), None);
        assert_eq!(n("attachment; filename=\"report.pdf\""), "report.pdf");
        assert_eq!(n("attachment; filename=plain.mp3"), "plain.mp3");
        assert_eq!(
            n("attachment; filename=\"fallback.txt\"; filename*=UTF-8''%E2%82%AC%20rates.txt"),
            "€ rates.txt"
        );
        // path tricks and reserved characters are removed
        assert_eq!(
            n("attachment; filename=\"..\\\\..\\\\evil?.exe\""),
            "evil.exe"
        );
        assert_eq!(n("inline"), "zzz");
    }

    #[test]
    fn long_names_keep_extension() {
        let url = format!("https://x.com/{}.mp4", "a".repeat(400));
        let n = file_name_for(&url, None, None);
        assert!(n.ends_with(".mp4"));
        assert!(n.chars().count() <= 150);
    }

    #[test]
    fn kinds() {
        assert_eq!(media_kind(Some("image/webp"), "x"), "image");
        assert_eq!(media_kind(Some("audio/mpeg"), "x"), "audio");
        assert_eq!(media_kind(None, "a.FLAC"), "audio");
        assert_eq!(
            media_kind(Some("application/octet-stream"), "a.png"),
            "image"
        );
        assert_eq!(media_kind(None, "a.mp4"), "video");
    }

    #[test]
    fn og_image_extraction() {
        let html = r#"<html><head><title>x</title>
            <meta charset="utf-8">
            <META PROPERTY="og:title" content="Hello">
            <meta property="og:image" content="https://cdn.example.com/p/1.jpg?a=1&amp;b=2" />
            </head></html>"#;
        assert_eq!(
            extract_og_image(html, "https://www.instagram.com/p/abc/").as_deref(),
            Some("https://cdn.example.com/p/1.jpg?a=1&b=2")
        );
        // attribute order, single quotes, relative URL, twitter fallback
        let html = "<meta content='/img/a.png' name='twitter:image'>";
        assert_eq!(
            extract_og_image(html, "https://site.org/post/1").as_deref(),
            Some("https://site.org/img/a.png")
        );
        // og wins over an earlier twitter tag
        let html = r#"<meta name="twitter:image" content="https://a/t.jpg"><meta property="og:image" content="https://a/o.jpg">"#;
        assert_eq!(
            extract_og_image(html, "https://a/").as_deref(),
            Some("https://a/o.jpg")
        );
        assert_eq!(extract_og_image("<html></html>", "https://a/"), None);
        assert_eq!(
            extract_og_image(
                r#"<meta property="og:image" content="data:x">"#,
                "https://a/"
            ),
            None
        );
    }

    #[test]
    fn progress_helpers() {
        assert_eq!(fraction(50, Some(200)), Some(0.25));
        assert_eq!(fraction(5, None), None);
        assert_eq!(fraction(5, Some(0)), None);
        assert_eq!(fraction(300, Some(200)), Some(1.0));
        assert_eq!(eta(100, Some(1100), 100), Some(10));
        assert_eq!(eta(100, Some(1100), 0), None);
        assert_eq!(eta(100, None, 10), None);
    }

    /// Serves `body` to one connection; reads the whole request first.
    fn serve_once(body: &'static [u8], headers: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(s.try_clone().unwrap());
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
            }
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n{headers}Connection: close\r\n\r\n",
                body.len()
            );
            let _ = s.write_all(head.as_bytes());
            let _ = s.write_all(body);
        });
        format!("http://127.0.0.1:{port}/files/clip%201.bin")
    }

    #[test]
    fn downloads_from_local_server_and_names_collisions() {
        let url = serve_once(b"hello world", "Content-Type: video/mp4\r\n");
        let dir = tempfile::tempdir().unwrap();
        let mut last = DlProgress::default();
        let out =
            download_direct(&url, dir.path(), &mut |p| last = p, &CancelToken::new()).unwrap();
        assert_eq!(out.file_name().unwrap(), "clip 1.bin");
        assert_eq!(std::fs::read(&out).unwrap(), b"hello world");
        assert_eq!(last.fraction, Some(1.0));
        // no temp leftovers
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);

        let url = serve_once(b"second", "");
        let out2 = download_direct(&url, dir.path(), &mut |_| {}, &CancelToken::new()).unwrap();
        assert_eq!(out2.file_name().unwrap(), "clip 1 (1).bin");
    }

    #[test]
    fn cancelled_before_start() {
        let cancel = CancelToken::new();
        cancel.cancel();
        let dir = tempfile::tempdir().unwrap();
        let r = download_direct("http://127.0.0.1:9/x.bin", dir.path(), &mut |_| {}, &cancel);
        assert!(matches!(r, Err(Error::Cancelled)));
    }
}

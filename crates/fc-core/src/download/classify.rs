//! Offline URL classification (no network). Used by the downloader and the smart clipboard.
//! OWNER: Task F.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LinkKind {
    /// A known media site page (YouTube watch/shorts, Instagram reel/p, TikTok video, X status, ...).
    MediaPage,
    /// URL path ends in an image extension (jpg, jpeg, png, webp, gif, bmp, avif, heic).
    DirectImage,
    /// URL path ends in a video/audio extension (mp4, webm, mov, mkv, m4v, mp3, m4a, ogg, opus, wav, flac).
    DirectMedia,
    /// Any other http(s) link.
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkInfo {
    /// Normalized: trimmed, scheme added if missing for known hosts ("youtu.be/x" -> "https://youtu.be/x").
    pub url: String,
    pub host: String,
    /// Display name for known sites ("YouTube", "Instagram", "TikTok", "X", "Facebook", "Reddit",
    /// "Vimeo", "Twitch", "Pinterest", "Dailymotion", "SoundCloud", "Giphy", "Tenor", "Imgur",
    /// "Streamable", "Bilibili", "Threads", "Bluesky", "Tumblr", "Snapchat", "Kick", "Rumble").
    pub site: Option<&'static str>,
    pub kind: LinkKind,
}

/// `None` unless the trimmed text is a single http(s) URL (or a bare known-host URL) with no
/// whitespace inside. Hosts compare case-insensitively and ignore `www.`/`m.`/`mobile.` prefixes.
/// Known sites whose path is clearly not media (e.g. youtube.com/ root, instagram.com/<user>/ profile)
/// still get `site` but `kind = Other`.
pub fn classify_url(text: &str) -> Option<LinkInfo> {
    let text = text.trim();
    if text.is_empty() || text.chars().any(char::is_whitespace) {
        return None;
    }
    let lower = text.to_ascii_lowercase();
    let (has_scheme, rest) = if lower.starts_with("https://") {
        (true, &text[8..])
    } else if lower.starts_with("http://") {
        (true, &text[7..])
    } else if text.contains("://") {
        return None;
    } else {
        (false, text)
    };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    let host_port = authority.rsplit('@').next().unwrap_or(authority);
    let host = host_port.split(':').next().unwrap_or(host_port);
    let host = strip_prefixes(&host.to_ascii_lowercase());
    if host.is_empty() || !host.contains('.') || host.starts_with('.') || host.ends_with('.') {
        return None;
    }
    if !host
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
    {
        return None;
    }
    let site = site_of(&host);
    if !has_scheme && site.is_none() {
        return None;
    }

    let (path_and_query, _) = tail.split_once('#').unwrap_or((tail, ""));
    let (path, query) = path_and_query
        .split_once('?')
        .unwrap_or((path_and_query, ""));
    let segs: Vec<String> = path
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase())
        .collect();

    let kind = match extension_kind(&segs) {
        Some(k) => k,
        None if site.is_some_and(|s| is_media_page(s, &host, &segs, query)) => LinkKind::MediaPage,
        None => LinkKind::Other,
    };
    let url = if has_scheme {
        text.to_string()
    } else {
        format!("https://{text}")
    };
    Some(LinkInfo {
        url,
        host,
        site,
        kind,
    })
}

fn strip_prefixes(host: &str) -> String {
    let mut h = host;
    for p in ["www.", "m.", "mobile."] {
        if let Some(r) = h.strip_prefix(p) {
            h = r;
        }
    }
    h.to_string()
}

const IMAGE_EXT: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "bmp", "avif", "heic"];
const MEDIA_EXT: &[&str] = &[
    "mp4", "webm", "mov", "mkv", "m4v", "mp3", "m4a", "ogg", "opus", "wav", "flac",
];

fn extension_kind(segs: &[String]) -> Option<LinkKind> {
    let last = segs.last()?;
    let ext = last.rsplit_once('.')?.1;
    if IMAGE_EXT.contains(&ext) {
        Some(LinkKind::DirectImage)
    } else if MEDIA_EXT.contains(&ext) {
        Some(LinkKind::DirectMedia)
    } else {
        None
    }
}

/// (domain, display name). A host matches when it equals the domain or ends with `.domain`.
const SITES: &[(&str, &str)] = &[
    ("youtube.com", "YouTube"),
    ("youtu.be", "YouTube"),
    ("youtube-nocookie.com", "YouTube"),
    ("instagram.com", "Instagram"),
    ("instagr.am", "Instagram"),
    ("tiktok.com", "TikTok"),
    ("twitter.com", "X"),
    ("x.com", "X"),
    ("facebook.com", "Facebook"),
    ("fb.watch", "Facebook"),
    ("fb.com", "Facebook"),
    ("reddit.com", "Reddit"),
    ("redd.it", "Reddit"),
    ("vimeo.com", "Vimeo"),
    ("twitch.tv", "Twitch"),
    ("pinterest.com", "Pinterest"),
    ("pinterest.co.uk", "Pinterest"),
    ("pinterest.de", "Pinterest"),
    ("pinterest.fr", "Pinterest"),
    ("pinterest.com.tr", "Pinterest"),
    ("pin.it", "Pinterest"),
    ("dailymotion.com", "Dailymotion"),
    ("dai.ly", "Dailymotion"),
    ("soundcloud.com", "SoundCloud"),
    ("giphy.com", "Giphy"),
    ("tenor.com", "Tenor"),
    ("imgur.com", "Imgur"),
    ("streamable.com", "Streamable"),
    ("bilibili.com", "Bilibili"),
    ("b23.tv", "Bilibili"),
    ("threads.net", "Threads"),
    ("threads.com", "Threads"),
    ("bsky.app", "Bluesky"),
    ("tumblr.com", "Tumblr"),
    ("snapchat.com", "Snapchat"),
    ("kick.com", "Kick"),
    ("rumble.com", "Rumble"),
];

fn host_is(host: &str, domain: &str) -> bool {
    host == domain || host.strip_suffix(domain).is_some_and(|p| p.ends_with('.'))
}

fn site_of(host: &str) -> Option<&'static str> {
    SITES
        .iter()
        .find(|(d, _)| host_is(host, d))
        .map(|(_, name)| *name)
}

fn is_id(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

/// True when the query has `key=<non-empty>`.
fn has_query_key(query: &str, key: &str) -> bool {
    query.split('&').any(|kv| {
        kv.split_once('=')
            .is_some_and(|(k, v)| k == key && !v.is_empty())
    })
}

fn is_media_page(site: &str, host: &str, segs: &[String], query: &str) -> bool {
    let s: Vec<&str> = segs.iter().map(String::as_str).collect();
    match site {
        "YouTube" => {
            if host_is(host, "youtu.be") {
                return !s.is_empty();
            }
            match s.as_slice() {
                ["watch", ..] => has_query_key(query, "v"),
                ["shorts" | "live" | "embed" | "v" | "clip", _, ..] => true,
                _ => false,
            }
        }
        "Instagram" => matches!(
            s.as_slice(),
            ["reel" | "reels" | "p" | "tv", _, ..]
                | ["stories", _, _, ..]
                | [_, "reel" | "p" | "tv", _, ..]
        ),
        "TikTok" => {
            if host.starts_with("vm.") || host.starts_with("vt.") {
                return !s.is_empty();
            }
            matches!(
                s.as_slice(),
                [_, "video" | "photo", _, ..] | ["t" | "v", _, ..]
            )
        }
        "X" => matches!(s.as_slice(), [_, "status" | "statuses", id, ..] if is_id(id)),
        "Facebook" => {
            if host_is(host, "fb.watch") {
                return !s.is_empty();
            }
            match s.as_slice() {
                ["watch", ..] | ["video.php", ..] => has_query_key(query, "v"),
                ["reel", _, ..] | ["share", "v" | "r" | "p", _, ..] | [_, "videos", _, ..] => true,
                _ => false,
            }
        }
        "Reddit" => {
            if host_is(host, "redd.it") {
                return !s.is_empty();
            }
            s.iter()
                .position(|x| *x == "comments")
                .is_some_and(|i| s.len() > i + 1)
        }
        "Vimeo" => s.iter().any(|x| is_id(x)),
        "Twitch" => {
            if host.starts_with("clips.") {
                return !s.is_empty();
            }
            matches!(s.as_slice(), ["videos", id, ..] if is_id(id))
                || matches!(s.as_slice(), [_, "clip", _, ..])
        }
        "Pinterest" => {
            if host_is(host, "pin.it") {
                return !s.is_empty();
            }
            matches!(s.as_slice(), ["pin", _, ..])
        }
        "Dailymotion" => {
            if host_is(host, "dai.ly") {
                return !s.is_empty();
            }
            matches!(s.as_slice(), ["video", _, ..])
        }
        "SoundCloud" => {
            s.len() >= 2
                && !matches!(
                    s[0],
                    "discover" | "you" | "stream" | "search" | "upload" | "pages" | "charts"
                )
        }
        "Giphy" => matches!(s.as_slice(), ["gifs" | "clips" | "stickers", _, ..]),
        "Tenor" => matches!(s.as_slice(), ["view", _, ..]),
        "Imgur" => match s.as_slice() {
            [id] => !matches!(*id, "upload" | "gallery" | "user" | "a" | "t" | "search"),
            ["gallery", _, ..] => true,
            _ => false,
        },
        "Streamable" => matches!(s.as_slice(), [_] | ["e", _, ..]),
        "Bilibili" => {
            if host_is(host, "b23.tv") {
                return !s.is_empty();
            }
            matches!(s.as_slice(), ["video", _, ..])
        }
        "Threads" => matches!(s.as_slice(), [u, "post", _, ..] if u.starts_with('@')),
        "Bluesky" => matches!(s.as_slice(), ["profile", _, "post", _, ..]),
        "Tumblr" => matches!(s.as_slice(), ["post", _, ..]) && host != "tumblr.com",
        "Snapchat" => matches!(s.as_slice(), ["spotlight" | "t", _, ..]),
        "Kick" => {
            matches!(s.as_slice(), [_, "clips", _, ..] | ["video", _, ..])
                || has_query_key(query, "clip")
        }
        "Rumble" => s
            .first()
            .is_some_and(|x| x.starts_with('v') && x.ends_with(".html")),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(text: &str) -> LinkKind {
        classify_url(text)
            .unwrap_or_else(|| panic!("not a link: {text}"))
            .kind
    }

    fn site(text: &str) -> Option<&'static str> {
        classify_url(text).and_then(|l| l.site)
    }

    #[test]
    fn youtube() {
        for u in [
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://youtube.com/watch?feature=share&v=dQw4w9WgXcQ",
            "https://m.youtube.com/watch?v=abc",
            "https://music.youtube.com/watch?v=abc&list=x",
            "https://www.youtube.com/shorts/abc123",
            "https://youtu.be/abc123?t=4",
            "https://www.youtube.com/live/abc",
            "https://www.youtube.com/embed/abc",
        ] {
            assert_eq!(k(u), LinkKind::MediaPage, "{u}");
            assert_eq!(site(u), Some("YouTube"), "{u}");
        }
        for u in [
            "https://www.youtube.com/",
            "https://www.youtube.com/@somechannel",
            "https://www.youtube.com/watch",
            "https://www.youtube.com/feed/subscriptions",
        ] {
            assert_eq!(k(u), LinkKind::Other, "{u}");
            assert_eq!(site(u), Some("YouTube"), "{u}");
        }
    }

    #[test]
    fn bare_known_host_gets_scheme() {
        let l = classify_url("  youtu.be/x  ").unwrap();
        assert_eq!(l.url, "https://youtu.be/x");
        assert_eq!(l.host, "youtu.be");
        assert_eq!(l.kind, LinkKind::MediaPage);
        let l = classify_url("www.youtube.com/watch?v=1").unwrap();
        assert_eq!(l.url, "https://www.youtube.com/watch?v=1");
        assert_eq!(l.host, "youtube.com");
    }

    #[test]
    fn instagram() {
        for u in [
            "https://www.instagram.com/reel/Cabc123/",
            "https://instagram.com/p/Cabc123/?igsh=x",
            "https://www.instagram.com/tv/Cabc123/",
            "https://www.instagram.com/stories/someone/3141592/",
            "https://www.instagram.com/someone/reel/Cabc/",
            "https://www.instagram.com/reels/Cabc/",
        ] {
            assert_eq!(k(u), LinkKind::MediaPage, "{u}");
        }
        for u in [
            "https://www.instagram.com/someone/",
            "https://www.instagram.com/",
            "https://www.instagram.com/explore/",
        ] {
            assert_eq!(k(u), LinkKind::Other, "{u}");
            assert_eq!(site(u), Some("Instagram"));
        }
    }

    #[test]
    fn tiktok() {
        for u in [
            "https://www.tiktok.com/@user/video/7234567890123456789",
            "https://vm.tiktok.com/ZMabc123/",
            "https://vt.tiktok.com/ZSabc/",
            "https://www.tiktok.com/t/ZTabc/",
        ] {
            assert_eq!(k(u), LinkKind::MediaPage, "{u}");
            assert_eq!(site(u), Some("TikTok"));
        }
        assert_eq!(k("https://www.tiktok.com/@user"), LinkKind::Other);
        assert_eq!(k("https://www.tiktok.com/"), LinkKind::Other);
    }

    #[test]
    fn x_twitter() {
        for u in [
            "https://x.com/user/status/1234567890",
            "https://twitter.com/user/status/1234567890?s=20",
            "https://mobile.twitter.com/user/status/1",
            "https://x.com/i/status/123",
        ] {
            assert_eq!(k(u), LinkKind::MediaPage, "{u}");
            assert_eq!(site(u), Some("X"));
        }
        assert_eq!(k("https://x.com/user"), LinkKind::Other);
        assert_eq!(k("https://x.com/home"), LinkKind::Other);
    }

    #[test]
    fn facebook() {
        for u in [
            "https://www.facebook.com/watch/?v=12345",
            "https://www.facebook.com/reel/12345",
            "https://fb.watch/abcDEF/",
            "https://www.facebook.com/somepage/videos/12345/",
            "https://www.facebook.com/share/v/abc/",
        ] {
            assert_eq!(k(u), LinkKind::MediaPage, "{u}");
            assert_eq!(site(u), Some("Facebook"));
        }
        assert_eq!(k("https://www.facebook.com/somepage"), LinkKind::Other);
        assert_eq!(k("https://www.facebook.com/watch/"), LinkKind::Other);
    }

    #[test]
    fn reddit_vimeo_twitch_pinterest() {
        assert_eq!(
            k("https://www.reddit.com/r/videos/comments/abc123/some_title/"),
            LinkKind::MediaPage
        );
        assert_eq!(k("https://v.redd.it/abc123"), LinkKind::MediaPage);
        assert_eq!(k("https://www.reddit.com/r/videos/"), LinkKind::Other);
        assert_eq!(site("https://old.reddit.com/r/a"), Some("Reddit"));

        assert_eq!(k("https://vimeo.com/123456789"), LinkKind::MediaPage);
        assert_eq!(
            k("https://player.vimeo.com/video/123456"),
            LinkKind::MediaPage
        );
        assert_eq!(k("https://vimeo.com/channels/staffpicks"), LinkKind::Other);

        assert_eq!(
            k("https://clips.twitch.tv/FunnyClipSlug"),
            LinkKind::MediaPage
        );
        assert_eq!(
            k("https://www.twitch.tv/streamer/clip/Slug-abc"),
            LinkKind::MediaPage
        );
        assert_eq!(
            k("https://www.twitch.tv/videos/123456"),
            LinkKind::MediaPage
        );
        assert_eq!(k("https://www.twitch.tv/streamer"), LinkKind::Other);
        assert_eq!(site("https://clips.twitch.tv/x"), Some("Twitch"));

        assert_eq!(
            k("https://www.pinterest.com/pin/123456/"),
            LinkKind::MediaPage
        );
        assert_eq!(k("https://pin.it/abc"), LinkKind::MediaPage);
        assert_eq!(k("https://tr.pinterest.com/pin/123/"), LinkKind::MediaPage);
        assert_eq!(k("https://www.pinterest.com/someone/"), LinkKind::Other);
    }

    #[test]
    fn other_sites() {
        assert_eq!(
            k("https://www.dailymotion.com/video/x8abc"),
            LinkKind::MediaPage
        );
        assert_eq!(
            k("https://soundcloud.com/artist/track"),
            LinkKind::MediaPage
        );
        assert_eq!(k("https://soundcloud.com/artist"), LinkKind::Other);
        assert_eq!(
            k("https://giphy.com/gifs/funny-abc123"),
            LinkKind::MediaPage
        );
        assert_eq!(
            k("https://tenor.com/view/funny-gif-123"),
            LinkKind::MediaPage
        );
        assert_eq!(k("https://streamable.com/abc12"), LinkKind::MediaPage);
        assert_eq!(
            k("https://www.bilibili.com/video/BV1xx"),
            LinkKind::MediaPage
        );
        assert_eq!(
            k("https://www.threads.net/@user/post/Cabc"),
            LinkKind::MediaPage
        );
        assert_eq!(
            k("https://bsky.app/profile/a.bsky.social/post/3k"),
            LinkKind::MediaPage
        );
        assert_eq!(
            k("https://kick.com/streamer/clips/clip_abc"),
            LinkKind::MediaPage
        );
        assert_eq!(
            k("https://rumble.com/v4abcd-title.html"),
            LinkKind::MediaPage
        );
        assert_eq!(
            k("https://someone.tumblr.com/post/12345/slug"),
            LinkKind::MediaPage
        );
        assert_eq!(
            site("https://www.snapchat.com/spotlight/abc"),
            Some("Snapchat")
        );
    }

    #[test]
    fn direct_files_by_extension() {
        assert_eq!(
            k("https://example.com/a/b/photo.JPG"),
            LinkKind::DirectImage
        );
        assert_eq!(
            k("https://example.com/photo.png?w=200&x=1"),
            LinkKind::DirectImage
        );
        assert_eq!(k("http://example.com/p.webp#frag"), LinkKind::DirectImage);
        assert_eq!(k("https://example.com/p.heic"), LinkKind::DirectImage);
        assert_eq!(k("https://example.com/clip.mp4"), LinkKind::DirectMedia);
        assert_eq!(
            k("https://example.com/clip.MKV?token=abc"),
            LinkKind::DirectMedia
        );
        assert_eq!(k("https://example.com/song.flac"), LinkKind::DirectMedia);
        assert_eq!(k("https://example.com/song.opus"), LinkKind::DirectMedia);
        assert_eq!(k("https://example.com/page.html"), LinkKind::Other);
        assert_eq!(k("https://example.com/"), LinkKind::Other);
        assert_eq!(k("https://example.com"), LinkKind::Other);
        let l = classify_url("https://i.imgur.com/abc.jpg").unwrap();
        assert_eq!(l.kind, LinkKind::DirectImage);
        assert_eq!(l.site, Some("Imgur"));
        // an extension in the query does not count
        assert_eq!(k("https://example.com/watch?file=a.mp4"), LinkKind::Other);
    }

    #[test]
    fn not_links() {
        for t in [
            "",
            "   ",
            "hello world",
            "https://youtu.be/a b",
            "watch this https://youtu.be/x",
            "example.com/clip.mp4",
            "ftp://example.com/a.mp4",
            "file:///C:/a.mp4",
            "https://",
            "https:///path",
            "https://localhost/a.mp4",
            "just-text",
            "C:\\Users\\me\\a.mp4",
        ] {
            assert!(classify_url(t).is_none(), "{t:?}");
        }
    }

    #[test]
    fn host_match_is_case_insensitive_and_suffix_safe() {
        assert_eq!(site("HTTPS://WWW.YOUTUBE.COM/watch?v=A"), Some("YouTube"));
        assert_eq!(site("https://notyoutube.com/watch?v=a"), None);
        assert_eq!(site("https://evilx.com/user/status/1"), None);
        assert_eq!(k("https://notyoutube.com/watch?v=a"), LinkKind::Other);
    }
}

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
    let _ = text;
    todo!("Task F")
}

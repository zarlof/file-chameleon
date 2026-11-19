use serde::{Deserialize, Serialize};

/// `engines.manifest.json` at the repo root, embedded in the app binary.
pub const BUILTIN_MANIFEST: &str = include_str!("../../../../engines.manifest.json");

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub schema: u32,
    pub engines: Vec<ToolEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Archive {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "zip")]
    Zip,
    #[serde(rename = "tar.gz")]
    TarGz,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMap {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub min_version: Option<String>,
    pub purpose: String,
    pub license: String,
    pub homepage: String,
    pub url: String,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub sha256_url: Option<String>,
    pub size: u64,
    pub archive: Archive,
    pub files: Vec<FileMap>,
    #[serde(default)]
    pub system: Vec<String>,
}

impl Manifest {
    pub fn builtin() -> Self {
        serde_json::from_str(BUILTIN_MANIFEST).expect("engines.manifest.json is valid")
    }
    pub fn get(&self, id: &str) -> Option<&ToolEntry> {
        self.engines.iter().find(|e| e.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn builtin_manifest_parses_and_every_entry_is_verifiable() {
        let m = Manifest::builtin();
        for id in ["ffmpeg", "yt-dlp", "deno", "pdfium"] {
            let e = m.get(id).unwrap_or_else(|| panic!("missing {id}"));
            assert!(
                e.sha256.is_some() || e.sha256_url.is_some(),
                "{id} unverifiable"
            );
            assert!(!e.files.is_empty());
            assert!(e.url.starts_with("https://"));
        }
    }
}

//! Base64 (standard alphabet, padded) for bytes crossing IPC, e.g. edited images sent back
//! from the image editor as `data:image/png;base64,...` URLs.

use crate::{Error, Result};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;

pub fn encode(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

/// Decode plain base64 or a data URL (`data:<anything>;base64,<payload>`).
/// Surrounding whitespace is ignored. Bad input -> `Error::Invalid`.
pub fn decode(text: &str) -> Result<Vec<u8>> {
    let text = text.trim();
    let payload = match text.strip_prefix("data:") {
        Some(rest) => {
            let (meta, data) = rest
                .split_once(',')
                .ok_or_else(|| Error::Invalid("data URL without a comma".into()))?;
            if !meta.ends_with(";base64") {
                return Err(Error::Invalid("data URL is not base64".into()));
            }
            data
        }
        None => text,
    };
    STANDARD
        .decode(payload.trim())
        .map_err(|e| Error::Invalid(format!("base64: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_garbage_and_non_base64_data_urls() {
        assert!(matches!(decode("@@@"), Err(Error::Invalid(_))));
        assert!(matches!(
            decode("data:text/plain,hello"),
            Err(Error::Invalid(_))
        ));
        assert!(matches!(
            decode("data:image/png;base64"),
            Err(Error::Invalid(_))
        ));
        assert_eq!(decode(" aGk= \n").unwrap(), b"hi");
        assert_eq!(decode("").unwrap(), b"");
    }
}

use std::path::PathBuf;

use url::Url;

use crate::MediaError;

const MAX_REFERENCE_LEN: usize = 2 * 1024 * 1024;

/// Closed media input forms accepted from adapter configuration or actions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MediaReference {
    /// Native local path or `file:` URL.
    Local(PathBuf),
    /// HTTPS resource subject to host and address policy.
    Remote(Url),
    /// Base64 payload without the `base64://` prefix.
    InlineBase64(String),
    /// Lowercase SHA-256 cache key.
    Cache(String),
}

impl MediaReference {
    /// Parses one OneBot-compatible media reference without performing I/O.
    ///
    /// # Errors
    ///
    /// Returns an error for empty/excessive input, unsupported URL schemes, invalid file URLs,
    /// or malformed cache keys.
    pub fn parse(value: &str) -> Result<Self, MediaError> {
        if value.is_empty() || value.len() > MAX_REFERENCE_LEN || value.contains('\0') {
            return Err(MediaError::ReferenceRejected);
        }
        if let Some(encoded) = value.strip_prefix("base64://") {
            if encoded.is_empty() {
                return Err(MediaError::ReferenceRejected);
            }
            return Ok(Self::InlineBase64(encoded.to_owned()));
        }
        if let Some(key) = value.strip_prefix("cache://") {
            if valid_cache_key(key) {
                return Ok(Self::Cache(key.to_owned()));
            }
            return Err(MediaError::ReferenceRejected);
        }
        if value.starts_with("https://") {
            return Url::parse(value)
                .map(Self::Remote)
                .map_err(|_error| MediaError::ReferenceRejected);
        }
        if value.starts_with("http://") {
            return Err(MediaError::ReferenceRejected);
        }
        if value.starts_with("file:") {
            let url = Url::parse(value).map_err(|_error| MediaError::ReferenceRejected)?;
            return url
                .to_file_path()
                .map(Self::Local)
                .map_err(|()| MediaError::ReferenceRejected);
        }
        if value.contains("://") {
            return Err(MediaError::ReferenceRejected);
        }
        Ok(Self::Local(PathBuf::from(value)))
    }
}

fn valid_cache_key(value: &str) -> bool {
    value.len() == 64
        && value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

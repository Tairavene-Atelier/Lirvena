use md5::Md5;
use sha1::Sha1;
use sha2::{Digest, Sha256};

/// Origin class retained without logging the source value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaSourceKind {
    /// User-authorized local regular file.
    LocalFile,
    /// Explicit allowlisted HTTPS resource.
    RemoteHttps,
    /// Bounded inline base64 bytes.
    InlineBase64,
    /// Existing cache object addressed by digest.
    Cache,
}

/// Acquired immutable media bytes and content identity.
#[derive(Clone, Eq, PartialEq)]
pub struct MediaObject {
    bytes: Box<[u8]>,
    md5: [u8; 16],
    sha1: [u8; 20],
    digest: [u8; 32],
    source: MediaSourceKind,
}

impl MediaObject {
    pub(crate) fn new(bytes: Vec<u8>, source: MediaSourceKind) -> Self {
        let digest = Sha256::digest(&bytes).into();
        Self {
            md5: Md5::digest(&bytes).into(),
            sha1: Sha1::digest(&bytes).into(),
            bytes: bytes.into_boxed_slice(),
            digest,
            source,
        }
    }

    /// Returns immutable media bytes.
    #[must_use]
    pub const fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns the SHA-256 content identity used by cache callers.
    #[must_use]
    pub const fn sha256(&self) -> [u8; 32] {
        self.digest
    }

    /// Returns the immutable content MD5 required by QQ upload metadata.
    #[must_use]
    pub const fn md5(&self) -> [u8; 16] {
        self.md5
    }

    /// Returns the MD5 of at most the first `limit` bytes for QQ file negotiation.
    #[must_use]
    pub fn md5_prefix(&self, limit: usize) -> [u8; 16] {
        Md5::digest(&self.bytes[..self.bytes.len().min(limit)]).into()
    }

    /// Returns the immutable content SHA-1 required by QQ upload metadata.
    #[must_use]
    pub const fn sha1(&self) -> [u8; 20] {
        self.sha1
    }

    /// Returns the source class without retaining its sensitive value.
    #[must_use]
    pub const fn source_kind(&self) -> MediaSourceKind {
        self.source
    }
}

impl core::fmt::Debug for MediaObject {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("MediaObject")
            .field("byte_len", &self.bytes.len())
            .field("source", &self.source)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::{MediaObject, MediaSourceKind};

    #[test]
    fn prefix_digest_is_bounded_and_matches_complete_digest_when_short() {
        let object = MediaObject::new(vec![1, 2, 3, 4], MediaSourceKind::InlineBase64);
        assert_eq!(object.md5_prefix(4), object.md5());
        assert_ne!(object.md5_prefix(2), object.md5());
    }
}

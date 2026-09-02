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
    digest: [u8; 32],
    source: MediaSourceKind,
}

impl MediaObject {
    pub(crate) fn new(bytes: Vec<u8>, source: MediaSourceKind) -> Self {
        let digest = Sha256::digest(&bytes).into();
        Self {
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

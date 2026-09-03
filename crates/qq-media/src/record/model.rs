/// QQ-compatible audio formats accepted without lossy re-encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordFormat {
    /// MPEG audio layer III.
    Mp3,
    /// Adaptive multi-rate audio.
    Amr,
    /// Tencent-prefixed SILK v3.
    TencentSilkV3,
}

impl RecordFormat {
    pub(super) const fn extension(self) -> &'static str {
        match self {
            Self::Mp3 => ".mp3",
            Self::Amr => ".amr",
            Self::TencentSilkV3 => ".silk",
        }
    }
}

/// Validated audio identity used by QQ's rich-media upload request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordDescriptor {
    pub(super) size: u32,
    pub(super) duration_seconds: u32,
    pub(super) format: RecordFormat,
    pub(super) md5: [u8; 16],
    pub(super) sha1: [u8; 20],
}

impl RecordDescriptor {
    /// Returns the bounded byte length.
    #[must_use]
    pub const fn size(&self) -> u32 {
        self.size
    }

    /// Returns the best evidence-backed whole-second duration, or zero when unavailable.
    #[must_use]
    pub const fn duration_seconds(&self) -> u32 {
        self.duration_seconds
    }

    /// Returns the detected QQ-compatible format.
    #[must_use]
    pub const fn format(&self) -> RecordFormat {
        self.format
    }

    /// Returns the complete-file MD5 used by QQ's upload protocol.
    #[must_use]
    pub const fn md5(&self) -> [u8; 16] {
        self.md5
    }

    /// Returns the complete-file SHA-1 used by QQ's upload protocol.
    #[must_use]
    pub const fn sha1(&self) -> [u8; 20] {
        self.sha1
    }
}

/// Normalized QQ-compatible audio and its upload descriptor.
#[derive(Clone, Eq, PartialEq)]
pub struct PreparedRecord {
    pub(super) bytes: Box<[u8]>,
    pub(super) descriptor: RecordDescriptor,
}

impl PreparedRecord {
    /// Returns normalized bytes suitable for QQ upload.
    #[must_use]
    pub const fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns the descriptor computed over the normalized bytes.
    #[must_use]
    pub const fn descriptor(&self) -> &RecordDescriptor {
        &self.descriptor
    }
}

impl core::fmt::Debug for PreparedRecord {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PreparedRecord")
            .field("byte_len", &self.bytes.len())
            .field("format", &self.descriptor.format)
            .field("duration_seconds", &self.descriptor.duration_seconds)
            .finish_non_exhaustive()
    }
}

/// Encoded QQ record metadata request and route.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordMetadataRequest {
    pub(super) command: &'static str,
    pub(super) body: Vec<u8>,
}

impl RecordMetadataRequest {
    /// Returns the QQ command route.
    #[must_use]
    pub const fn command(&self) -> &'static str {
        self.command
    }

    /// Returns the exact OIDB request body.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

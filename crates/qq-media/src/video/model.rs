/// Validated MP4 identity used by QQ's rich-media upload request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VideoDescriptor {
    pub(super) size: u32,
    pub(super) md5: [u8; 16],
    pub(super) sha1: [u8; 20],
}

impl VideoDescriptor {
    /// Returns the bounded byte length.
    #[must_use]
    pub const fn size(&self) -> u32 {
        self.size
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

/// Encoded QQ video metadata request and route.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VideoMetadataRequest {
    pub(super) command: &'static str,
    pub(super) body: Vec<u8>,
}

impl VideoMetadataRequest {
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

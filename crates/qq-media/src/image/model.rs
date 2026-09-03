/// Image formats accepted by the audited Linux NT upload path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageFormat {
    /// JPEG image.
    Jpeg,
    /// PNG image.
    Png,
    /// WebP image.
    Webp,
    /// JPEG 2000 image.
    Jpeg2000,
    /// Windows bitmap image.
    Bmp,
    /// TIFF image.
    Tiff,
    /// GIF image.
    Gif,
}

impl ImageFormat {
    pub(super) const fn qq_code(self) -> u32 {
        match self {
            Self::Jpeg => 1_000,
            Self::Png => 1_001,
            Self::Webp => 1_002,
            Self::Jpeg2000 => 1_003,
            Self::Bmp => 1_005,
            Self::Tiff => 1_006,
            Self::Gif => 2_000,
        }
    }

    pub(super) const fn extension(self) -> &'static str {
        match self {
            Self::Jpeg => ".jpg",
            Self::Png => ".png",
            Self::Webp => ".webp",
            Self::Jpeg2000 => ".jp2",
            Self::Bmp => ".bmp",
            Self::Tiff => ".tiff",
            Self::Gif => ".gif",
        }
    }
}

/// Validated image identity and dimensions used by upload metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageDescriptor {
    pub(super) size: u32,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) format: ImageFormat,
    pub(super) md5: [u8; 16],
    pub(super) sha1: [u8; 20],
}

impl ImageDescriptor {
    /// Returns the bounded byte length.
    #[must_use]
    pub const fn size(&self) -> u32 {
        self.size
    }

    /// Returns the decoded pixel width.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Returns the decoded pixel height.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Returns the audited QQ image format.
    #[must_use]
    pub const fn format(&self) -> ImageFormat {
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

/// Recipient scene for image metadata negotiation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageTarget<'a> {
    /// Direct friend UID resolved from QQ's authenticated directory.
    Direct(&'a str),
    /// Numeric QQ group identifier.
    Group(u32),
}

/// Encoded QQ metadata request and route.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageMetadataRequest {
    pub(super) command: &'static str,
    pub(super) body: Vec<u8>,
}

impl ImageMetadataRequest {
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

/// Tencent-created message material and optional upload continuation.
#[derive(Clone, Eq, PartialEq)]
pub struct ImageUploadPlan {
    pub(super) message_info: Vec<u8>,
    pub(super) compatibility_message: Vec<u8>,
    pub(super) highway_extension: Option<Vec<u8>>,
    pub(super) command_id: u32,
}

impl ImageUploadPlan {
    /// Returns Tencent-created modern message information unchanged.
    #[must_use]
    pub fn message_info(&self) -> &[u8] {
        &self.message_info
    }

    /// Returns Tencent-created legacy compatibility material unchanged.
    #[must_use]
    pub fn compatibility_message(&self) -> &[u8] {
        &self.compatibility_message
    }

    /// Returns the upload extension, or `None` when fast upload already completed.
    #[must_use]
    pub fn highway_extension(&self) -> Option<&[u8]> {
        self.highway_extension.as_deref()
    }

    /// Returns the audited Highway media command identifier.
    #[must_use]
    pub const fn command_id(&self) -> u32 {
        self.command_id
    }
}

impl core::fmt::Debug for ImageUploadPlan {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ImageUploadPlan")
            .field("message_info_bytes", &self.message_info.len())
            .field("compatibility_bytes", &self.compatibility_message.len())
            .field("highway_required", &self.highway_extension.is_some())
            .field("command_id", &self.command_id)
            .finish()
    }
}

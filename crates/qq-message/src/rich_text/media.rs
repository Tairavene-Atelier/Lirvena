/// Shared bounded metadata for an incoming media object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaFile {
    uuid: Option<String>,
    name: String,
    digest: Option<String>,
    sha1: Option<String>,
    remote_reference: Option<String>,
    size: u32,
    width: u32,
    height: u32,
    duration_seconds: u32,
}

impl MediaFile {
    pub(super) fn new(spec: MediaFileSpec) -> Self {
        Self {
            uuid: spec.uuid,
            name: spec.name,
            digest: spec.digest,
            sha1: spec.sha1,
            remote_reference: spec.remote_reference,
            size: spec.size,
            width: spec.width,
            height: spec.height,
            duration_seconds: spec.duration_seconds,
        }
    }

    /// Returns the media UUID when present.
    #[must_use]
    pub fn uuid(&self) -> Option<&str> {
        self.uuid.as_deref()
    }

    /// Returns the sender-provided file name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the normalized hexadecimal primary digest when present.
    #[must_use]
    pub fn digest(&self) -> Option<&str> {
        self.digest.as_deref()
    }

    /// Returns the normalized hexadecimal SHA-1 when present.
    #[must_use]
    pub fn sha1(&self) -> Option<&str> {
        self.sha1.as_deref()
    }

    /// Returns the untrusted remote media reference without fetching it.
    #[must_use]
    pub fn remote_reference(&self) -> Option<&str> {
        self.remote_reference.as_deref()
    }

    /// Returns the declared byte size.
    #[must_use]
    pub const fn size(&self) -> u32 {
        self.size
    }

    /// Returns the declared media width.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Returns the declared media height.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Returns the declared duration in seconds.
    #[must_use]
    pub const fn duration_seconds(&self) -> u32 {
        self.duration_seconds
    }
}

pub(super) struct MediaFileSpec {
    pub uuid: Option<String>,
    pub name: String,
    pub digest: Option<String>,
    pub sha1: Option<String>,
    pub remote_reference: Option<String>,
    pub size: u32,
    pub width: u32,
    pub height: u32,
    pub duration_seconds: u32,
}

/// Whether an incoming media object belongs to a direct or group conversation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaScope {
    /// The compatibility descriptor does not prove a conversation scope.
    Unknown,
    /// Direct conversation.
    Direct,
    /// Group conversation.
    Group,
}

/// Incoming image metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageSegment {
    file: MediaFile,
    scope: MediaScope,
    summary: Option<String>,
    subtype: u32,
}

impl ImageSegment {
    pub(super) const fn new(
        file: MediaFile,
        scope: MediaScope,
        summary: Option<String>,
        subtype: u32,
    ) -> Self {
        Self {
            file,
            scope,
            summary,
            subtype,
        }
    }

    /// Returns shared file metadata.
    #[must_use]
    pub const fn file(&self) -> &MediaFile {
        &self.file
    }

    /// Returns the conversation scope.
    #[must_use]
    pub const fn scope(&self) -> MediaScope {
        self.scope
    }

    /// Returns bounded sender-provided summary text.
    #[must_use]
    pub fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    /// Returns the image subtype.
    #[must_use]
    pub const fn subtype(&self) -> u32 {
        self.subtype
    }
}

/// Incoming video metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VideoSegment {
    file: MediaFile,
    scope: MediaScope,
}

impl VideoSegment {
    pub(super) const fn new(file: MediaFile, scope: MediaScope) -> Self {
        Self { file, scope }
    }

    /// Returns shared file metadata.
    #[must_use]
    pub const fn file(&self) -> &MediaFile {
        &self.file
    }

    /// Returns the conversation scope.
    #[must_use]
    pub const fn scope(&self) -> MediaScope {
        self.scope
    }
}

/// Incoming voice metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoiceSegment {
    file: MediaFile,
    scope: MediaScope,
}

impl VoiceSegment {
    pub(super) const fn new(file: MediaFile, scope: MediaScope) -> Self {
        Self { file, scope }
    }

    /// Returns shared file metadata.
    #[must_use]
    pub const fn file(&self) -> &MediaFile {
        &self.file
    }

    /// Returns the conversation scope.
    #[must_use]
    pub const fn scope(&self) -> MediaScope {
        self.scope
    }
}

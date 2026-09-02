/// Bounded media acquisition or conversion failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaError {
    /// Configuration violated a compiled limit.
    Configuration,
    /// A media reference was malformed or outside the configured policy.
    ReferenceRejected,
    /// Local media was missing, not regular, or outside an allowed root.
    LocalFileRejected,
    /// Remote media destination or response was rejected.
    RemoteRejected,
    /// Decoded or downloaded media exceeded the configured byte bound.
    SizeLimit,
    /// Media bytes could not be acquired.
    Io,
    /// The explicit converter failed or exceeded its deadline.
    Conversion,
}

impl core::fmt::Display for MediaError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Configuration => "media configuration is invalid",
            Self::ReferenceRejected => "media reference was rejected",
            Self::LocalFileRejected => "local media file was rejected",
            Self::RemoteRejected => "remote media source was rejected",
            Self::SizeLimit => "media exceeded its byte limit",
            Self::Io => "media input or output failed",
            Self::Conversion => "media conversion failed",
        })
    }
}

impl std::error::Error for MediaError {}

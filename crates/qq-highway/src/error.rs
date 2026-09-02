use core::fmt;

/// Redacted failure raised by the bounded Highway transport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HighwayError {
    /// A caller supplied an empty, inconsistent, or unbounded value.
    InvalidInput,
    /// A protobuf or binary frame was malformed.
    MalformedFrame,
    /// QQ returned no usable upload session.
    UnusableSession,
    /// QQ rejected an upload block.
    RemoteRejected,
    /// Every authenticated QQ-provided endpoint failed transport validation.
    Transport,
}

impl fmt::Display for HighwayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidInput => "Highway input rejected",
            Self::MalformedFrame => "Highway frame rejected",
            Self::UnusableSession => "Highway session rejected",
            Self::RemoteRejected => "Highway upload rejected by QQ",
            Self::Transport => "Highway transport failed",
        })
    }
}

impl std::error::Error for HighwayError {}

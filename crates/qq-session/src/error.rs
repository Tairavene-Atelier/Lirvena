use core::fmt;

/// Rejected authenticated session operation.
#[derive(Debug)]
pub enum SessionError {
    /// The length-delimited transport failed.
    Transport(qq_transport::TransportError),
    /// An authenticated envelope failed validation.
    Envelope(qq_envelope::EnvelopeError),
    /// A response was rejected or an unconfigured Push was received.
    Protocol,
    /// Too many Push frames interrupted one request or filled the bounded queue.
    PushLimit,
}

impl fmt::Display for SessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("authenticated QQ session rejected")
    }
}

impl std::error::Error for SessionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Transport(error) => Some(error),
            Self::Envelope(error) => Some(error),
            Self::Protocol | Self::PushLimit => None,
        }
    }
}

impl SessionError {
    /// Returns whether an idle read reached its configured deadline without losing framing state.
    #[must_use]
    pub const fn is_idle_timeout(&self) -> bool {
        matches!(self, Self::Transport(qq_transport::TransportError::Timeout))
    }
}

impl From<qq_transport::TransportError> for SessionError {
    fn from(error: qq_transport::TransportError) -> Self {
        Self::Transport(error)
    }
}

impl From<qq_envelope::EnvelopeError> for SessionError {
    fn from(error: qq_envelope::EnvelopeError) -> Self {
        Self::Envelope(error)
    }
}

use core::fmt;

use ceylith_crypto::SecureSessionError;
use ceylith_protocol::CodecError;

/// Redacted Ceylith client failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientError {
    /// Public wire data was malformed, unsupported or out of bounds.
    Protocol,
    /// Secure-session authentication, ordering or lifetime failed.
    SecureSession,
    /// Response belonged to a different secure session.
    SessionBinding,
    /// The client connection is terminally closed.
    Closed,
    /// Installation identity generation failed.
    Identity,
    /// Profile digest, signature or expiry validation failed.
    ProfileAuthentication,
    /// The bounded carrier failed or timed out.
    Carrier,
    /// Installation client queue configuration was invalid.
    Configuration,
    /// The installation client worker ended unexpectedly.
    Worker,
}

impl fmt::Display for ClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Ceylith client operation failed")
    }
}

impl std::error::Error for ClientError {}

impl From<CodecError> for ClientError {
    fn from(_: CodecError) -> Self {
        Self::Protocol
    }
}

impl From<SecureSessionError> for ClientError {
    fn from(_: SecureSessionError) -> Self {
        Self::SecureSession
    }
}

impl From<prost::DecodeError> for ClientError {
    fn from(_: prost::DecodeError) -> Self {
        Self::Protocol
    }
}

impl From<prost::EncodeError> for ClientError {
    fn from(_: prost::EncodeError) -> Self {
        Self::Protocol
    }
}

impl From<std::io::Error> for ClientError {
    fn from(_: std::io::Error) -> Self {
        Self::Carrier
    }
}

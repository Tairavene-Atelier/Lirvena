use core::fmt;

use qq_wire::WireError;

use crate::QqTeaError;

/// Redacted SSO or service-envelope failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvelopeError {
    /// A bounded big-endian field was invalid.
    Wire,
    /// QQ TEA encryption failed.
    Crypto,
    /// A required ordinary envelope field was invalid.
    InvalidField,
    /// A valid but unsupported protocol or compression generation was received.
    Unsupported,
}

impl fmt::Display for EnvelopeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("QQ envelope rejected")
    }
}

impl std::error::Error for EnvelopeError {}

impl From<WireError> for EnvelopeError {
    fn from(_: WireError) -> Self {
        Self::Wire
    }
}

impl From<QqTeaError> for EnvelopeError {
    fn from(_: QqTeaError) -> Self {
        Self::Crypto
    }
}

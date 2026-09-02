use core::fmt;

use qq_envelope::QqTeaError;
use qq_wire::WireError;

/// Redacted credential-exchange packet failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialExchangeError {
    /// An ordinary profile, credential or packet field was invalid.
    InvalidField,
    /// A bounded binary field failed validation.
    Wire,
    /// QQ login encryption failed.
    Crypto,
}

impl fmt::Display for CredentialExchangeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("QQ credential exchange rejected")
    }
}

impl std::error::Error for CredentialExchangeError {}

impl From<WireError> for CredentialExchangeError {
    fn from(_: WireError) -> Self {
        Self::Wire
    }
}

impl From<QqTeaError> for CredentialExchangeError {
    fn from(_: QqTeaError) -> Self {
        Self::Crypto
    }
}

use core::fmt;

use qq_envelope::QqTeaError;
use qq_wire::WireError;

/// Redacted credential-exchange packet failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialExchangeError {
    /// An ordinary profile, credential or packet field was invalid.
    InvalidField,
    /// The outer `WtLogin` response envelope did not match the 52194 shape.
    InvalidResponseEnvelope,
    /// The response account binding did not match the confirmed QR account.
    InvalidResponseAccount,
    /// The decrypted response did not contain the expected login command.
    InvalidResponseCommand,
    /// The response TLV collection was malformed or incomplete.
    InvalidResponseTlvs,
    /// The returned public account profile was malformed.
    InvalidResponseProfile,
    /// The returned UID envelope was malformed or empty.
    InvalidResponseUid,
    /// The returned session key had an invalid width.
    InvalidResponseSessionKey,
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

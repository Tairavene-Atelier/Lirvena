use core::fmt;

use qq_envelope::QqTeaError;
use qq_wire::WireError;

/// Redacted QR packet construction failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QrPacketError {
    /// An ordinary profile, device or sequence field was invalid.
    InvalidField,
    /// A bounded big-endian field failed.
    Wire,
    /// QQ login encryption failed.
    Crypto,
}

impl fmt::Display for QrPacketError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("QQ QR packet rejected")
    }
}

impl std::error::Error for QrPacketError {}

impl From<WireError> for QrPacketError {
    fn from(_: WireError) -> Self {
        Self::Wire
    }
}

impl From<QqTeaError> for QrPacketError {
    fn from(_: QqTeaError) -> Self {
        Self::Crypto
    }
}

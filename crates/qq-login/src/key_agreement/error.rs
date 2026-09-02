use core::fmt;

/// Redacted login key-agreement failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyAgreementError {
    /// The configured peer public value was invalid for the compiled curve.
    InvalidPeer,
    /// The platform cryptographic backend failed.
    Backend,
    /// The derived shared value had an unexpected width.
    InvalidSharedValue,
}

impl fmt::Display for KeyAgreementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("QQ login key agreement failed")
    }
}

impl std::error::Error for KeyAgreementError {}

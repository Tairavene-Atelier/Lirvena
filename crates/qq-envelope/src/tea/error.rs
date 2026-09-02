use core::fmt;

/// Redacted QQ TEA codec failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QqTeaError {
    /// Plaintext or ciphertext exceeded the compiled packet bound.
    LengthLimit,
    /// Checked length arithmetic overflowed.
    LengthOverflow,
    /// Operating-system entropy was unavailable.
    Entropy,
    /// Deterministic test padding had the wrong length.
    PaddingLength,
    /// Ciphertext width, padding header or trailing sentinel was invalid.
    InvalidCiphertext,
}

impl fmt::Display for QqTeaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("QQ TEA value rejected")
    }
}

impl std::error::Error for QqTeaError {}

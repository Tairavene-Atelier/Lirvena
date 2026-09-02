use core::fmt;

/// Invalid QQ profile value without embedded profile material.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileValueError {
    /// A required numeric field was zero.
    ZeroNumber,
    /// A profile text field was empty, too long or outside its allowed alphabet.
    InvalidText,
}

impl fmt::Display for ProfileValueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("QQ profile value rejected")
    }
}

impl std::error::Error for ProfileValueError {}

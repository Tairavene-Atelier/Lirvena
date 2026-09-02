use core::fmt;

/// Length-prefix encoding used by a bounded QQ field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LengthPrefix {
    /// A `u8` containing only the payload length.
    U8Payload,
    /// A `u8` containing prefix and payload length.
    U8Inclusive,
    /// A big-endian `u16` containing only the payload length.
    U16Payload,
    /// A big-endian `u16` containing prefix and payload length.
    U16Inclusive,
    /// A big-endian `u32` containing only the payload length.
    U32Payload,
    /// A big-endian `u32` containing prefix and payload length.
    U32Inclusive,
}

impl LengthPrefix {
    pub(crate) const fn width(self) -> usize {
        match self {
            Self::U8Payload | Self::U8Inclusive => 1,
            Self::U16Payload | Self::U16Inclusive => 2,
            Self::U32Payload | Self::U32Inclusive => 4,
        }
    }

    pub(crate) const fn includes_prefix(self) -> bool {
        matches!(
            self,
            Self::U8Inclusive | Self::U16Inclusive | Self::U32Inclusive
        )
    }
}

/// Redacted bounded-wire failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WireError {
    /// Input ended before the requested field boundary.
    Truncated {
        /// Required byte length from the current cursor.
        needed: usize,
        /// Available byte length from the current cursor.
        available: usize,
    },
    /// Bytes remained after the declared packet was decoded.
    TrailingBytes {
        /// Number of unconsumed bytes.
        remaining: usize,
    },
    /// Checked length arithmetic overflowed.
    LengthOverflow,
    /// A requested or declared value exceeded the configured bound.
    LengthLimitExceeded {
        /// Maximum accepted length.
        limit: usize,
        /// Rejected length.
        actual: usize,
    },
    /// An inclusive length was smaller than its own prefix.
    InvalidInclusiveLength,
}

impl fmt::Display for WireError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("QQ wire value rejected")
    }
}

impl std::error::Error for WireError {}

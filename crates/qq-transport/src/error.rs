use core::fmt;

/// Redacted QQ transport failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportError {
    /// Timeout or frame bounds were invalid.
    InvalidConfiguration,
    /// A frame length was truncated, contradictory or outside its bound.
    InvalidFrame,
    /// Connection, DNS, read or write failed.
    Io,
    /// A configured operation deadline elapsed.
    Timeout,
}

impl fmt::Display for TransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("QQ transport operation failed")
    }
}

impl std::error::Error for TransportError {}

impl From<std::io::Error> for TransportError {
    fn from(_: std::io::Error) -> Self {
        Self::Io
    }
}

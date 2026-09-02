use core::fmt;

/// Rejected authenticated message Push.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MessageDecodeError;

impl fmt::Display for MessageDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("QQ message Push rejected")
    }
}

impl std::error::Error for MessageDecodeError {}

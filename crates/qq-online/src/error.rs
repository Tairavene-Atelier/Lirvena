use core::fmt;

/// Rejected online packet input or response.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OnlinePacketError;

impl fmt::Display for OnlinePacketError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("QQ online packet rejected")
    }
}

impl std::error::Error for OnlinePacketError {}

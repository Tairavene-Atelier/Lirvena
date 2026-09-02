use core::time::Duration;

use crate::TransportError;

/// Compile-time allowlisted QQ transport endpoint.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum QqEndpoint {
    /// Primary SSO endpoint.
    #[default]
    Primary,
    /// Alternate SSO port used by the same upstream service.
    Alternate,
}

impl QqEndpoint {
    pub(crate) const fn address(self) -> &'static str {
        match self {
            Self::Primary => "msfwifi.3g.qq.com:8080",
            Self::Alternate => "msfwifi.3g.qq.com:14000",
        }
    }
}

/// Bounded timeout and frame configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransportConfig {
    pub(crate) connect_timeout: Duration,
    pub(crate) io_timeout: Duration,
    pub(crate) maximum_frame_len: usize,
}

impl TransportConfig {
    /// Hard maximum frame accepted by this transport implementation.
    pub const HARD_MAX_FRAME_LEN: usize = 2 * 1024 * 1024;

    /// Creates a transport configuration.
    ///
    /// # Errors
    ///
    /// Returns an error for zero timeouts or an invalid frame bound.
    pub const fn new(
        connect_timeout: Duration,
        io_timeout: Duration,
        maximum_frame_len: usize,
    ) -> Result<Self, TransportError> {
        if connect_timeout.is_zero()
            || io_timeout.is_zero()
            || maximum_frame_len < 4
            || maximum_frame_len > Self::HARD_MAX_FRAME_LEN
        {
            return Err(TransportError::InvalidConfiguration);
        }
        Ok(Self {
            connect_timeout,
            io_timeout,
            maximum_frame_len,
        })
    }

    /// Returns the maximum accepted complete frame length.
    #[must_use]
    pub const fn maximum_frame_len(self) -> usize {
        self.maximum_frame_len
    }
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            io_timeout: Duration::from_secs(15),
            maximum_frame_len: Self::HARD_MAX_FRAME_LEN,
        }
    }
}

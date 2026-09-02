use core::fmt;

use qq_domain::TransitionError;

/// Rejected QR artifact without URL or image data in the error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QrArtifactError {
    /// QR URL was absent, too long or not HTTPS.
    InvalidUrl,
    /// PNG bytes were absent, oversized or lacked the PNG signature.
    InvalidPng,
    /// The declared lifetime was zero, excessive or overflowed.
    InvalidLifetime,
    /// The QR URL could not be encoded into a terminal matrix.
    MatrixEncoding,
}

impl fmt::Display for QrArtifactError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("QR artifact rejected")
    }
}

impl std::error::Error for QrArtifactError {}

/// Unknown closed QQ QR polling state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnknownQrPollState {
    value: u8,
}

impl UnknownQrPollState {
    pub(crate) const fn new(value: u8) -> Self {
        Self { value }
    }

    /// Returns the rejected public protocol value.
    #[must_use]
    pub const fn value(self) -> u8 {
        self.value
    }
}

impl fmt::Display for UnknownQrPollState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("unknown QR polling state")
    }
}

impl std::error::Error for UnknownQrPollState {}

/// Checked QR flow failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QrFlowError {
    /// The account login machine rejected the requested transition.
    Transition(TransitionError),
}

impl fmt::Display for QrFlowError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("QR login flow rejected")
    }
}

impl std::error::Error for QrFlowError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Transition(error) => Some(error),
        }
    }
}

impl From<TransitionError> for QrFlowError {
    fn from(error: TransitionError) -> Self {
        Self::Transition(error)
    }
}

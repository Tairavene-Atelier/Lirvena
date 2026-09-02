use core::fmt;

/// Closed secure-session failure without key or transcript details.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecureSessionError {
    /// Operating-system cryptographic randomness was unavailable.
    EntropyUnavailable,
    /// A supplied X25519 public key was non-contributory.
    InvalidPublicKey,
    /// The fixed handshake could not authenticate or complete.
    HandshakeFailed,
    /// A handshake or transport message exceeded the fixed suite bound.
    MessageTooLarge,
    /// The explicit counter was repeated, skipped or reordered.
    CounterMismatch,
    /// Per-direction message lifetime was exhausted.
    SessionExpired,
    /// Ciphertext authentication failed.
    AuthenticationFailed,
    /// Session state was already terminally closed.
    SessionClosed,
    /// A configured message cap was outside the compiled range.
    InvalidLimit,
}

impl fmt::Display for SecureSessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("secure session operation failed")
    }
}

impl std::error::Error for SecureSessionError {}

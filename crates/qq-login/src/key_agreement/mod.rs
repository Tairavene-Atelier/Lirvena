mod error;
#[cfg(target_os = "linux")]
mod linux;

use qq_envelope::QqTeaKey;

pub use error::KeyAgreementError;
#[cfg(target_os = "linux")]
pub use linux::LinuxKeyAgreement;

/// Narrow login key-agreement result consumed by the QQ envelope layer.
pub trait QqKeyAgreement {
    /// Returns the compressed ephemeral public key for the login header.
    fn public_key(&self) -> &[u8];

    /// Returns the derived QQ TEA key.
    fn tea_key(&self) -> &QqTeaKey;

    /// Derives a response TEA key from a server-provided ephemeral point.
    ///
    /// # Errors
    ///
    /// Returns an error when the point is invalid or the platform backend fails.
    fn derive_response_key(&self, _peer_public: &[u8]) -> Result<QqTeaKey, KeyAgreementError> {
        Err(KeyAgreementError::InvalidPeer)
    }
}

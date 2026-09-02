use core::fmt;

use noise_protocol::U8Array;
use noise_rust_crypto::sensitive::Sensitive;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroizing;

use crate::SecureSessionError;

/// Zeroized X25519 static or ephemeral private key material.
pub struct NoisePrivateKey(Zeroizing<[u8; 32]>);

impl NoisePrivateKey {
    /// Imports an exact 32-byte private key from protected provisioning.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(Zeroizing::new(bytes))
    }

    /// Generates a fresh private key from the operating-system CSPRNG.
    ///
    /// # Errors
    ///
    /// Returns an error when operating-system entropy is unavailable.
    pub fn generate() -> Result<Self, SecureSessionError> {
        let mut bytes = [0_u8; 32];
        getrandom::fill(&mut bytes).map_err(|_| SecureSessionError::EntropyUnavailable)?;
        Ok(Self::from_bytes(bytes))
    }

    /// Derives the matching X25519 public key.
    #[must_use]
    pub fn public_key(&self) -> NoisePublicKey {
        let secret = StaticSecret::from(*self.0);
        NoisePublicKey(*PublicKey::from(&secret).as_bytes())
    }

    pub(crate) fn sensitive(&self) -> Sensitive<[u8; 32]> {
        Sensitive::from_slice(self.0.as_slice())
    }
}

impl fmt::Debug for NoisePrivateKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("NoisePrivateKey([REDACTED])")
    }
}

/// Exact X25519 public key used by the fixed suite.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct NoisePublicKey([u8; 32]);

impl NoisePublicKey {
    /// Imports a public key and rejects the all-zero non-contributory value.
    ///
    /// # Errors
    ///
    /// Returns an error for the all-zero non-contributory public key.
    pub fn try_from_bytes(bytes: [u8; 32]) -> Result<Self, SecureSessionError> {
        if bytes == [0; 32] {
            Err(SecureSessionError::InvalidPublicKey)
        } else {
            Ok(Self(bytes))
        }
    }

    /// Borrows the exact public representation.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for NoisePublicKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("NoisePublicKey(<opaque>)")
    }
}

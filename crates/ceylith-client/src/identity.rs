use core::fmt;

use ceylith_crypto::{NoisePrivateKey, NoisePublicKey};
use ceylith_protocol::{
    InstallationId, MAX_ACCESS_TOKEN_LEN, proto, session_hello_signing_transcript,
    validate_session_hello,
};
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use zeroize::Zeroizing;

use crate::{ClientError, RuntimeDescriptor};

/// Zeroized access token that never exposes its contents through formatting.
pub struct AccessToken(Zeroizing<Vec<u8>>);

impl AccessToken {
    /// Validates a non-empty token under the public wire bound.
    ///
    /// # Errors
    ///
    /// Returns an error when the token is empty or exceeds the public wire bound.
    pub fn new(bytes: Vec<u8>) -> Result<Self, ClientError> {
        if bytes.is_empty() || bytes.len() > MAX_ACCESS_TOKEN_LEN {
            return Err(ClientError::Identity);
        }
        Ok(Self(Zeroizing::new(bytes)))
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for AccessToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AccessToken([REDACTED])")
    }
}

/// Installation signing and Noise identity.
pub struct InstallationIdentity {
    installation_id: InstallationId,
    signing_key: SigningKey,
    noise_key: NoisePrivateKey,
}

impl InstallationIdentity {
    /// Generates a fresh installation identity using operating-system randomness.
    ///
    /// # Errors
    ///
    /// Returns an error when operating-system entropy is unavailable.
    pub fn generate() -> Result<Self, ClientError> {
        let mut installation_id = [0_u8; InstallationId::LENGTH];
        let mut signing_seed = Zeroizing::new([0_u8; 32]);
        getrandom::fill(&mut installation_id).map_err(|_| ClientError::Identity)?;
        getrandom::fill(signing_seed.as_mut()).map_err(|_| ClientError::Identity)?;
        let noise_key = NoisePrivateKey::generate().map_err(|_| ClientError::Identity)?;
        Ok(Self {
            installation_id: InstallationId::from_bytes(installation_id),
            signing_key: SigningKey::from_bytes(&signing_seed),
            noise_key,
        })
    }

    /// Imports exact identity material from protected local provisioning.
    #[must_use]
    pub fn from_parts(
        installation_id: InstallationId,
        signing_seed: [u8; 32],
        noise_seed: [u8; 32],
    ) -> Self {
        Self {
            installation_id,
            signing_key: SigningKey::from_bytes(&signing_seed),
            noise_key: NoisePrivateKey::from_bytes(noise_seed),
        }
    }

    /// Installation identifier.
    #[must_use]
    pub const fn installation_id(&self) -> InstallationId {
        self.installation_id
    }

    /// Installation signing public key.
    #[must_use]
    pub fn signing_public_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Installation Noise public key.
    #[must_use]
    pub fn noise_public_key(&self) -> NoisePublicKey {
        self.noise_key.public_key()
    }

    pub(crate) fn noise_private_key(&self) -> &NoisePrivateKey {
        &self.noise_key
    }

    pub(crate) fn signed_hello(
        &self,
        token: Option<&AccessToken>,
        runtime: &RuntimeDescriptor,
        requested_feature_bits: u64,
    ) -> Result<proto::SessionHello, ClientError> {
        let mut hello = proto::SessionHello {
            installation_id: self.installation_id.as_bytes().to_vec(),
            installation_sign_public_key: self.signing_public_key().to_bytes().to_vec(),
            installation_noise_public_key: self.noise_public_key().as_bytes().to_vec(),
            transcript_signature: Vec::new(),
            access_token: token.map_or_else(Vec::new, |value| value.as_bytes().to_vec()),
            runtime: Some(runtime.as_wire().clone()),
            requested_feature_bits,
        };
        let transcript = Zeroizing::new(session_hello_signing_transcript(&hello)?);
        hello.transcript_signature = self.signing_key.sign(&transcript).to_bytes().to_vec();
        validate_session_hello(&hello)?;
        Ok(hello)
    }
}

impl fmt::Debug for InstallationIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InstallationIdentity")
            .field("installation_id", &self.installation_id)
            .field("signing_key", &"[REDACTED]")
            .field("noise_key", &self.noise_key)
            .finish()
    }
}

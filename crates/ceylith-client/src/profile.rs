use core::fmt;

use ceylith_protocol::{
    ProfileOutcome, decode_profile_outcome, profile_decision_signing_transcript, proto,
};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};

use crate::ClientError;

/// Verifies cacheable ready-profile integrity and authenticity.
pub struct ProfileVerifier {
    verifying_key: VerifyingKey,
}

impl ProfileVerifier {
    /// Imports the Ceylith profile-signing public key.
    ///
    /// # Errors
    ///
    /// Returns an error when the public key encoding is invalid.
    pub fn from_bytes(bytes: &[u8; 32]) -> Result<Self, ClientError> {
        let verifying_key =
            VerifyingKey::from_bytes(bytes).map_err(|_| ClientError::ProfileAuthentication)?;
        Ok(Self { verifying_key })
    }

    /// Verifies a decision before its profile material is cached or executed.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid structure, expiry, digest, or signature.
    pub fn verify(
        &self,
        decision: &proto::ProfileDecision,
        now_ms: u64,
    ) -> Result<ProfileOutcome, ClientError> {
        let outcome =
            decode_profile_outcome(decision).map_err(|_| ClientError::ProfileAuthentication)?;
        if let ProfileOutcome::Ready(ready) = &outcome {
            if now_ms >= ready.expires_at_ms() {
                return Err(ClientError::ProfileAuthentication);
            }
            let digest = Sha256::digest(ready.manifest());
            if digest.as_slice() != ready.manifest_digest().as_bytes() {
                return Err(ClientError::ProfileAuthentication);
            }
            let transcript = profile_decision_signing_transcript(decision)?;
            let signature = Signature::from_bytes(ready.manifest_signature());
            self.verifying_key
                .verify(&transcript, &signature)
                .map_err(|_| ClientError::ProfileAuthentication)?;
        }
        Ok(outcome)
    }
}

impl fmt::Debug for ProfileVerifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProfileVerifier(<provisioned>)")
    }
}

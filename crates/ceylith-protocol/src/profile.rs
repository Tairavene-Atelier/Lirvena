use crate::{Digest32, MAX_MANIFEST_LEN, ProfileError, ProfileId, proto};

/// Structurally validated profile negotiation outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProfileOutcome {
    /// The requested profile can run under the client's compiled runtime ABI.
    Ready(ReadyProfile),
    /// The profile exists but requires a newer compiled runtime ABI.
    ClientUpgradeRequired {
        /// Requested profile identifier.
        profile_id: ProfileId,
        /// Minimum required runtime ABI.
        required_runtime_abi: u32,
        /// Server policy generation.
        policy_epoch: u64,
    },
    /// No currently usable profile is available.
    Unavailable {
        /// Requested profile identifier.
        profile_id: ProfileId,
        /// Server policy generation.
        policy_epoch: u64,
    },
}

/// Structurally validated ready profile material.
#[derive(Clone, Eq, PartialEq)]
pub struct ReadyProfile {
    profile_id: ProfileId,
    manifest: Box<[u8]>,
    manifest_digest: Digest32,
    manifest_signature: [u8; 64],
    expires_at_ms: u64,
    policy_epoch: u64,
}

impl ReadyProfile {
    /// Profile identifier.
    #[must_use]
    pub const fn profile_id(&self) -> ProfileId {
        self.profile_id
    }

    /// Signed public manifest bytes.
    #[must_use]
    pub fn manifest(&self) -> &[u8] {
        &self.manifest
    }

    /// Declared manifest digest.
    #[must_use]
    pub const fn manifest_digest(&self) -> Digest32 {
        self.manifest_digest
    }

    /// Detached manifest signature.
    #[must_use]
    pub const fn manifest_signature(&self) -> &[u8; 64] {
        &self.manifest_signature
    }

    /// Exclusive manifest expiry.
    #[must_use]
    pub const fn expires_at_ms(&self) -> u64 {
        self.expires_at_ms
    }

    /// Server policy generation.
    #[must_use]
    pub const fn policy_epoch(&self) -> u64 {
        self.policy_epoch
    }
}

impl core::fmt::Debug for ReadyProfile {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ReadyProfile")
            .field("profile_id", &self.profile_id)
            .field("manifest_len", &self.manifest.len())
            .field("manifest_digest", &self.manifest_digest)
            .field("manifest_signature", &"<opaque>")
            .field("expires_at_ms", &self.expires_at_ms)
            .field("policy_epoch", &self.policy_epoch)
            .finish()
    }
}

/// Decodes a protobuf decision into a closed structural outcome.
///
/// # Errors
///
/// Returns an error when status-specific fields are incomplete or contradictory.
pub fn decode_profile_outcome(
    decision: &proto::ProfileDecision,
) -> Result<ProfileOutcome, ProfileError> {
    use proto::ProfileStatus;

    let profile_id = ProfileId::try_from(decision.profile_id.as_slice())
        .map_err(|_| ProfileError::InvalidProfileId)?;
    let status =
        ProfileStatus::try_from(decision.status).map_err(|_| ProfileError::InvalidStatus)?;
    match status {
        ProfileStatus::Ready => decode_ready(decision, profile_id).map(ProfileOutcome::Ready),
        ProfileStatus::ClientUpgradeRequired => {
            require_empty_manifest(decision)?;
            if decision.required_runtime_abi == 0 {
                return Err(ProfileError::InvalidStatus);
            }
            Ok(ProfileOutcome::ClientUpgradeRequired {
                profile_id,
                required_runtime_abi: decision.required_runtime_abi,
                policy_epoch: decision.policy_epoch,
            })
        }
        ProfileStatus::Unavailable => {
            require_empty_manifest(decision)?;
            Ok(ProfileOutcome::Unavailable {
                profile_id,
                policy_epoch: decision.policy_epoch,
            })
        }
        ProfileStatus::Unspecified => Err(ProfileError::InvalidStatus),
    }
}

fn decode_ready(
    decision: &proto::ProfileDecision,
    profile_id: ProfileId,
) -> Result<ReadyProfile, ProfileError> {
    if decision.manifest.is_empty()
        || decision.manifest.len() > MAX_MANIFEST_LEN
        || decision.expires_at_ms == 0
    {
        return Err(ProfileError::IncompleteReady);
    }
    let manifest_digest = Digest32::try_from(decision.manifest_digest.as_slice())
        .map_err(|_| ProfileError::IncompleteReady)?;
    let manifest_signature = decision
        .manifest_signature
        .as_slice()
        .try_into()
        .map_err(|_| ProfileError::IncompleteReady)?;
    Ok(ReadyProfile {
        profile_id,
        manifest: decision.manifest.clone().into_boxed_slice(),
        manifest_digest,
        manifest_signature,
        expires_at_ms: decision.expires_at_ms,
        policy_epoch: decision.policy_epoch,
    })
}

fn require_empty_manifest(decision: &proto::ProfileDecision) -> Result<(), ProfileError> {
    if decision.manifest.is_empty()
        && decision.manifest_digest.is_empty()
        && decision.manifest_signature.is_empty()
        && decision.expires_at_ms == 0
    {
        Ok(())
    } else {
        Err(ProfileError::UnexpectedManifest)
    }
}

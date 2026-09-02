use std::collections::BTreeSet;

use prost::Message;

use crate::{
    CodecError, Digest32, InstallationId, MAX_ACCESS_TOKEN_LEN, MAX_RUNTIME_LEASE_LEN, SessionId,
    bounds::MAX_CONTRACTS, proto,
};

const HELLO_SIGNATURE_DOMAIN: &[u8] = b"Ceylith session hello v2\0";
const PROFILE_SIGNATURE_DOMAIN: &[u8] = b"Ceylith profile decision v2\0";

/// Closed authorization class returned by Ceylith.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrantClass {
    /// Anonymous Basic capability.
    Public,
    /// Reviewed Token with bounded Full capability.
    Community,
    /// Owner or trusted-person Full capability.
    Full,
}

/// Structurally validated session admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionAdmission {
    session_id: SessionId,
    runtime_lease: Box<[u8]>,
    lease_expires_at_ms: u64,
    grant_class: GrantClass,
    max_full_accounts: u32,
    max_active_installations: u32,
    max_registered_installations: u32,
    server_time_ms: u64,
    policy_epoch: u64,
    accepted_contracts: Box<[u32]>,
}

impl SessionAdmission {
    /// Secure-session identifier.
    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// Opaque runtime lease.
    #[must_use]
    pub fn runtime_lease(&self) -> &[u8] {
        &self.runtime_lease
    }

    /// Exclusive runtime-lease expiry.
    #[must_use]
    pub const fn lease_expires_at_ms(&self) -> u64 {
        self.lease_expires_at_ms
    }

    /// Effective grant class.
    #[must_use]
    pub const fn grant_class(&self) -> GrantClass {
        self.grant_class
    }

    /// Maximum simultaneous Full accounts; zero is valid only for Full policy.
    #[must_use]
    pub const fn max_full_accounts(&self) -> u32 {
        self.max_full_accounts
    }

    /// Maximum active installations; zero is valid only for Full policy.
    #[must_use]
    pub const fn max_active_installations(&self) -> u32 {
        self.max_active_installations
    }

    /// Maximum registered installations; zero is valid only for Full policy.
    #[must_use]
    pub const fn max_registered_installations(&self) -> u32 {
        self.max_registered_installations
    }

    /// Authenticated server wall-clock sample.
    #[must_use]
    pub const fn server_time_ms(&self) -> u64 {
        self.server_time_ms
    }

    /// Server policy generation.
    #[must_use]
    pub const fn policy_epoch(&self) -> u64 {
        self.policy_epoch
    }

    /// Accepted public contract identifiers.
    #[must_use]
    pub fn accepted_contracts(&self) -> &[u32] {
        &self.accepted_contracts
    }
}

/// Validates a public runtime advertisement.
///
/// # Errors
///
/// Returns an error when a required identifier, digest, or contract list is invalid.
pub fn validate_client_runtime(runtime: &proto::ClientRuntime) -> Result<(), CodecError> {
    if runtime.runtime_abi == 0
        || runtime.envelope_contract == 0
        || runtime.platform == 0
        || runtime.architecture == 0
    {
        return Err(CodecError::InvalidField);
    }
    Digest32::try_from(runtime.build_digest.as_slice()).map_err(|_| CodecError::InvalidField)?;
    validate_contracts(&runtime.action_contracts)?;
    validate_contracts(&runtime.source_contracts)
}

/// Validates a client hello before signature verification or admission.
///
/// # Errors
///
/// Returns an error when identity material, token bounds, or runtime fields are invalid.
pub fn validate_session_hello(hello: &proto::SessionHello) -> Result<(), CodecError> {
    InstallationId::try_from(hello.installation_id.as_slice())
        .map_err(|_| CodecError::InvalidField)?;
    validate_exact(&hello.installation_sign_public_key, 32)?;
    validate_exact(&hello.installation_noise_public_key, 32)?;
    validate_exact(&hello.transcript_signature, 64)?;
    if hello.access_token.len() > MAX_ACCESS_TOKEN_LEN {
        return Err(CodecError::InvalidField);
    }
    validate_client_runtime(hello.runtime.as_ref().ok_or(CodecError::InvalidField)?)
}

/// Builds the canonical bytes signed by the installation signing key.
///
/// # Errors
///
/// Returns an error if the transcript length overflows or protobuf encoding fails.
pub fn session_hello_signing_transcript(
    hello: &proto::SessionHello,
) -> Result<Vec<u8>, CodecError> {
    let mut unsigned = hello.clone();
    unsigned.transcript_signature.clear();
    let encoded_len = unsigned.encoded_len();
    let mut transcript = Vec::with_capacity(
        HELLO_SIGNATURE_DOMAIN
            .len()
            .checked_add(encoded_len)
            .ok_or(CodecError::LengthOverflow)?,
    );
    transcript.extend_from_slice(HELLO_SIGNATURE_DOMAIN);
    unsigned
        .encode(&mut transcript)
        .map_err(|_| CodecError::Protobuf)?;
    Ok(transcript)
}

/// Builds the canonical bytes signed for a cacheable ready profile decision.
///
/// # Errors
///
/// Returns an error if the transcript length overflows or protobuf encoding fails.
pub fn profile_decision_signing_transcript(
    decision: &proto::ProfileDecision,
) -> Result<Vec<u8>, CodecError> {
    let mut unsigned = decision.clone();
    unsigned.manifest_signature.clear();
    let encoded_len = unsigned.encoded_len();
    let mut transcript = Vec::with_capacity(
        PROFILE_SIGNATURE_DOMAIN
            .len()
            .checked_add(encoded_len)
            .ok_or(CodecError::LengthOverflow)?,
    );
    transcript.extend_from_slice(PROFILE_SIGNATURE_DOMAIN);
    unsigned
        .encode(&mut transcript)
        .map_err(|_| CodecError::Protobuf)?;
    Ok(transcript)
}

/// Decodes and validates a successful secure-session admission.
///
/// # Errors
///
/// Returns an error when identity, lease, grant, quota, or contract fields are invalid.
pub fn decode_session_welcome(
    welcome: &proto::SessionWelcome,
) -> Result<SessionAdmission, CodecError> {
    let session_id =
        SessionId::try_from(welcome.session_id.as_slice()).map_err(|_| CodecError::InvalidField)?;
    if welcome.runtime_lease.is_empty() || welcome.runtime_lease.len() > MAX_RUNTIME_LEASE_LEN {
        return Err(CodecError::InvalidField);
    }
    if welcome.lease_expires_at_ms <= welcome.server_time_ms
        || welcome.server_time_ms == 0
        || welcome.policy_epoch == 0
    {
        return Err(CodecError::InvalidField);
    }
    let grant_class = match proto::GrantClass::try_from(welcome.grant_class)
        .map_err(|_| CodecError::InvalidField)?
    {
        proto::GrantClass::Public => GrantClass::Public,
        proto::GrantClass::Community => GrantClass::Community,
        proto::GrantClass::Full => GrantClass::Full,
        proto::GrantClass::Unspecified => return Err(CodecError::InvalidField),
    };
    validate_quotas(welcome, grant_class)?;
    validate_contracts(&welcome.accepted_contracts)?;
    Ok(SessionAdmission {
        session_id,
        runtime_lease: welcome.runtime_lease.clone().into_boxed_slice(),
        lease_expires_at_ms: welcome.lease_expires_at_ms,
        grant_class,
        max_full_accounts: welcome.max_full_accounts,
        max_active_installations: welcome.max_active_installations,
        max_registered_installations: welcome.max_registered_installations,
        server_time_ms: welcome.server_time_ms,
        policy_epoch: welcome.policy_epoch,
        accepted_contracts: welcome.accepted_contracts.clone().into_boxed_slice(),
    })
}

fn validate_quotas(
    welcome: &proto::SessionWelcome,
    grant_class: GrantClass,
) -> Result<(), CodecError> {
    match grant_class {
        GrantClass::Public if welcome.max_full_accounts != 0 => Err(CodecError::InvalidField),
        GrantClass::Community
            if welcome.max_full_accounts == 0
                || welcome.max_active_installations == 0
                || welcome.max_registered_installations == 0 =>
        {
            Err(CodecError::InvalidField)
        }
        _ => Ok(()),
    }
}

fn validate_contracts(contracts: &[u32]) -> Result<(), CodecError> {
    if contracts.is_empty() || contracts.len() > MAX_CONTRACTS {
        return Err(CodecError::InvalidField);
    }
    let mut unique = BTreeSet::new();
    for contract in contracts {
        if *contract == 0 || !unique.insert(*contract) {
            return Err(CodecError::InvalidField);
        }
    }
    Ok(())
}

fn validate_exact(value: &[u8], expected: usize) -> Result<(), CodecError> {
    if value.len() == expected {
        Ok(())
    } else {
        Err(CodecError::InvalidField)
    }
}

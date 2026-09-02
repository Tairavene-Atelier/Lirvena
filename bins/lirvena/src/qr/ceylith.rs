use std::io;

use ceylith_client::{
    AccessToken, Architecture, CeylithTcpClient, InstallationClient, InstallationClientRuntime,
    InstallationIdentity, OpaqueExchangeContext, Platform, ProfileVerifier, RequestedAccess,
    RuntimeDescriptor, decode_opaque_exchange_response, spawn_installation_client,
};
use ceylith_crypto::NoisePublicKey;
use ceylith_protocol::{
    AccountSlotId, Digest32, ExchangeId, OpaqueSlot, OpaqueSlotId, OpaqueSlots, ProfileId,
    ProfileOutcome, WireLimits, opaque_binding_digest, proto,
};
use qq_profile::{LinuxNtProfile, decode_linux_manifest};
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

use crate::config::ProcessConfig;
use crate::support::{now_ms, random_array};

const PROFILE_SLOT: u32 = 1;
const REQUEST_SLOT_A: u32 = 1_001;
const REQUEST_SLOT_B: u32 = 1_002;
const REQUEST_SLOT_C: u32 = 1_901;
const REQUEST_SLOT_D: u32 = 1_902;
const RESPONSE_SLOT: u32 = 2_001;
const REQUEST_LIFETIME_MS: u64 = 10_000;

#[derive(Clone, Copy)]
pub(super) struct OpaqueOperation(u32);

impl OpaqueOperation {
    pub(super) const A: Self = Self(1);
    pub(super) const B: Self = Self(2);
}

#[derive(Clone)]
pub(super) struct NegotiatedProfile {
    pub(super) profile: LinuxNtProfile,
    pub(super) manifest_digest: Digest32,
}

pub(super) fn runtime() -> Result<RuntimeDescriptor, Box<dyn std::error::Error>> {
    let build_digest = Sha256::digest(concat!(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")));
    Ok(RuntimeDescriptor::new(
        3,
        2,
        vec![1],
        vec![1],
        Platform::Linux,
        Architecture::X86_64,
        Digest32::from_bytes(build_digest.into()),
    )?)
}

pub(super) async fn connect(
    config: &ProcessConfig,
    runtime: &RuntimeDescriptor,
) -> Result<InstallationClientRuntime, Box<dyn std::error::Error>> {
    let mut signing_seed = *config.installation_signing_seed;
    let mut noise_seed = *config.installation_noise_seed;
    let identity = InstallationIdentity::from_parts(
        ceylith_protocol::InstallationId::from_bytes(config.installation_id),
        signing_seed,
        noise_seed,
    );
    signing_seed.zeroize();
    noise_seed.zeroize();
    let token = config
        .token
        .as_ref()
        .map(|value| AccessToken::new(value.to_vec()))
        .transpose()?;
    let connection = CeylithTcpClient::connect(
        config.ceylith_address,
        &identity,
        NoisePublicKey::try_from_bytes(config.ceylith_noise_public_key)?,
        token.as_ref(),
        runtime,
        0,
        WireLimits::default(),
    )
    .await?;
    Ok(spawn_installation_client(connection, 64)?)
}

pub(super) fn ensure_matching_admission(
    operations: &ceylith_protocol::SessionAdmission,
    watch: &ceylith_protocol::SessionAdmission,
) -> Result<(), io::Error> {
    let matches = operations.grant_class() == watch.grant_class()
        && operations.max_full_accounts() == watch.max_full_accounts()
        && operations.max_active_installations() == watch.max_active_installations()
        && operations.max_registered_installations() == watch.max_registered_installations()
        && operations.policy_epoch() == watch.policy_epoch()
        && operations.accepted_contracts() == watch.accepted_contracts();
    if matches {
        Ok(())
    } else {
        Err(io::Error::other(
            "Ceylith operation and Watch admissions do not match",
        ))
    }
}

pub(super) async fn negotiate_profile(
    ceylith: &InstallationClient,
    runtime: &RuntimeDescriptor,
    config: &ProcessConfig,
    requested_access: RequestedAccess,
) -> Result<NegotiatedProfile, Box<dyn std::error::Error>> {
    let request = ceylith.profile_request(
        ProfileId::from_bytes(config.profile_id),
        None,
        requested_access,
        runtime,
    );
    let response = ceylith.exchange(request).await?;
    let Some(proto::inner_frame::Body::ProfileDecision(decision)) = response.body.as_ref() else {
        return Err(io::Error::other("Ceylith returned no Profile decision").into());
    };
    let verifier = ProfileVerifier::from_bytes(&config.ceylith_profile_verifying_key)?;
    let ProfileOutcome::Ready(ready) = verifier.verify(decision, now_ms()?)? else {
        return Err(io::Error::other("Ceylith Profile is not ready").into());
    };
    Ok(NegotiatedProfile {
        profile: decode_linux_manifest(ready.manifest())?,
        manifest_digest: ready.manifest_digest(),
    })
}

pub(super) fn profile_peer(profile: &LinuxNtProfile) -> Result<&[u8], io::Error> {
    required_slot(profile.opaque_slots(), PROFILE_SLOT)
}

pub(super) async fn request_reserve(
    ceylith: &InstallationClient,
    account_slot_id: AccountSlotId,
    operation: OpaqueOperation,
    body: &[u8],
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let slots = request_slots(operation, body)?;
    let exchange_id = ExchangeId::from_bytes(random_array()?);
    let issued_at_ms = now_ms()?;
    let expires_at_ms = issued_at_ms
        .checked_add(REQUEST_LIFETIME_MS)
        .ok_or_else(|| io::Error::other("request deadline overflow"))?;
    let context = OpaqueExchangeContext {
        exchange_id,
        account_slot_id,
        generation: 1,
        expires_at_ms,
        binding_digest: opaque_binding_digest(
            exchange_id,
            account_slot_id,
            1,
            expires_at_ms,
            &slots,
        ),
    };
    let request = ceylith.opaque_exchange_request(context, &slots, issued_at_ms)?;
    let response = ceylith.exchange(request).await?;
    let result = decode_opaque_exchange_response(&response, context, now_ms()?)?;
    Ok(required_slot(result.slots(), RESPONSE_SLOT)?.to_vec())
}

fn request_slots(
    operation: OpaqueOperation,
    body: &[u8],
) -> Result<OpaqueSlots, Box<dyn std::error::Error>> {
    Ok(OpaqueSlots::new(vec![
        opaque_slot(REQUEST_SLOT_A, operation.0.to_be_bytes().to_vec())?,
        opaque_slot(REQUEST_SLOT_B, body.to_vec())?,
        opaque_slot(REQUEST_SLOT_C, random_array::<16>()?.to_vec())?,
        opaque_slot(REQUEST_SLOT_D, random_array::<32>()?.to_vec())?,
    ])?)
}

fn opaque_slot(id: u32, value: Vec<u8>) -> Result<OpaqueSlot, Box<dyn std::error::Error>> {
    Ok(OpaqueSlot::new(OpaqueSlotId::new(id)?, value)?)
}

fn required_slot(slots: &OpaqueSlots, id: u32) -> Result<&[u8], io::Error> {
    let id = OpaqueSlotId::new(id).map_err(|_| io::Error::other("compiled slot is invalid"))?;
    slots
        .get(id)
        .map(OpaqueSlot::value)
        .ok_or_else(|| io::Error::other("required opaque slot is missing"))
}

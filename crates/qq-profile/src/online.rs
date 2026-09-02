use core::fmt;

use ceylith_protocol::OpaqueSlotId;
use qq_domain::{OnlinePlan, OnlinePlanSpec, PlanActionId};
use qq_wire::{WireReader, WireWriter};

use crate::LinuxNtProfile;

const MAGIC: [u8; 4] = *b"LQOP";
const VERSION: u16 = 1;
const MAX_PLAN_LEN: usize = 256;

/// Numeric Profile slot carrying the bounded online plan.
pub const ONLINE_PLAN_SLOT_ID: u32 = 2;

/// Rejected or missing canonical online plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OnlinePlanManifestError;

impl fmt::Display for OnlinePlanManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("profile online plan rejected")
    }
}

impl std::error::Error for OnlinePlanManifestError {}

/// Encodes one validated online plan into its canonical opaque Profile slot.
///
/// # Errors
///
/// Returns an error if the fixed representation exceeds its compiled bound.
pub fn encode_online_plan(plan: OnlinePlan) -> Result<Vec<u8>, OnlinePlanManifestError> {
    let spec = plan.spec();
    let mut writer = WireWriter::new(MAX_PLAN_LEN);
    writer.put_bytes(&MAGIC).map_err(map_wire)?;
    writer.put_u16(VERSION).map_err(map_wire)?;
    put_action(&mut writer, spec.initial_sync)?;
    put_action(&mut writer, spec.delayed_sync)?;
    put_action(&mut writer, spec.security_bootstrap)?;
    if let Some(action) = spec.status_confirmation {
        writer.put_u8(1).map_err(map_wire)?;
        put_action(&mut writer, action)?;
    } else {
        writer.put_u8(0).map_err(map_wire)?;
    }
    put_action(&mut writer, spec.business_heartbeat)?;
    writer
        .put_u64(spec.initial_heartbeat_ms)
        .map_err(map_wire)?;
    writer
        .put_u64(spec.minimum_heartbeat_ms)
        .map_err(map_wire)?;
    writer
        .put_u64(spec.maximum_heartbeat_ms)
        .map_err(map_wire)?;
    writer
        .put_u64(spec.minimum_delayed_sync_ms)
        .map_err(map_wire)?;
    writer
        .put_u64(spec.maximum_delayed_sync_ms)
        .map_err(map_wire)?;
    Ok(writer.finish())
}

/// Decodes and revalidates the online plan carried by a signed Linux Profile.
///
/// # Errors
///
/// Returns an error for a missing, malformed, excessive or non-canonical slot.
pub fn decode_online_plan(profile: &LinuxNtProfile) -> Result<OnlinePlan, OnlinePlanManifestError> {
    let id = OpaqueSlotId::new(ONLINE_PLAN_SLOT_ID).map_err(|_| OnlinePlanManifestError)?;
    let bytes = profile
        .opaque_slots()
        .get(id)
        .ok_or(OnlinePlanManifestError)?
        .value();
    decode(bytes)
}

fn decode(bytes: &[u8]) -> Result<OnlinePlan, OnlinePlanManifestError> {
    if bytes.len() > MAX_PLAN_LEN {
        return Err(OnlinePlanManifestError);
    }
    let mut reader = WireReader::new(bytes);
    if reader.read_bytes(MAGIC.len()).map_err(map_wire)? != MAGIC
        || reader.read_u16().map_err(map_wire)? != VERSION
    {
        return Err(OnlinePlanManifestError);
    }
    let initial_sync = read_action(&mut reader)?;
    let delayed_sync = read_action(&mut reader)?;
    let security_bootstrap = read_action(&mut reader)?;
    let status_confirmation = match reader.read_u8().map_err(map_wire)? {
        0 => None,
        1 => Some(read_action(&mut reader)?),
        _ => return Err(OnlinePlanManifestError),
    };
    let business_heartbeat = read_action(&mut reader)?;
    let plan = OnlinePlan::new(OnlinePlanSpec {
        initial_sync,
        delayed_sync,
        security_bootstrap,
        status_confirmation,
        business_heartbeat,
        initial_heartbeat_ms: reader.read_u64().map_err(map_wire)?,
        minimum_heartbeat_ms: reader.read_u64().map_err(map_wire)?,
        maximum_heartbeat_ms: reader.read_u64().map_err(map_wire)?,
        minimum_delayed_sync_ms: reader.read_u64().map_err(map_wire)?,
        maximum_delayed_sync_ms: reader.read_u64().map_err(map_wire)?,
    })
    .map_err(|_| OnlinePlanManifestError)?;
    reader.finish().map_err(map_wire)?;
    if encode_online_plan(plan)? != bytes {
        return Err(OnlinePlanManifestError);
    }
    Ok(plan)
}

fn put_action(
    writer: &mut WireWriter,
    action: PlanActionId,
) -> Result<(), OnlinePlanManifestError> {
    writer.put_bytes(&action.as_bytes()).map_err(map_wire)
}

fn read_action(reader: &mut WireReader<'_>) -> Result<PlanActionId, OnlinePlanManifestError> {
    let bytes: [u8; 16] = reader
        .read_bytes(16)
        .map_err(map_wire)?
        .try_into()
        .map_err(|_| OnlinePlanManifestError)?;
    PlanActionId::new(bytes).map_err(|_| OnlinePlanManifestError)
}

fn map_wire(_: qq_wire::WireError) -> OnlinePlanManifestError {
    OnlinePlanManifestError
}

use ceylith_protocol::OpaqueSlotId;
use qq_wire::{LengthPrefix, WireReader, WireWriter};

use super::{
    OnlinePacketPlan, OnlinePacketPlanError, OnlinePacketPlanSpec, OnlinePacketTuning,
    OnlinePacketTuningSpec,
};
use crate::LinuxNtProfile;

const MAGIC: [u8; 4] = *b"LQPW";
const VERSION: u16 = 1;
const MAX_PLAN_LEN: usize = 512;
const MAX_ROUTE_LEN: usize = 128;

/// Numeric Profile slot carrying packet routes and version-selected values.
pub const ONLINE_PACKET_PLAN_SLOT_ID: u32 = 3;

/// Encodes one validated packet plan into its canonical opaque Profile slot.
///
/// # Errors
///
/// Returns an error if the representation exceeds its compiled bound.
pub fn encode_online_packet_plan(
    plan: &OnlinePacketPlan,
) -> Result<Vec<u8>, OnlinePacketPlanError> {
    let mut writer = WireWriter::new(MAX_PLAN_LEN);
    writer.put_bytes(&MAGIC).map_err(map_wire)?;
    writer.put_u16(VERSION).map_err(map_wire)?;
    put_route(&mut writer, plan.initial_sync_route())?;
    put_route(&mut writer, plan.delayed_sync_route())?;
    if let Some(route) = plan.status_register_route() {
        writer.put_u8(1).map_err(map_wire)?;
        put_route(&mut writer, route)?;
    } else {
        writer.put_u8(0).map_err(map_wire)?;
    }
    put_route(&mut writer, plan.heartbeat_route())?;
    put_tuning(&mut writer, plan.tuning())?;
    Ok(writer.finish())
}

/// Decodes and revalidates the packet plan carried by a signed Linux Profile.
///
/// # Errors
///
/// Returns an error for a missing, malformed, excessive or non-canonical slot.
pub fn decode_online_packet_plan(
    profile: &LinuxNtProfile,
) -> Result<OnlinePacketPlan, OnlinePacketPlanError> {
    let id = OpaqueSlotId::new(ONLINE_PACKET_PLAN_SLOT_ID).map_err(|_| OnlinePacketPlanError)?;
    let bytes = profile
        .opaque_slots()
        .get(id)
        .ok_or(OnlinePacketPlanError)?
        .value();
    decode(bytes)
}

fn decode(bytes: &[u8]) -> Result<OnlinePacketPlan, OnlinePacketPlanError> {
    if bytes.len() > MAX_PLAN_LEN {
        return Err(OnlinePacketPlanError);
    }
    let mut reader = WireReader::new(bytes);
    if reader.read_bytes(MAGIC.len()).map_err(map_wire)? != MAGIC
        || reader.read_u16().map_err(map_wire)? != VERSION
    {
        return Err(OnlinePacketPlanError);
    }
    let initial_sync_route = read_route(&mut reader)?;
    let delayed_sync_route = read_route(&mut reader)?;
    let status_register_route = match reader.read_u8().map_err(map_wire)? {
        0 => None,
        1 => Some(read_route(&mut reader)?),
        _ => return Err(OnlinePacketPlanError),
    };
    let heartbeat_route = read_route(&mut reader)?;
    let tuning = read_tuning(&mut reader)?;
    reader.finish().map_err(map_wire)?;
    let plan = OnlinePacketPlan::new(OnlinePacketPlanSpec {
        initial_sync_route,
        delayed_sync_route,
        status_register_route,
        heartbeat_route,
        tuning,
    })?;
    if encode_online_packet_plan(&plan)? != bytes {
        return Err(OnlinePacketPlanError);
    }
    Ok(plan)
}

fn put_route(writer: &mut WireWriter, route: &str) -> Result<(), OnlinePacketPlanError> {
    writer
        .put_prefixed_bytes(LengthPrefix::U8Payload, route.as_bytes())
        .map_err(map_wire)
}

fn read_route(reader: &mut WireReader<'_>) -> Result<String, OnlinePacketPlanError> {
    let bytes = reader
        .read_prefixed_bytes(LengthPrefix::U8Payload, MAX_ROUTE_LEN)
        .map_err(map_wire)?;
    core::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_error| OnlinePacketPlanError)
}

fn put_tuning(
    writer: &mut WireWriter,
    tuning: OnlinePacketTuning,
) -> Result<(), OnlinePacketPlanError> {
    let spec = tuning.spec();
    writer.put_u32(spec.sync_flag).map_err(map_wire)?;
    put_i32(writer, spec.locale_id)?;
    put_i32(writer, spec.initial_vendor_type)?;
    put_i32(writer, spec.initial_register_type)?;
    put_i32(writer, spec.status_vendor_type)?;
    put_i32(writer, spec.status_register_type)?;
    writer.put_u32(spec.auxiliary_flag).map_err(map_wire)?;
    put_i32(writer, spec.heartbeat_type)
}

fn read_tuning(reader: &mut WireReader<'_>) -> Result<OnlinePacketTuning, OnlinePacketPlanError> {
    OnlinePacketTuning::new(OnlinePacketTuningSpec {
        sync_flag: reader.read_u32().map_err(map_wire)?,
        locale_id: reader.read_i32().map_err(map_wire)?,
        initial_vendor_type: reader.read_i32().map_err(map_wire)?,
        initial_register_type: reader.read_i32().map_err(map_wire)?,
        status_vendor_type: reader.read_i32().map_err(map_wire)?,
        status_register_type: reader.read_i32().map_err(map_wire)?,
        auxiliary_flag: reader.read_u32().map_err(map_wire)?,
        heartbeat_type: reader.read_i32().map_err(map_wire)?,
    })
}

fn put_i32(writer: &mut WireWriter, value: i32) -> Result<(), OnlinePacketPlanError> {
    writer.put_bytes(&value.to_be_bytes()).map_err(map_wire)
}

fn map_wire(_: qq_wire::WireError) -> OnlinePacketPlanError {
    OnlinePacketPlanError
}

use ceylith_protocol::OpaqueSlotId;
use qq_wire::{LengthPrefix, WireReader, WireWriter};

use super::{PushBehavior, PushPlan, PushPlanEntry, PushPlanError, PushPlanSpec};
use crate::LinuxNtProfile;

const MAGIC: [u8; 4] = *b"LQPS";
const VERSION: u16 = 1;
const MAX_PLAN_LEN: usize = 4 * 1024;
const MAX_ROUTE_LEN: usize = 128;

/// Numeric Profile slot carrying bounded Push routes and compiled behavior selectors.
pub const PUSH_PLAN_SLOT_ID: u32 = 4;

/// Encodes one validated Push plan into its canonical opaque Profile slot.
///
/// # Errors
///
/// Returns an error if the representation exceeds its compiled bound.
pub fn encode_push_plan(plan: &PushPlan) -> Result<Vec<u8>, PushPlanError> {
    let mut writer = WireWriter::new(MAX_PLAN_LEN);
    writer.put_bytes(&MAGIC).map_err(map_wire)?;
    writer.put_u16(VERSION).map_err(map_wire)?;
    writer
        .put_u8(u8::try_from(plan.entries().len()).map_err(|_| PushPlanError)?)
        .map_err(map_wire)?;
    for entry in plan.entries() {
        put_route(&mut writer, entry.route())?;
        writer.put_u8(entry.behavior() as u8).map_err(map_wire)?;
        if let Some(route) = entry.response_route() {
            writer.put_u8(1).map_err(map_wire)?;
            put_route(&mut writer, route)?;
        } else {
            writer.put_u8(0).map_err(map_wire)?;
        }
        writer.put_u32(entry.parameter()).map_err(map_wire)?;
        writer.put_u32(entry.maximum_body_len()).map_err(map_wire)?;
    }
    Ok(writer.finish())
}

/// Decodes and revalidates the Push plan carried by a signed Linux Profile.
///
/// # Errors
///
/// Returns an error for a missing, malformed, excessive or non-canonical slot.
pub fn decode_push_plan(profile: &LinuxNtProfile) -> Result<PushPlan, PushPlanError> {
    let id = OpaqueSlotId::new(PUSH_PLAN_SLOT_ID).map_err(|_| PushPlanError)?;
    let bytes = profile.opaque_slots().get(id).ok_or(PushPlanError)?.value();
    decode(bytes)
}

fn decode(bytes: &[u8]) -> Result<PushPlan, PushPlanError> {
    if bytes.len() > MAX_PLAN_LEN {
        return Err(PushPlanError);
    }
    let mut reader = WireReader::new(bytes);
    if reader.read_bytes(MAGIC.len()).map_err(map_wire)? != MAGIC
        || reader.read_u16().map_err(map_wire)? != VERSION
    {
        return Err(PushPlanError);
    }
    let count = usize::from(reader.read_u8().map_err(map_wire)?);
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let route = read_route(&mut reader)?;
        let behavior = PushBehavior::from_wire(reader.read_u8().map_err(map_wire)?)?;
        let response_route = match reader.read_u8().map_err(map_wire)? {
            0 => None,
            1 => Some(read_route(&mut reader)?),
            _ => return Err(PushPlanError),
        };
        let parameter = reader.read_u32().map_err(map_wire)?;
        let maximum_body_len = reader.read_u32().map_err(map_wire)?;
        entries.push(PushPlanEntry::new(
            route,
            behavior,
            response_route,
            parameter,
            maximum_body_len,
        )?);
    }
    reader.finish().map_err(map_wire)?;
    let plan = PushPlan::new(PushPlanSpec { entries })?;
    if encode_push_plan(&plan)? != bytes {
        return Err(PushPlanError);
    }
    Ok(plan)
}

fn put_route(writer: &mut WireWriter, route: &str) -> Result<(), PushPlanError> {
    writer
        .put_prefixed_bytes(LengthPrefix::U8Payload, route.as_bytes())
        .map_err(map_wire)
}

fn read_route(reader: &mut WireReader<'_>) -> Result<String, PushPlanError> {
    let bytes = reader
        .read_prefixed_bytes(LengthPrefix::U8Payload, MAX_ROUTE_LEN)
        .map_err(map_wire)?;
    core::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_error| PushPlanError)
}

fn map_wire(_: qq_wire::WireError) -> PushPlanError {
    PushPlanError
}

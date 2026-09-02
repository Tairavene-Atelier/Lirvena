use std::io;

use ceylith_client::{InstallationClient, OpaqueExchangeContext, decode_opaque_exchange_response};
use ceylith_protocol::{
    AccountSlotId, ExchangeId, OpaqueSlot, OpaqueSlotId, OpaqueSlots, opaque_binding_digest,
};

use crate::support::{now_ms, random_array};

const REQUEST_SLOT_A: u32 = 1_001;
const REQUEST_SLOT_B: u32 = 1_002;
const REQUEST_SLOT_C: u32 = 1_901;
const REQUEST_SLOT_D: u32 = 1_902;
const RESPONSE_SLOT: u32 = 2_001;
const REQUEST_LIFETIME_MS: u64 = 10_000;

#[derive(Clone, Copy)]
pub(crate) struct OpaqueOperation(u32);

impl OpaqueOperation {
    pub(crate) const A: Self = Self(1);
    pub(crate) const B: Self = Self(2);
    pub(crate) const C: Self = Self(3);

    pub(crate) const fn numeric(value: u32) -> Self {
        Self(value)
    }
}

pub(crate) async fn request_reserve(
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

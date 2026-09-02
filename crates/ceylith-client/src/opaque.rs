use ceylith_protocol::{
    AccountSlotId, CURRENT_INNER_CONTRACT, Digest32, ExchangeId, OpaqueSlots, proto,
};

use crate::ClientError;

/// Binding values for one short-lived opaque Ceylith exchange.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpaqueExchangeContext {
    /// Logical exchange identifier.
    pub exchange_id: ExchangeId,
    /// Local account runtime slot.
    pub account_slot_id: AccountSlotId,
    /// Account lifecycle generation.
    pub generation: u64,
    /// Exclusive request deadline.
    pub expires_at_ms: u64,
    /// Digest binding the request to its local operation.
    pub binding_digest: Digest32,
}

/// Validated opaque Ceylith exchange response.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpaqueExchangeResult {
    context: OpaqueExchangeContext,
    slots: OpaqueSlots,
}

impl OpaqueExchangeResult {
    /// Returns the authenticated exchange binding.
    #[must_use]
    pub const fn context(&self) -> OpaqueExchangeContext {
        self.context
    }

    /// Returns the response slots without attaching private semantics.
    #[must_use]
    pub const fn slots(&self) -> &OpaqueSlots {
        &self.slots
    }
}

/// Validates and binds an opaque response to one expected request.
///
/// # Errors
///
/// Returns an error for the wrong body, binding mismatch, expiry or invalid slots.
pub fn decode_opaque_exchange_response(
    frame: &proto::InnerFrame,
    expected: OpaqueExchangeContext,
    now_ms: u64,
) -> Result<OpaqueExchangeResult, ClientError> {
    if frame.contract != CURRENT_INNER_CONTRACT {
        return Err(ClientError::Protocol);
    }
    let Some(proto::inner_frame::Body::OpaqueExchangeResponse(response)) = frame.body.as_ref()
    else {
        return Err(ClientError::Protocol);
    };
    let exchange_id = ExchangeId::try_from(response.exchange_id.as_slice())
        .map_err(|_error| ClientError::Protocol)?;
    let binding_digest = Digest32::try_from(response.binding_digest.as_slice())
        .map_err(|_error| ClientError::Protocol)?;
    if exchange_id != expected.exchange_id
        || response.generation != expected.generation
        || binding_digest != expected.binding_digest
        || response.expires_at_ms == 0
        || response.expires_at_ms > expected.expires_at_ms
        || now_ms >= response.expires_at_ms
    {
        return Err(ClientError::SessionBinding);
    }
    let slots = OpaqueSlots::from_wire(&response.slots).map_err(|_error| ClientError::Protocol)?;
    if slots.is_empty() {
        return Err(ClientError::Protocol);
    }
    Ok(OpaqueExchangeResult {
        context: OpaqueExchangeContext {
            expires_at_ms: response.expires_at_ms,
            ..expected
        },
        slots,
    })
}

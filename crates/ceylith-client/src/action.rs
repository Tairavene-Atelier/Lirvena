use ceylith_protocol::{
    ActionFlowId, ActionId, Digest32, MAX_ACTION_MARKS, OpaqueSlot, OpaqueSlotId, OpaqueSlots,
    proto,
};

use crate::ClientError;

const INPUT_SLOT_A: u32 = 3_901;
const INPUT_SLOT_B: u32 = 3_902;

/// Constructs the fixed numeric input set for one authenticated action flow.
///
/// # Errors
///
/// Returns an error when any value exceeds the public slot bounds.
pub fn action_flow_inputs(first: Vec<u8>, second: Vec<u8>) -> Result<OpaqueSlots, ClientError> {
    OpaqueSlots::new(vec![
        input(INPUT_SLOT_A, first)?,
        input(INPUT_SLOT_B, second)?,
    ])
    .map_err(|_error| ClientError::Protocol)
}

/// One numeric envelope mark whose semantics stay in the compiled executor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionMark {
    slot: u16,
    value: Box<[u8]>,
}

impl ActionMark {
    /// Returns the numeric insertion slot.
    #[must_use]
    pub const fn slot(&self) -> u16 {
        self.slot
    }

    /// Borrows the opaque mark bytes.
    #[must_use]
    pub fn value(&self) -> &[u8] {
        &self.value
    }
}

/// Authenticated, bounded action for the current account transport.
#[derive(Clone, Eq, PartialEq)]
pub struct ActionDirective {
    action_id: ActionId,
    route_shard: Box<[u8]>,
    body_shard: Box<[u8]>,
    envelope_contract: u32,
    marks: Box<[ActionMark]>,
    expected_body_digest: Digest32,
    transport_epoch: u64,
    response_policy: u32,
    timeout_ms: u32,
    action_digest: Digest32,
    delay_ms: u32,
}

impl ActionDirective {
    /// Exact action identifier.
    #[must_use]
    pub const fn action_id(&self) -> ActionId {
        self.action_id
    }
    /// Opaque local route selector.
    #[must_use]
    pub fn route_shard(&self) -> &[u8] {
        &self.route_shard
    }
    /// Opaque request body.
    #[must_use]
    pub fn body_shard(&self) -> &[u8] {
        &self.body_shard
    }
    /// Compiled local envelope contract.
    #[must_use]
    pub const fn envelope_contract(&self) -> u32 {
        self.envelope_contract
    }
    /// Numeric envelope marks.
    #[must_use]
    pub fn marks(&self) -> &[ActionMark] {
        &self.marks
    }
    /// Expected digest after local envelope assembly.
    #[must_use]
    pub const fn expected_body_digest(&self) -> Digest32 {
        self.expected_body_digest
    }
    /// Required account transport generation.
    #[must_use]
    pub const fn transport_epoch(&self) -> u64 {
        self.transport_epoch
    }
    /// Closed response policy identifier.
    #[must_use]
    pub const fn response_policy(&self) -> u32 {
        self.response_policy
    }
    /// Per-action response timeout.
    #[must_use]
    pub const fn timeout_ms(&self) -> u32 {
        self.timeout_ms
    }
    /// Digest of the complete action contract.
    #[must_use]
    pub const fn action_digest(&self) -> Digest32 {
        self.action_digest
    }
    /// Required relative delay before writing the action.
    #[must_use]
    pub const fn delay_ms(&self) -> u32 {
        self.delay_ms
    }
}

impl core::fmt::Debug for ActionDirective {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ActionDirective")
            .field("action_id", &self.action_id)
            .field("route_len", &self.route_shard.len())
            .field("body_len", &self.body_shard.len())
            .field("envelope_contract", &self.envelope_contract)
            .field("mark_count", &self.marks.len())
            .field("transport_epoch", &self.transport_epoch)
            .field("response_policy", &self.response_policy)
            .field("timeout_ms", &self.timeout_ms)
            .field("delay_ms", &self.delay_ms)
            .finish_non_exhaustive()
    }
}

/// Next authenticated state of one action flow.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActionFlowUpdate {
    /// Execute exactly one bounded action and submit its observation.
    Action(ActionDirective),
    /// The flow reached a durable terminal state.
    Complete,
}

/// Decodes an already authenticated action-flow response and binds it to the expected flow.
///
/// # Errors
///
/// Returns an error for a mismatched flow, expired update or malformed directive.
pub fn decode_action_flow_update(
    frame: &proto::InnerFrame,
    expected_flow: ActionFlowId,
    now_ms: u64,
) -> Result<ActionFlowUpdate, ClientError> {
    let Some(proto::inner_frame::Body::ActionFlowUpdate(update)) = frame.body.as_ref() else {
        return Err(ClientError::Protocol);
    };
    if ActionFlowId::try_from(update.flow_id.as_slice()).map_err(|_| ClientError::Protocol)?
        != expected_flow
        || now_ms >= update.expires_at_ms
    {
        return Err(ClientError::Protocol);
    }
    match proto::ActionFlowStatus::try_from(update.status).map_err(|_| ClientError::Protocol)? {
        proto::ActionFlowStatus::Action => update
            .action
            .as_ref()
            .ok_or(ClientError::Protocol)
            .and_then(decode_action)
            .map(ActionFlowUpdate::Action),
        proto::ActionFlowStatus::Complete if update.action.is_none() => {
            Ok(ActionFlowUpdate::Complete)
        }
        proto::ActionFlowStatus::Complete | proto::ActionFlowStatus::Unspecified => {
            Err(ClientError::Protocol)
        }
    }
}

fn decode_action(value: &proto::ActionDirective) -> Result<ActionDirective, ClientError> {
    let marks = value
        .marks
        .iter()
        .map(|mark| {
            Ok(ActionMark {
                slot: u16::try_from(mark.slot).map_err(|_| ClientError::Protocol)?,
                value: mark.value.clone().into_boxed_slice(),
            })
        })
        .collect::<Result<Vec<_>, ClientError>>()?;
    if marks.len() > MAX_ACTION_MARKS {
        return Err(ClientError::Protocol);
    }
    Ok(ActionDirective {
        action_id: ActionId::try_from(value.action_id.as_slice())
            .map_err(|_| ClientError::Protocol)?,
        route_shard: value.route_shard.clone().into_boxed_slice(),
        body_shard: value.body_shard.clone().into_boxed_slice(),
        envelope_contract: value.envelope_contract,
        marks: marks.into_boxed_slice(),
        expected_body_digest: Digest32::try_from(value.expected_body_digest.as_slice())
            .map_err(|_| ClientError::Protocol)?,
        transport_epoch: value.transport_epoch,
        response_policy: value.response_policy,
        timeout_ms: value.timeout_ms,
        action_digest: Digest32::try_from(value.action_digest.as_slice())
            .map_err(|_| ClientError::Protocol)?,
        delay_ms: value.delay_ms,
    })
}

fn input(slot: u32, value: Vec<u8>) -> Result<OpaqueSlot, ClientError> {
    OpaqueSlot::new(
        OpaqueSlotId::new(slot).map_err(|_error| ClientError::Protocol)?,
        value,
    )
    .map_err(|_error| ClientError::Protocol)
}

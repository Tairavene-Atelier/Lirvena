use sha2::{Digest, Sha256};

use crate::{AccountSlotId, ActionFlowId, ActionId, Digest32, OpaqueSlots};

const FLOW_BINDING_DOMAIN: &[u8] = b"ceylith/v3/action-flow/v1";
const OBSERVATION_BINDING_DOMAIN: &[u8] = b"ceylith/v3/action-observation/v1";

/// Generation-bound context for starting one closed action flow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActionFlowContext {
    /// Client-created correlation identifier.
    pub flow_id: ActionFlowId,
    /// Local account slot without upstream account identifiers.
    pub account_slot_id: AccountSlotId,
    /// Login generation.
    pub login_epoch: u64,
    /// Online generation.
    pub online_epoch: u64,
    /// Current transport generation.
    pub transport_epoch: u64,
    /// Absolute request expiry.
    pub expires_at_ms: u64,
}

/// Closed neutral outcome set for one action observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ActionObservationKind {
    /// A bounded opaque response was received.
    Response = 1,
    /// The transport failed after admission.
    TransportFailure = 2,
    /// The response deadline elapsed.
    Timeout = 3,
    /// Execution was cancelled.
    Cancelled = 4,
    /// The response belonged to an old transport generation.
    StaleTransport = 5,
    /// Local envelope assembly failed.
    LocalEncodeFailure = 6,
}

/// Bound observation of one server-issued action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActionObservation<'a> {
    /// Parent action flow.
    pub flow_id: ActionFlowId,
    /// Exact action identifier.
    pub action_id: ActionId,
    /// Digest of the complete action contract.
    pub action_digest: Digest32,
    /// Neutral transport outcome.
    pub outcome: ActionObservationKind,
    /// Opaque response bytes, present only for `Response`.
    pub payload: &'a [u8],
    /// Local observation timestamp in Unix milliseconds.
    pub observed_at_ms: u64,
}

/// Computes the canonical digest for one action-flow begin request.
#[must_use]
pub fn action_flow_binding_digest(context: ActionFlowContext, inputs: &OpaqueSlots) -> Digest32 {
    let mut digest = Sha256::new();
    digest.update(FLOW_BINDING_DOMAIN);
    digest.update(context.flow_id.as_bytes());
    digest.update(context.account_slot_id.as_bytes());
    digest.update(context.login_epoch.to_be_bytes());
    digest.update(context.online_epoch.to_be_bytes());
    digest.update(context.transport_epoch.to_be_bytes());
    digest.update(context.expires_at_ms.to_be_bytes());
    digest.update(
        u32::try_from(inputs.len())
            .unwrap_or(u32::MAX)
            .to_be_bytes(),
    );
    for input in inputs.iter() {
        digest.update(input.id().get().to_be_bytes());
        digest.update(
            u32::try_from(input.value().len())
                .unwrap_or(u32::MAX)
                .to_be_bytes(),
        );
        digest.update(input.value());
    }
    Digest32::from_bytes(digest.finalize().into())
}

/// Computes the canonical digest for one action observation.
#[must_use]
pub fn action_observation_binding_digest(observation: ActionObservation<'_>) -> Digest32 {
    let mut digest = Sha256::new();
    digest.update(OBSERVATION_BINDING_DOMAIN);
    digest.update(observation.flow_id.as_bytes());
    digest.update(observation.action_id.as_bytes());
    digest.update(observation.action_digest.as_bytes());
    digest.update((observation.outcome as u32).to_be_bytes());
    digest.update(observation.observed_at_ms.to_be_bytes());
    digest.update(
        u32::try_from(observation.payload.len())
            .unwrap_or(u32::MAX)
            .to_be_bytes(),
    );
    digest.update(observation.payload);
    Digest32::from_bytes(digest.finalize().into())
}

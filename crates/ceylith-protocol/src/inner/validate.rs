use std::collections::BTreeSet;

use crate::bounds::MAX_WATCH_WAIT_MS;
use crate::{
    AccountSlotId, ActionFlowContext, ActionFlowId, ActionId, ActionObservation,
    ActionObservationKind, CodecError, Digest32, ExchangeId, IncidentId, MAX_ACTION_BODY_LEN,
    MAX_ACTION_DELAY_MS, MAX_ACTION_MARK_AGGREGATE_LEN, MAX_ACTION_MARK_LEN, MAX_ACTION_MARKS,
    MAX_ACTION_PAYLOAD_LEN, MAX_ACTION_ROUTE_LEN, MAX_ACTION_TIMEOUT_MS, MAX_OPAQUE_AGGREGATE_LEN,
    MAX_OPAQUE_SLOT_LEN, MAX_OPAQUE_SLOTS, MAX_RUNTIME_LEASE_LEN, MAX_WATCH_PAYLOAD_LEN,
    OpaqueSlots, ProfileId, action_flow_binding_digest, action_observation_binding_digest,
    decode_profile_outcome, inner::CURRENT_INNER_CONTRACT, proto,
};

pub(crate) fn validate_inner(frame: &proto::InnerFrame) -> Result<(), CodecError> {
    if frame.contract != CURRENT_INNER_CONTRACT {
        return Err(CodecError::InvalidContract);
    }
    let body = frame.body.as_ref().ok_or(CodecError::UnsupportedBody)?;
    match body {
        proto::inner_frame::Body::ProfileRequest(value) => validate_profile_request(value),
        proto::inner_frame::Body::ProfileDecision(value) => {
            decode_profile_outcome(value).map_err(|_| CodecError::InvalidField)?;
            Ok(())
        }
        proto::inner_frame::Body::OpaqueExchangeRequest(value) => validate_opaque_request(value),
        proto::inner_frame::Body::OpaqueExchangeResponse(value) => validate_opaque_response(value),
        proto::inner_frame::Body::WatchRequest(value) => validate_watch_request(value),
        proto::inner_frame::Body::WatchEvent(value) => validate_watch_event(value),
        proto::inner_frame::Body::ActionFlowBegin(value) => validate_action_flow_begin(value),
        proto::inner_frame::Body::ActionObservation(value) => validate_action_observation(value),
        proto::inner_frame::Body::ActionFlowUpdate(value) => validate_action_flow_update(value),
        proto::inner_frame::Body::GenericResult(value) => {
            validate_bounded(&value.payload, MAX_WATCH_PAYLOAD_LEN)
        }
        proto::inner_frame::Body::Error(value) => validate_error(value),
    }
}

fn validate_action_flow_begin(value: &proto::ActionFlowBegin) -> Result<(), CodecError> {
    validate_runtime_lease(&value.runtime_lease)?;
    ActionFlowId::try_from(value.flow_id.as_slice()).map_err(|_| CodecError::InvalidField)?;
    AccountSlotId::try_from(value.account_slot_id.as_slice())
        .map_err(|_| CodecError::InvalidField)?;
    if value.login_epoch == 0
        || value.transport_epoch == 0
        || value.expires_at_ms == 0
        || value.inputs.is_empty()
    {
        return Err(CodecError::InvalidField);
    }
    let binding = Digest32::try_from(value.binding_digest.as_slice())
        .map_err(|_| CodecError::InvalidField)?;
    validate_slots(&value.inputs)?;
    let inputs = OpaqueSlots::from_wire(&value.inputs).map_err(|_| CodecError::InvalidField)?;
    let context = ActionFlowContext {
        flow_id: ActionFlowId::try_from(value.flow_id.as_slice())
            .map_err(|_| CodecError::InvalidField)?,
        account_slot_id: AccountSlotId::try_from(value.account_slot_id.as_slice())
            .map_err(|_| CodecError::InvalidField)?,
        login_epoch: value.login_epoch,
        online_epoch: value.online_epoch,
        transport_epoch: value.transport_epoch,
        expires_at_ms: value.expires_at_ms,
    };
    if binding != action_flow_binding_digest(context, &inputs) {
        return Err(CodecError::InvalidField);
    }
    Ok(())
}

fn validate_action_observation(value: &proto::ActionObservation) -> Result<(), CodecError> {
    validate_runtime_lease(&value.runtime_lease)?;
    ActionFlowId::try_from(value.flow_id.as_slice()).map_err(|_| CodecError::InvalidField)?;
    ActionId::try_from(value.action_id.as_slice()).map_err(|_| CodecError::InvalidField)?;
    Digest32::try_from(value.action_digest.as_slice()).map_err(|_| CodecError::InvalidField)?;
    let binding = Digest32::try_from(value.binding_digest.as_slice())
        .map_err(|_| CodecError::InvalidField)?;
    let outcome = proto::ActionObservationKind::try_from(value.outcome)
        .map_err(|_| CodecError::InvalidField)?;
    if outcome == proto::ActionObservationKind::Unspecified || value.observed_at_ms == 0 {
        return Err(CodecError::InvalidField);
    }
    if (outcome == proto::ActionObservationKind::Response) == value.payload.is_empty() {
        return Err(CodecError::InvalidField);
    }
    validate_bounded(&value.payload, MAX_ACTION_PAYLOAD_LEN)?;
    let outcome = match outcome {
        proto::ActionObservationKind::Response => ActionObservationKind::Response,
        proto::ActionObservationKind::TransportFailure => ActionObservationKind::TransportFailure,
        proto::ActionObservationKind::Timeout => ActionObservationKind::Timeout,
        proto::ActionObservationKind::Cancelled => ActionObservationKind::Cancelled,
        proto::ActionObservationKind::StaleTransport => ActionObservationKind::StaleTransport,
        proto::ActionObservationKind::LocalEncodeFailure => {
            ActionObservationKind::LocalEncodeFailure
        }
        proto::ActionObservationKind::Unspecified => return Err(CodecError::InvalidField),
    };
    let observation = ActionObservation {
        flow_id: ActionFlowId::try_from(value.flow_id.as_slice())
            .map_err(|_| CodecError::InvalidField)?,
        action_id: ActionId::try_from(value.action_id.as_slice())
            .map_err(|_| CodecError::InvalidField)?,
        action_digest: Digest32::try_from(value.action_digest.as_slice())
            .map_err(|_| CodecError::InvalidField)?,
        outcome,
        payload: &value.payload,
        observed_at_ms: value.observed_at_ms,
    };
    if binding != action_observation_binding_digest(observation) {
        return Err(CodecError::InvalidField);
    }
    Ok(())
}

fn validate_action_flow_update(value: &proto::ActionFlowUpdate) -> Result<(), CodecError> {
    ActionFlowId::try_from(value.flow_id.as_slice()).map_err(|_| CodecError::InvalidField)?;
    let status =
        proto::ActionFlowStatus::try_from(value.status).map_err(|_| CodecError::InvalidField)?;
    if value.expires_at_ms == 0 {
        return Err(CodecError::InvalidField);
    }
    match status {
        proto::ActionFlowStatus::Action => {
            validate_action(value.action.as_ref().ok_or(CodecError::InvalidField)?)
        }
        proto::ActionFlowStatus::Complete if value.action.is_none() => Ok(()),
        proto::ActionFlowStatus::Complete | proto::ActionFlowStatus::Unspecified => {
            Err(CodecError::InvalidField)
        }
    }
}

fn validate_action(value: &proto::ActionDirective) -> Result<(), CodecError> {
    ActionId::try_from(value.action_id.as_slice()).map_err(|_| CodecError::InvalidField)?;
    Digest32::try_from(value.expected_body_digest.as_slice())
        .map_err(|_| CodecError::InvalidField)?;
    Digest32::try_from(value.action_digest.as_slice()).map_err(|_| CodecError::InvalidField)?;
    if value.route_shard.is_empty()
        || value.body_shard.is_empty()
        || value.envelope_contract == 0
        || value.transport_epoch == 0
        || !matches!(value.response_policy, 1 | 2)
        || value.timeout_ms == 0
        || value.timeout_ms > MAX_ACTION_TIMEOUT_MS
        || value.delay_ms > MAX_ACTION_DELAY_MS
    {
        return Err(CodecError::InvalidField);
    }
    validate_bounded(&value.route_shard, MAX_ACTION_ROUTE_LEN)?;
    validate_bounded(&value.body_shard, MAX_ACTION_BODY_LEN)?;
    validate_action_marks(&value.marks)
}

fn validate_action_marks(marks: &[proto::ActionMark]) -> Result<(), CodecError> {
    if marks.len() > MAX_ACTION_MARKS {
        return Err(CodecError::InvalidField);
    }
    let mut unique = BTreeSet::new();
    let mut aggregate = 0_usize;
    for mark in marks {
        let slot = u16::try_from(mark.slot).map_err(|_| CodecError::InvalidField)?;
        if slot == 0 || mark.value.is_empty() || !unique.insert(slot) {
            return Err(CodecError::InvalidField);
        }
        validate_bounded(&mark.value, MAX_ACTION_MARK_LEN)?;
        aggregate = aggregate
            .checked_add(mark.value.len())
            .ok_or(CodecError::LengthOverflow)?;
    }
    validate_bounded_len(aggregate, MAX_ACTION_MARK_AGGREGATE_LEN)
}

fn validate_profile_request(value: &proto::ProfileRequest) -> Result<(), CodecError> {
    validate_runtime_lease(&value.runtime_lease)?;
    ProfileId::try_from(value.profile_id.as_slice()).map_err(|_| CodecError::InvalidField)?;
    if !value.cached_manifest_digest.is_empty() {
        Digest32::try_from(value.cached_manifest_digest.as_slice())
            .map_err(|_| CodecError::InvalidField)?;
    }
    if !matches!(value.requested_access, 1 | 2) {
        return Err(CodecError::InvalidField);
    }
    crate::validate_client_runtime(value.runtime.as_ref().ok_or(CodecError::InvalidField)?)
}

fn validate_opaque_request(value: &proto::OpaqueExchangeRequest) -> Result<(), CodecError> {
    validate_runtime_lease(&value.runtime_lease)?;
    ExchangeId::try_from(value.exchange_id.as_slice()).map_err(|_| CodecError::InvalidField)?;
    AccountSlotId::try_from(value.account_slot_id.as_slice())
        .map_err(|_| CodecError::InvalidField)?;
    if value.generation == 0 || value.expires_at_ms == 0 {
        return Err(CodecError::InvalidField);
    }
    Digest32::try_from(value.binding_digest.as_slice()).map_err(|_| CodecError::InvalidField)?;
    validate_slots(&value.slots)
}

fn validate_opaque_response(value: &proto::OpaqueExchangeResponse) -> Result<(), CodecError> {
    ExchangeId::try_from(value.exchange_id.as_slice()).map_err(|_| CodecError::InvalidField)?;
    if value.generation == 0 || value.expires_at_ms == 0 {
        return Err(CodecError::InvalidField);
    }
    Digest32::try_from(value.binding_digest.as_slice()).map_err(|_| CodecError::InvalidField)?;
    validate_slots(&value.slots)
}

fn validate_slots(slots: &[proto::OpaqueSlot]) -> Result<(), CodecError> {
    if slots.is_empty() || slots.len() > MAX_OPAQUE_SLOTS {
        return Err(CodecError::InvalidField);
    }
    let mut unique = BTreeSet::new();
    let mut aggregate = 0_usize;
    for slot in slots {
        if slot.slot == 0 || !unique.insert(slot.slot) {
            return Err(CodecError::InvalidField);
        }
        validate_bounded(&slot.value, MAX_OPAQUE_SLOT_LEN)?;
        aggregate = aggregate
            .checked_add(slot.value.len())
            .ok_or(CodecError::LengthOverflow)?;
    }
    validate_bounded_len(aggregate, MAX_OPAQUE_AGGREGATE_LEN)
}

fn validate_watch_request(value: &proto::WatchRequest) -> Result<(), CodecError> {
    validate_runtime_lease(&value.runtime_lease)?;
    if value.max_wait_ms == 0 || value.max_wait_ms > MAX_WATCH_WAIT_MS {
        return Err(CodecError::InvalidField);
    }
    Ok(())
}

fn validate_watch_event(value: &proto::WatchEvent) -> Result<(), CodecError> {
    let kind = proto::WatchEventKind::try_from(value.kind).map_err(|_| CodecError::InvalidField)?;
    if value.cursor == 0
        || kind == proto::WatchEventKind::Unspecified
        || value.occurred_at_ms == 0
        || value.reason_code == 0
    {
        return Err(CodecError::InvalidField);
    }
    if !value.account_slot_id.is_empty() {
        AccountSlotId::try_from(value.account_slot_id.as_slice())
            .map_err(|_| CodecError::InvalidField)?;
    }
    validate_bounded(&value.payload, MAX_WATCH_PAYLOAD_LEN)?;
    match kind {
        proto::WatchEventKind::GrantExpiring
        | proto::WatchEventKind::RenewalPaused
        | proto::WatchEventKind::GrantRevoked
        | proto::WatchEventKind::QuotaChanged
        | proto::WatchEventKind::PolicyChanged
        | proto::WatchEventKind::GrantRestored => {
            let grant = value.grant.as_ref().ok_or(CodecError::InvalidField)?;
            validate_watch_grant(grant)?;
            validate_watch_kind_grant(kind, grant)
        }
        proto::WatchEventKind::ProfileChanged | proto::WatchEventKind::Maintenance => {
            if value.grant.is_some() {
                return Err(CodecError::InvalidField);
            }
            Ok(())
        }
        proto::WatchEventKind::Unspecified => Err(CodecError::InvalidField),
    }
}

fn validate_watch_kind_grant(
    kind: proto::WatchEventKind,
    grant: &proto::WatchGrantSnapshot,
) -> Result<(), CodecError> {
    let renewal =
        proto::RenewalState::try_from(grant.renewal_state).map_err(|_| CodecError::InvalidField)?;
    match kind {
        proto::WatchEventKind::RenewalPaused if renewal != proto::RenewalState::Paused => {
            Err(CodecError::InvalidField)
        }
        proto::WatchEventKind::GrantRevoked if renewal != proto::RenewalState::Revoked => {
            Err(CodecError::InvalidField)
        }
        proto::WatchEventKind::GrantRestored if renewal != proto::RenewalState::Current => {
            Err(CodecError::InvalidField)
        }
        proto::WatchEventKind::GrantExpiring
        | proto::WatchEventKind::QuotaChanged
        | proto::WatchEventKind::PolicyChanged
            if renewal == proto::RenewalState::Revoked =>
        {
            Err(CodecError::InvalidField)
        }
        _ => Ok(()),
    }
}

fn validate_watch_grant(value: &proto::WatchGrantSnapshot) -> Result<(), CodecError> {
    let grant =
        proto::GrantClass::try_from(value.grant_class).map_err(|_| CodecError::InvalidField)?;
    let renewal =
        proto::RenewalState::try_from(value.renewal_state).map_err(|_| CodecError::InvalidField)?;
    if grant == proto::GrantClass::Unspecified
        || renewal == proto::RenewalState::Unspecified
        || value.expires_at_ms == 0
        || value.policy_epoch == 0
    {
        return Err(CodecError::InvalidField);
    }
    match grant {
        proto::GrantClass::Public if value.max_full_accounts != 0 => Err(CodecError::InvalidField),
        proto::GrantClass::Community
            if value.max_full_accounts == 0 || value.max_active_installations == 0 =>
        {
            Err(CodecError::InvalidField)
        }
        proto::GrantClass::Public | proto::GrantClass::Community | proto::GrantClass::Full => {
            Ok(())
        }
        proto::GrantClass::Unspecified => Err(CodecError::InvalidField),
    }
}

fn validate_error(value: &proto::ErrorFrame) -> Result<(), CodecError> {
    if value.code == 0 {
        return Err(CodecError::InvalidField);
    }
    if !value.incident_id.is_empty() {
        IncidentId::try_from(value.incident_id.as_slice()).map_err(|_| CodecError::InvalidField)?;
    }
    Ok(())
}

fn validate_runtime_lease(value: &[u8]) -> Result<(), CodecError> {
    if value.is_empty() {
        return Err(CodecError::InvalidField);
    }
    validate_bounded(value, MAX_RUNTIME_LEASE_LEN)
}

fn validate_bounded(value: &[u8], limit: usize) -> Result<(), CodecError> {
    validate_bounded_len(value.len(), limit)
}

fn validate_bounded_len(actual: usize, limit: usize) -> Result<(), CodecError> {
    if actual > limit {
        Err(CodecError::LengthLimitExceeded {
            kind: crate::LengthKind::Field,
            limit,
            actual,
        })
    } else {
        Ok(())
    }
}

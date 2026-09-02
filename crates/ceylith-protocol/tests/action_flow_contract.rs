//! Canonical public action-flow framing and fail-closed validation.

use ceylith_protocol::{
    AccountSlotId, ActionFlowContext, ActionFlowId, ActionId, ActionObservation,
    ActionObservationKind, CURRENT_INNER_CONTRACT, Digest32, OpaqueSlot, OpaqueSlotId, OpaqueSlots,
    WireLimits, action_flow_binding_digest, action_observation_binding_digest, decode_inner_frame,
    encode_inner_frame, proto,
};

#[test]
fn begin_and_observation_are_canonically_bound() -> Result<(), Box<dyn std::error::Error>> {
    let inputs = inputs()?;
    let context = context();
    let begin = proto::ActionFlowBegin {
        runtime_lease: vec![7; 32],
        flow_id: context.flow_id.as_bytes().to_vec(),
        account_slot_id: context.account_slot_id.as_bytes().to_vec(),
        login_epoch: context.login_epoch,
        online_epoch: context.online_epoch,
        transport_epoch: context.transport_epoch,
        inputs: inputs.to_wire(),
        expires_at_ms: context.expires_at_ms,
        binding_digest: action_flow_binding_digest(context, &inputs)
            .as_bytes()
            .to_vec(),
    };
    round_trip(proto::inner_frame::Body::ActionFlowBegin(begin))?;

    let observation = ActionObservation {
        flow_id: context.flow_id,
        action_id: ActionId::from_bytes([4; 16]),
        action_digest: Digest32::from_bytes([5; 32]),
        outcome: ActionObservationKind::Response,
        payload: b"opaque-response",
        observed_at_ms: 1_000,
    };
    round_trip(proto::inner_frame::Body::ActionObservation(
        proto::ActionObservation {
            runtime_lease: vec![7; 32],
            flow_id: observation.flow_id.as_bytes().to_vec(),
            action_id: observation.action_id.as_bytes().to_vec(),
            action_digest: observation.action_digest.as_bytes().to_vec(),
            outcome: observation.outcome as i32,
            payload: observation.payload.to_vec(),
            observed_at_ms: observation.observed_at_ms,
            binding_digest: action_observation_binding_digest(observation)
                .as_bytes()
                .to_vec(),
        },
    ))?;
    Ok(())
}

#[test]
fn changed_input_or_response_shape_fails_closed() -> Result<(), Box<dyn std::error::Error>> {
    let inputs = inputs()?;
    let context = context();
    let mut begin = proto::ActionFlowBegin {
        runtime_lease: vec![7; 32],
        flow_id: context.flow_id.as_bytes().to_vec(),
        account_slot_id: context.account_slot_id.as_bytes().to_vec(),
        login_epoch: context.login_epoch,
        online_epoch: context.online_epoch,
        transport_epoch: context.transport_epoch,
        inputs: inputs.to_wire(),
        expires_at_ms: context.expires_at_ms,
        binding_digest: action_flow_binding_digest(context, &inputs)
            .as_bytes()
            .to_vec(),
    };
    begin.inputs[0].value[0] ^= 1;
    assert!(encode(begin_body(begin)).is_err());

    let observation = proto::ActionObservation {
        runtime_lease: vec![7; 32],
        flow_id: context.flow_id.as_bytes().to_vec(),
        action_id: vec![4; 16],
        action_digest: vec![5; 32],
        outcome: proto::ActionObservationKind::Timeout as i32,
        payload: vec![1],
        observed_at_ms: 1_000,
        binding_digest: vec![6; 32],
    };
    assert!(encode(proto::inner_frame::Body::ActionObservation(observation)).is_err());
    Ok(())
}

#[test]
fn action_and_complete_updates_have_disjoint_shapes() -> Result<(), Box<dyn std::error::Error>> {
    round_trip(proto::inner_frame::Body::ActionFlowUpdate(
        proto::ActionFlowUpdate {
            flow_id: vec![1; 16],
            status: proto::ActionFlowStatus::Action as i32,
            action: Some(proto::ActionDirective {
                action_id: vec![2; 16],
                route_shard: b"opaque-route".to_vec(),
                body_shard: vec![3; 32],
                envelope_contract: 77,
                marks: vec![proto::ActionMark {
                    slot: 1,
                    value: vec![4; 16],
                }],
                expected_body_digest: vec![5; 32],
                transport_epoch: 9,
                response_policy: 2,
                timeout_ms: 3_000,
                action_digest: vec![6; 32],
                delay_ms: 25,
            }),
            expires_at_ms: 2_000,
        },
    ))?;
    round_trip(proto::inner_frame::Body::ActionFlowUpdate(
        proto::ActionFlowUpdate {
            flow_id: vec![1; 16],
            status: proto::ActionFlowStatus::Complete as i32,
            action: None,
            expires_at_ms: 2_000,
        },
    ))?;
    Ok(())
}

fn context() -> ActionFlowContext {
    ActionFlowContext {
        flow_id: ActionFlowId::from_bytes([1; 16]),
        account_slot_id: AccountSlotId::from_bytes([2; 16]),
        login_epoch: 7,
        online_epoch: 0,
        transport_epoch: 9,
        expires_at_ms: 2_000,
    }
}

fn inputs() -> Result<OpaqueSlots, ceylith_protocol::OpaqueError> {
    OpaqueSlots::new(vec![
        OpaqueSlot::new(OpaqueSlotId::new(3101)?, vec![3; 32])?,
        OpaqueSlot::new(OpaqueSlotId::new(3102)?, vec![4; 8])?,
    ])
}

fn begin_body(begin: proto::ActionFlowBegin) -> proto::inner_frame::Body {
    proto::inner_frame::Body::ActionFlowBegin(begin)
}

fn encode(body: proto::inner_frame::Body) -> Result<Vec<u8>, ceylith_protocol::CodecError> {
    encode_inner_frame(
        &proto::InnerFrame {
            contract: CURRENT_INNER_CONTRACT,
            body: Some(body),
        },
        WireLimits::default(),
    )
}

fn round_trip(body: proto::inner_frame::Body) -> Result<(), ceylith_protocol::CodecError> {
    let encoded = encode(body)?;
    decode_inner_frame(&encoded, WireLimits::default())?;
    Ok(())
}

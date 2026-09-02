//! Public client action-flow decoding contract.

use ceylith_client::{ActionFlowUpdate, action_flow_inputs, decode_action_flow_update};
use ceylith_protocol::{ActionFlowId, CURRENT_INNER_CONTRACT, proto};

#[test]
fn update_decoder_binds_flow_and_redacts_payload_debug() -> Result<(), Box<dyn std::error::Error>> {
    let flow = ActionFlowId::from_bytes([1; 16]);
    let frame = proto::InnerFrame {
        contract: CURRENT_INNER_CONTRACT,
        body: Some(proto::inner_frame::Body::ActionFlowUpdate(
            proto::ActionFlowUpdate {
                flow_id: flow.as_bytes().to_vec(),
                status: proto::ActionFlowStatus::Action as i32,
                action: Some(proto::ActionDirective {
                    action_id: vec![2; 16],
                    route_shard: b"private-runtime-route".to_vec(),
                    body_shard: vec![3; 32],
                    envelope_contract: 77,
                    marks: Vec::new(),
                    expected_body_digest: vec![4; 32],
                    transport_epoch: 5,
                    response_policy: 2,
                    timeout_ms: 3_000,
                    action_digest: vec![6; 32],
                    delay_ms: 25,
                }),
                expires_at_ms: 2_000,
            },
        )),
    };
    let update = decode_action_flow_update(&frame, flow, 1_000)?;
    let ActionFlowUpdate::Action(action) = update else {
        return Err("action expected".into());
    };
    let debug = format!("{action:?}");
    assert!(!debug.contains("private-runtime-route"));
    assert_eq!(action.transport_epoch(), 5);
    assert!(decode_action_flow_update(&frame, ActionFlowId::from_bytes([9; 16]), 1_000).is_err());
    assert!(decode_action_flow_update(&frame, flow, 2_000).is_err());
    Ok(())
}

#[test]
fn action_inputs_are_fixed_and_bounded() -> Result<(), Box<dyn std::error::Error>> {
    let inputs = action_flow_inputs(vec![3; 8], vec![4; 16])?;
    let wire = inputs.to_wire();
    assert_eq!(
        wire.iter().map(|slot| slot.slot).collect::<Vec<_>>(),
        [3_901, 3_902]
    );
    assert_eq!(wire[0].value, [3; 8]);
    Ok(())
}

//! Bounded inner-contract parser tests.

use ceylith_protocol::{
    CURRENT_INNER_CONTRACT, CodecError, WireLimits, decode_inner_frame, encode_inner_frame, proto,
};

fn runtime() -> proto::ClientRuntime {
    proto::ClientRuntime {
        runtime_abi: 1,
        envelope_contract: 1,
        action_contracts: vec![1],
        source_contracts: vec![1],
        platform: 1,
        architecture: 1,
        build_digest: vec![3; 32],
    }
}

#[test]
fn profile_request_round_trips_under_structural_validation() -> Result<(), CodecError> {
    let frame = proto::InnerFrame {
        contract: CURRENT_INNER_CONTRACT,
        body: Some(proto::inner_frame::Body::ProfileRequest(
            proto::ProfileRequest {
                runtime_lease: vec![1, 2, 3],
                profile_id: vec![4; 16],
                cached_manifest_digest: vec![5; 32],
                requested_access: 2,
                runtime: Some(runtime()),
            },
        )),
    };
    let encoded = encode_inner_frame(&frame, WireLimits::default())?;
    assert_eq!(decode_inner_frame(&encoded, WireLimits::default())?, frame);
    Ok(())
}

#[test]
fn body_namespace_is_fail_closed_but_metadata_can_extend() {
    let valid = [0x08, 0x03, 0x82, 0x01, 0x02, 0x08, 0x01];
    let mut metadata = vec![0x10, 0x07];
    metadata.extend_from_slice(&valid);
    assert!(decode_inner_frame(&metadata, WireLimits::default()).is_ok());

    let unknown_body = [0x08, 0x03, 0xa2, 0x01, 0x00];
    assert_eq!(
        decode_inner_frame(&unknown_body, WireLimits::default()).err(),
        Some(CodecError::UnsupportedBody)
    );

    let mut duplicate = valid.to_vec();
    duplicate.extend_from_slice(&valid[2..]);
    assert_eq!(
        decode_inner_frame(&duplicate, WireLimits::default()).err(),
        Some(CodecError::UnsupportedBody)
    );
}

#[test]
fn malformed_or_missing_contracts_are_rejected() {
    let missing_contract = [0x82, 0x01, 0x00];
    assert_eq!(
        decode_inner_frame(&missing_contract, WireLimits::default()).err(),
        Some(CodecError::InvalidContract)
    );

    let malformed_length = [0x08, 0x03, 0x82, 0x01, 0x05, 0x08];
    assert!(matches!(
        decode_inner_frame(&malformed_length, WireLimits::default()),
        Err(CodecError::Truncated { .. })
    ));
}

#[test]
fn opaque_slots_must_be_unique_and_bounded() {
    let frame = proto::InnerFrame {
        contract: CURRENT_INNER_CONTRACT,
        body: Some(proto::inner_frame::Body::OpaqueExchangeResponse(
            proto::OpaqueExchangeResponse {
                exchange_id: vec![1; 16],
                generation: 1,
                slots: vec![
                    proto::OpaqueSlot {
                        slot: 7,
                        value: vec![1],
                    },
                    proto::OpaqueSlot {
                        slot: 7,
                        value: vec![2],
                    },
                ],
                expires_at_ms: 1,
                binding_digest: vec![2; 32],
            },
        )),
    };
    assert_eq!(
        encode_inner_frame(&frame, WireLimits::default()).err(),
        Some(CodecError::InvalidField)
    );
}

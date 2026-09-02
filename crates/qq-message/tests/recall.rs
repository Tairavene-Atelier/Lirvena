//! Regression vectors for bounded QQ message recall requests.

use qq_message::{
    GroupRecallInput, PrivateRecallInput, encode_group_recall, encode_private_recall,
    validate_group_recall_response, validate_private_recall_response,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn group_recall_matches_the_compiled_wire_shape() -> TestResult {
    let encoded = encode_group_recall(GroupRecallInput {
        group_uin: 123_456,
        sequence: 987_654,
    })?;
    assert_eq!(
        encoded,
        [
            0x08, 0x01, 0x10, 0xc0, 0xc4, 0x07, 0x1a, 0x04, 0x08, 0x86, 0xa4, 0x3c, 0x22, 0x00,
        ]
    );
    Ok(())
}

#[test]
fn private_recall_binds_every_original_message_correlation() -> TestResult {
    let encoded = encode_private_recall(PrivateRecallInput {
        target_uid: "u_test_peer",
        sequence: 81,
        client_sequence: 700,
        random: 0x1122_3344,
        timestamp: 1_700_000_000,
    })?;
    assert_eq!(
        encoded,
        [
            0x08, 0x01, 0x1a, 0x0b, 0x75, 0x5f, 0x74, 0x65, 0x73, 0x74, 0x5f, 0x70, 0x65, 0x65,
            0x72, 0x22, 0x1b, 0x08, 0x51, 0x10, 0xc4, 0xe6, 0x88, 0x89, 0x01, 0x18, 0xc4, 0xe6,
            0x88, 0x89, 0x81, 0x80, 0x80, 0x80, 0x01, 0x20, 0x80, 0xe2, 0xcf, 0xaa, 0x06, 0x30,
            0xbc, 0x05, 0x2a, 0x00,
        ]
    );
    Ok(())
}

#[test]
fn recall_rejects_incomplete_correlations() {
    assert!(
        encode_group_recall(GroupRecallInput {
            group_uin: 0,
            sequence: 1,
        })
        .is_err()
    );
    assert!(
        encode_private_recall(PrivateRecallInput {
            target_uid: "peer",
            sequence: 1,
            client_sequence: 0,
            random: 1,
            timestamp: 1,
        })
        .is_err()
    );
}

#[test]
fn group_recall_response_is_bound_to_the_requested_sequence() -> TestResult {
    let response = [0x22, 0x04, 0x0a, 0x02, 0x08, 0x2a];
    validate_group_recall_response(&response, 42)?;
    assert!(validate_group_recall_response(&response, 43).is_err());
    assert!(validate_group_recall_response(&[0x08, 0x03], 42).is_err());
    assert!(
        validate_group_recall_response(&[0x22, 0x06, 0x0a, 0x02, 0x08, 0x2a, 0x10, 0x05], 42,)
            .is_err()
    );
    Ok(())
}

#[test]
fn private_recall_response_is_bound_to_the_requested_client_sequence() -> TestResult {
    let response = [0x2a, 0x04, 0x1a, 0x02, 0x30, 0x2a];
    validate_private_recall_response(&response, 42)?;
    assert!(validate_private_recall_response(&response, 43).is_err());
    assert!(validate_private_recall_response(&[0x08, 0x03], 42).is_err());
    assert!(
        validate_private_recall_response(&[0x2a, 0x06, 0x08, 0x05, 0x1a, 0x02, 0x30, 0x2a], 42,)
            .is_err()
    );
    Ok(())
}

#[test]
fn recall_response_rejects_missing_and_ambiguous_items() {
    assert!(validate_group_recall_response(&[0x12, 0x00], 42).is_err());
    assert!(validate_private_recall_response(&[0x12, 0x00], 42).is_err());
    assert!(
        validate_group_recall_response(
            &[
                0x22, 0x04, 0x0a, 0x02, 0x08, 0x2a, 0x22, 0x04, 0x0a, 0x02, 0x08, 0x2a,
            ],
            42,
        )
        .is_err()
    );
    assert!(
        validate_private_recall_response(
            &[
                0x2a, 0x04, 0x1a, 0x02, 0x30, 0x2a, 0x2a, 0x04, 0x1a, 0x02, 0x30, 0x2a,
            ],
            42,
        )
        .is_err()
    );
    assert!(validate_group_recall_response(&vec![0; 64 * 1024 + 1], 42).is_err());
}

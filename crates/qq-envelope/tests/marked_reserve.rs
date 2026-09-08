//! Compiled numeric mark placement tests.

use prost::Message;
use qq_envelope::{
    EnvelopeMark, attach_account_identity, encode_account_reserve, encode_marked_reserve,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Clone, PartialEq, Message)]
struct MarkedFields {
    #[prost(bytes = "vec", tag = "1")]
    third: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    first: Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    second: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
struct ReserveFields {
    #[prost(uint32, optional, tag = "11")]
    locale: Option<u32>,
    #[prost(uint32, optional, tag = "14")]
    new_connection: Option<u32>,
    #[prost(string, optional, tag = "15")]
    correlation: Option<String>,
    #[prost(string, optional, tag = "16")]
    account_identity: Option<String>,
    #[prost(uint32, optional, tag = "18")]
    subscriber_identity: Option<u32>,
    #[prost(uint32, optional, tag = "19")]
    network: Option<u32>,
    #[prost(uint32, optional, tag = "20")]
    address_stack: Option<u32>,
    #[prost(message, optional, tag = "24")]
    marked: Option<MarkedFields>,
    #[prost(uint32, optional, tag = "26")]
    core_version: Option<u32>,
}

#[test]
fn account_reserve_matches_frozen_trace_and_identity_shape() -> TestResult {
    let encoded = encode_account_reserve("u_test", true)?;
    let decoded = ReserveFields::decode(encoded.as_slice())?;
    let trace = decoded.correlation.ok_or("missing trace")?;
    assert_eq!(trace.len(), 55);
    assert!(trace.starts_with("00-") && trace.ends_with("-01"));
    assert_eq!(decoded.account_identity.as_deref(), Some("u_test"));
    assert_eq!(decoded.locale, Some(2_052));
    assert_eq!(decoded.new_connection, Some(1));
    assert_eq!(decoded.subscriber_identity, Some(0));
    assert_eq!(decoded.network, Some(1));
    assert_eq!(decoded.address_stack, Some(1));
    assert_eq!(decoded.core_version, Some(100));
    assert!(decoded.marked.is_none());
    Ok(())
}

#[test]
fn account_identity_insertion_preserves_opaque_reserve_bytes() -> TestResult {
    let original = ReserveFields {
        locale: Some(2_052),
        new_connection: Some(1),
        correlation: Some("00-00112233445566778899aabbccddeeff-0011223344556677-01".to_owned()),
        account_identity: None,
        subscriber_identity: Some(0),
        network: Some(1),
        address_stack: Some(1),
        marked: Some(MarkedFields {
            third: vec![3; 32],
            first: b"token".to_vec(),
            second: b"extra".to_vec(),
        }),
        core_version: Some(100),
    }
    .encode_to_vec();
    let completed = attach_account_identity(&original, "u_test")?;
    let decoded = ReserveFields::decode(completed.as_slice())?;
    assert_eq!(decoded.locale, Some(2_052));
    assert_eq!(decoded.account_identity.as_deref(), Some("u_test"));
    assert_eq!(decoded.marked.ok_or("missing marks")?.third, vec![3; 32]);
    assert!(attach_account_identity(&completed, "u_test").is_err());
    Ok(())
}

#[test]
fn compiled_contract_places_numeric_marks_once() -> TestResult {
    let encoded = encode_marked_reserve(
        77,
        &[
            EnvelopeMark {
                slot: 2,
                value: b"second",
            },
            EnvelopeMark {
                slot: 3,
                value: b"third",
            },
            EnvelopeMark {
                slot: 1,
                value: b"first",
            },
        ],
        "01-00112233445566778899aabbccddeeff-0011223344556677-01",
        "account-identity",
        true,
    )?;
    let decoded = ReserveFields::decode(encoded.as_slice())?;
    let marked = decoded.marked.ok_or("missing marked fields")?;
    assert_eq!(marked.first, b"first");
    assert_eq!(marked.second, b"second");
    assert_eq!(marked.third, b"third");
    assert_eq!(
        decoded.account_identity.as_deref(),
        Some("account-identity")
    );
    let golden = encode_marked_reserve(
        77,
        &[
            EnvelopeMark {
                slot: 1,
                value: b"first",
            },
            EnvelopeMark {
                slot: 2,
                value: b"second",
            },
            EnvelopeMark {
                slot: 3,
                value: b"third",
            },
        ],
        "trace",
        "uid",
        false,
    )?;
    assert_eq!(
        golden,
        b"\x7a\x05trace\x82\x01\x03uid\xc2\x01\x16\x0a\x05third\x12\x05first\x1a\x06second"
    );
    Ok(())
}

#[test]
fn unknown_duplicate_and_incomplete_contracts_fail_closed() {
    let one = EnvelopeMark {
        slot: 1,
        value: b"one",
    };
    let two = EnvelopeMark {
        slot: 2,
        value: b"two",
    };
    let three = EnvelopeMark {
        slot: 3,
        value: b"three",
    };
    assert!(encode_marked_reserve(78, &[one, two, three], "trace", "uid", false).is_err());
    assert!(encode_marked_reserve(77, &[one, one, three], "trace", "uid", false).is_err());
    assert!(encode_marked_reserve(77, &[one, two], "trace", "uid", false).is_err());
}

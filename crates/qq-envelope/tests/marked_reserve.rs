//! Compiled numeric mark placement tests.

use prost::Message;
use qq_envelope::{EnvelopeMark, encode_marked_reserve};

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
    #[prost(string, optional, tag = "15")]
    correlation: Option<String>,
    #[prost(string, optional, tag = "16")]
    account_identity: Option<String>,
    #[prost(message, optional, tag = "24")]
    marked: Option<MarkedFields>,
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
    assert!(encode_marked_reserve(78, &[one, two, three], "trace", "uid").is_err());
    assert!(encode_marked_reserve(77, &[one, one, three], "trace", "uid").is_err());
    assert!(encode_marked_reserve(77, &[one, two], "trace", "uid").is_err());
}

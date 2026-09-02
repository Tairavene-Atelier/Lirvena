//! Shared bounded OIDB envelope contracts.

use prost::Message;
use qq_wire::{decode_oidb_request, decode_oidb_response, encode_oidb_request};

#[test]
fn shared_oidb_request_is_bounded_and_canonical() -> Result<(), Box<dyn std::error::Error>> {
    let encoded = encode_oidb_request(0x10c0, 1, &[0x08, 0x14], 0)?;
    let decoded = decode_oidb_request(&encoded)?;
    assert_eq!(decoded.command(), 0x10c0);
    assert_eq!(decoded.subcommand(), 1);
    assert_eq!(decoded.body(), [0x08, 0x14]);
    assert_eq!(decoded.reserved(), 0);
    assert!(encode_oidb_request(0, 1, &[1], 0).is_err());
    assert!(encode_oidb_request(1, 1, &[], 0).is_err());
    Ok(())
}

#[test]
fn shared_oidb_response_preserves_only_public_result_and_body()
-> Result<(), Box<dyn std::error::Error>> {
    let encoded = TestResponse {
        error_code: 7,
        body: vec![1, 2, 3],
        error_message: "rejected".to_owned(),
    }
    .encode_to_vec();
    let decoded = decode_oidb_response(&encoded)?;
    assert_eq!(decoded.error_code(), 7);
    assert_eq!(decoded.body(), [1, 2, 3]);
    Ok(())
}

#[derive(Clone, PartialEq, Message)]
struct TestResponse {
    #[prost(uint32, tag = "3")]
    error_code: u32,
    #[prost(bytes = "vec", tag = "4")]
    body: Vec<u8>,
    #[prost(string, tag = "5")]
    error_message: String,
}

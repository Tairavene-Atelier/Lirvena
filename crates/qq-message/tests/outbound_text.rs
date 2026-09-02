//! Plain-text send packet golden vectors.

use qq_message::{SendTextInput, SendTextTarget, encode_text_message, parse_send_message_response};

#[test]
fn group_text_matches_tested_protobuf_shape() -> Result<(), Box<dyn std::error::Error>> {
    let encoded = encode_text_message(&SendTextInput {
        target: SendTextTarget::Group { group_code: 42 },
        text: "hi",
        client_sequence: 7,
        random: 9,
        unix_seconds: 10,
    })?;
    assert_eq!(
        encoded,
        [
            0x0a, 0x04, 0x12, 0x02, 0x08, 0x2a, 0x12, 0x06, 0x08, 0x01, 0x10, 0x00, 0x18, 0x00,
            0x1a, 0x0a, 0x0a, 0x08, 0x12, 0x06, 0x0a, 0x04, 0x0a, 0x02, 0x68, 0x69, 0x20, 0x07,
            0x28, 0x09,
        ]
    );
    Ok(())
}

#[test]
fn private_text_contains_uid_and_control_timestamp() -> Result<(), Box<dyn std::error::Error>> {
    let encoded = encode_text_message(&SendTextInput {
        target: SendTextTarget::Private { uin: 42, uid: "u" },
        text: "hi",
        client_sequence: 7,
        random: 9,
        unix_seconds: 10,
    })?;
    assert!(
        encoded
            .windows(5)
            .any(|value| value == [0x08, 0x2a, 0x12, 0x01, 0x75])
    );
    assert!(encoded.ends_with(&[0x62, 0x02, 0x08, 0x0a]));
    Ok(())
}

#[test]
fn send_response_requires_a_success_sequence() -> Result<(), Box<dyn std::error::Error>> {
    let parsed = parse_send_message_response(&[0x08, 0x00, 0x18, 0x64, 0x58, 0x2a])?;
    assert_eq!(parsed.result, 0);
    assert_eq!(parsed.sequence, 42);
    assert_eq!(parsed.timestamp, 100);
    assert!(parse_send_message_response(&[0x08, 0x00]).is_err());
    Ok(())
}

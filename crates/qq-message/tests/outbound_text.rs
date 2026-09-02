//! Plain-text send packet golden vectors.

use prost::Message;
use qq_message::{
    OutboundSegment, SendMessageInput, SendTextInput, SendTextTarget, encode_message,
    encode_text_message, parse_send_message_response,
};

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

#[test]
fn group_mentions_and_classic_face_keep_wire_order() -> Result<(), Box<dyn std::error::Error>> {
    let segments = [
        OutboundSegment::Text("hi"),
        OutboundSegment::MentionEveryone { display: "@all" },
        OutboundSegment::Mention {
            uin: 42,
            uid: "u_target",
            display: "@target",
        },
        OutboundSegment::Face(14),
    ];
    let encoded = encode_message(&SendMessageInput {
        target: SendTextTarget::Group { group_code: 7 },
        segments: &segments,
        client_sequence: 8,
        random: 9,
        unix_seconds: 10,
    })?;
    let message = TestMessage::decode(encoded.as_slice())?;
    let elements = message
        .body
        .and_then(|body| body.rich_text)
        .ok_or("missing rich text")?
        .elements;
    assert_eq!(elements.len(), 4);
    assert_eq!(
        elements[0]
            .text
            .as_ref()
            .and_then(|text| text.value.as_deref()),
        Some("hi")
    );
    let everyone = MentionExtra::decode(
        elements[1]
            .text
            .as_ref()
            .and_then(|text| text.reserve.as_deref())
            .ok_or("missing everyone reserve")?,
    )?;
    assert_eq!((everyone.kind, everyone.uin), (Some(1), Some(0)));
    let member = MentionExtra::decode(
        elements[2]
            .text
            .as_ref()
            .and_then(|text| text.reserve.as_deref())
            .ok_or("missing member reserve")?,
    )?;
    assert_eq!(member.kind, Some(2));
    assert_eq!(member.uin, Some(42));
    assert_eq!(member.uid.as_deref(), Some("u_target"));
    assert_eq!(
        elements[3].face.as_ref().and_then(|face| face.index),
        Some(14)
    );
    Ok(())
}

#[derive(Clone, PartialEq, Message)]
struct TestMessage {
    #[prost(message, optional, tag = "3")]
    body: Option<TestBody>,
}

#[derive(Clone, PartialEq, Message)]
struct TestBody {
    #[prost(message, optional, tag = "1")]
    rich_text: Option<TestRichText>,
}

#[derive(Clone, PartialEq, Message)]
struct TestRichText {
    #[prost(message, repeated, tag = "2")]
    elements: Vec<TestElement>,
}

#[derive(Clone, PartialEq, Message)]
struct TestElement {
    #[prost(message, optional, tag = "1")]
    text: Option<TestText>,
    #[prost(message, optional, tag = "2")]
    face: Option<TestFace>,
}

#[derive(Clone, PartialEq, Message)]
struct TestText {
    #[prost(string, optional, tag = "1")]
    value: Option<String>,
    #[prost(bytes = "vec", optional, tag = "12")]
    reserve: Option<Vec<u8>>,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct TestFace {
    #[prost(int32, optional, tag = "1")]
    index: Option<i32>,
}

#[derive(Clone, PartialEq, Message)]
struct MentionExtra {
    #[prost(int32, optional, tag = "3")]
    kind: Option<i32>,
    #[prost(uint32, optional, tag = "4")]
    uin: Option<u32>,
    #[prost(string, optional, tag = "9")]
    uid: Option<String>,
}

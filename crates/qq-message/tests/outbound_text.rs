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

#[test]
fn image_uses_legacy_then_modern_elements_without_reencoding()
-> Result<(), Box<dyn std::error::Error>> {
    let segments = [OutboundSegment::Image {
        group: true,
        message_info: &[0x08, 0x01],
        compatibility: &[0x10, 0x02],
    }];
    let encoded = encode_message(&SendMessageInput {
        target: SendTextTarget::Group { group_code: 7 },
        segments: &segments,
        client_sequence: 8,
        random: 9,
        unix_seconds: 10,
    })?;
    let elements = TestMessage::decode(encoded.as_slice())?
        .body
        .and_then(|body| body.rich_text)
        .ok_or("missing rich text")?
        .elements;
    assert_eq!(elements.len(), 2);
    assert_eq!(
        elements[0].custom_face.as_deref(),
        Some([0x10, 0x02].as_slice())
    );
    let common = elements[1]
        .common
        .as_ref()
        .ok_or("missing common element")?;
    assert_eq!((common.service_type, common.business_type), (48, 20));
    assert_eq!(common.protobuf, [0x08, 0x01]);
    Ok(())
}

#[test]
fn image_without_compatibility_uses_only_modern_element() -> Result<(), Box<dyn std::error::Error>>
{
    let segments = [OutboundSegment::Image {
        group: false,
        message_info: &[0x08, 0x01],
        compatibility: &[],
    }];
    let encoded = encode_message(&SendMessageInput {
        target: SendTextTarget::Private {
            uin: 7,
            uid: "u_target",
        },
        segments: &segments,
        client_sequence: 8,
        random: 9,
        unix_seconds: 10,
    })?;
    let elements = TestMessage::decode(encoded.as_slice())?
        .body
        .and_then(|body| body.rich_text)
        .ok_or("missing rich text")?
        .elements;
    assert_eq!(elements.len(), 1);
    assert!(elements[0].not_online_image.is_none());
    let common = elements[0]
        .common
        .as_ref()
        .ok_or("missing common element")?;
    assert_eq!((common.service_type, common.business_type), (48, 10));
    Ok(())
}

#[test]
fn record_uses_only_the_modern_voice_business_type() -> Result<(), Box<dyn std::error::Error>> {
    let segments = [OutboundSegment::Record {
        group: true,
        message_info: &[0x08, 0x01],
    }];
    let encoded = encode_message(&SendMessageInput {
        target: SendTextTarget::Group { group_code: 7 },
        segments: &segments,
        client_sequence: 8,
        random: 9,
        unix_seconds: 10,
    })?;
    let elements = TestMessage::decode(encoded.as_slice())?
        .body
        .and_then(|body| body.rich_text)
        .ok_or("missing rich text")?
        .elements;
    assert_eq!(elements.len(), 1);
    let common = elements[0]
        .common
        .as_ref()
        .ok_or("missing common element")?;
    assert_eq!((common.service_type, common.business_type), (48, 22));
    assert_eq!(common.protobuf, [0x08, 0x01]);
    Ok(())
}

#[test]
fn video_preserves_legacy_material_before_modern_group_element()
-> Result<(), Box<dyn std::error::Error>> {
    let segments = [OutboundSegment::Video {
        group: true,
        message_info: &[0x08, 0x01],
        compatibility: &[0x10, 0x02],
    }];
    let encoded = encode_message(&SendMessageInput {
        target: SendTextTarget::Group { group_code: 7 },
        segments: &segments,
        client_sequence: 8,
        random: 9,
        unix_seconds: 10,
    })?;
    let elements = TestMessage::decode(encoded.as_slice())?
        .body
        .and_then(|body| body.rich_text)
        .ok_or("missing rich text")?
        .elements;
    assert_eq!(elements.len(), 2);
    assert_eq!(elements[0].video.as_deref(), Some(&[0x10, 0x02][..]));
    let common = elements[1]
        .common
        .as_ref()
        .ok_or("missing common element")?;
    assert_eq!((common.service_type, common.business_type), (48, 21));
    Ok(())
}

#[test]
fn json_xml_and_poke_use_distinct_qq_elements() -> Result<(), Box<dyn std::error::Error>> {
    let segments = [
        OutboundSegment::Json("{\"app\":\"demo\"}"),
        OutboundSegment::Xml {
            body: "<msg/>",
            service_id: 35,
        },
        OutboundSegment::Poke {
            kind: 2,
            strength: 7,
        },
    ];
    let encoded = encode_message(&SendMessageInput {
        target: SendTextTarget::Group { group_code: 7 },
        segments: &segments,
        client_sequence: 8,
        random: 9,
        unix_seconds: 10,
    })?;
    let elements = TestMessage::decode(encoded.as_slice())?
        .body
        .and_then(|body| body.rich_text)
        .ok_or("missing rich text")?
        .elements;
    assert_eq!(elements.len(), 3);
    assert_eq!(
        elements[0].light_app.as_ref().map(|app| app.data[0]),
        Some(1)
    );
    let rich = elements[1]
        .rich_message
        .as_ref()
        .ok_or("missing rich message")?;
    assert_eq!((rich.service_id, rich.template[0]), (Some(35), 1));
    let common = elements[2]
        .common
        .as_ref()
        .ok_or("missing poke common element")?;
    assert_eq!((common.service_type, common.business_type), (2, 2));
    let poke = TestPoke::decode(common.protobuf.as_slice())?;
    assert_eq!((poke.kind, poke.strength), (2, 7));
    Ok(())
}

#[test]
fn group_reply_preserves_source_evidence_and_compatibility_mention()
-> Result<(), Box<dyn std::error::Error>> {
    let original = vec![vec![0x0a, 0x02, 0x68, 0x69]];
    let segments = [OutboundSegment::Reply {
        group: true,
        sequence: 101,
        message_uid: 202,
        sender_uin: 303,
        sender_uid: "u_sender",
        timestamp: 404,
        elements: &original,
    }];
    let encoded = encode_message(&SendMessageInput {
        target: SendTextTarget::Group { group_code: 7 },
        segments: &segments,
        client_sequence: 8,
        random: 9,
        unix_seconds: 10,
    })?;
    let elements = TestMessage::decode(encoded.as_slice())?
        .body
        .and_then(|body| body.rich_text)
        .ok_or("missing rich text")?
        .elements;
    assert_eq!(elements.len(), 2);
    let source = elements[0].source.as_ref().ok_or("missing source")?;
    assert_eq!(source.sequences, [101]);
    assert_eq!(source.sender_uin, 303);
    assert_eq!(source.timestamp, Some(404));
    assert_eq!(source.elements, original);
    assert_eq!(source.to_uin, Some(0));
    let reserve =
        TestSourceReserve::decode(source.reserve.as_deref().ok_or("missing source reserve")?)?;
    assert_eq!(reserve.message_uid, 202);
    assert_eq!(reserve.sender_uid.as_deref(), Some("u_sender"));
    let mention = MentionExtra::decode(
        elements[1]
            .text
            .as_ref()
            .and_then(|text| text.reserve.as_deref())
            .ok_or("missing compatibility mention")?,
    )?;
    assert_eq!((mention.kind, mention.uin), (Some(2), Some(0)));
    assert_eq!(mention.uid.as_deref(), Some("u_sender"));
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
    #[prost(bytes = "vec", optional, tag = "4")]
    not_online_image: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "8")]
    custom_face: Option<Vec<u8>>,
    #[prost(message, optional, tag = "12")]
    rich_message: Option<TestRichMessage>,
    #[prost(bytes = "vec", optional, tag = "19")]
    video: Option<Vec<u8>>,
    #[prost(message, optional, tag = "51")]
    light_app: Option<TestLightApp>,
    #[prost(message, optional, tag = "53")]
    common: Option<TestCommon>,
    #[prost(message, optional, tag = "45")]
    source: Option<TestSource>,
}

#[derive(Clone, PartialEq, Message)]
struct TestSource {
    #[prost(uint32, repeated, tag = "1")]
    sequences: Vec<u32>,
    #[prost(uint64, tag = "2")]
    sender_uin: u64,
    #[prost(int32, optional, tag = "3")]
    timestamp: Option<i32>,
    #[prost(bytes = "vec", repeated, tag = "5")]
    elements: Vec<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "8")]
    reserve: Option<Vec<u8>>,
    #[prost(uint64, optional, tag = "10")]
    to_uin: Option<u64>,
}

#[derive(Clone, PartialEq, Message)]
struct TestSourceReserve {
    #[prost(uint64, tag = "3")]
    message_uid: u64,
    #[prost(string, optional, tag = "6")]
    sender_uid: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
struct TestRichMessage {
    #[prost(bytes = "vec", tag = "1")]
    template: Vec<u8>,
    #[prost(int32, optional, tag = "2")]
    service_id: Option<i32>,
}

#[derive(Clone, PartialEq, Message)]
struct TestLightApp {
    #[prost(bytes = "vec", tag = "1")]
    data: Vec<u8>,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct TestPoke {
    #[prost(uint32, tag = "1")]
    kind: u32,
    #[prost(uint32, tag = "7")]
    strength: u32,
}

#[derive(Clone, PartialEq, Message)]
struct TestCommon {
    #[prost(int32, tag = "1")]
    service_type: i32,
    #[prost(bytes = "vec", tag = "2")]
    protobuf: Vec<u8>,
    #[prost(uint32, tag = "3")]
    business_type: u32,
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

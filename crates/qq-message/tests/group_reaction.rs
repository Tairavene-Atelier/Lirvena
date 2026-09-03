//! Authenticated Linux NT group-reaction notice contracts.

use prost::Message;
use qq_message::{MessageDecoder, MessageDisposition, decode_group_reaction};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Clone, PartialEq, Message)]
struct PushBody {
    #[prost(message, optional, tag = "1")]
    response: Option<Response>,
    #[prost(message, optional, tag = "2")]
    content: Option<Content>,
    #[prost(message, optional, tag = "3")]
    body: Option<Body>,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct Response {
    #[prost(uint32, tag = "1")]
    from_uin: u32,
    #[prost(uint32, tag = "5")]
    to_uin: u32,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct Content {
    #[prost(uint32, tag = "1")]
    message_type: u32,
    #[prost(uint32, optional, tag = "2")]
    sub_type: Option<u32>,
    #[prost(int64, optional, tag = "6")]
    timestamp: Option<i64>,
}

#[derive(Clone, PartialEq, Message)]
struct Body {
    #[prost(bytes = "vec", optional, tag = "2")]
    content: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, Message)]
struct Notice {
    #[prost(uint32, tag = "4")]
    group_id: u32,
    #[prost(uint32, optional, tag = "13")]
    kind: Option<u32>,
    #[prost(message, optional, tag = "44")]
    reaction: Option<LevelZero>,
}

#[derive(Clone, PartialEq, Message)]
struct LevelZero {
    #[prost(message, optional, tag = "1")]
    data: Option<LevelOne>,
}

#[derive(Clone, PartialEq, Message)]
struct LevelOne {
    #[prost(message, optional, tag = "1")]
    data: Option<ReactionBody>,
}

#[derive(Clone, PartialEq, Message)]
struct ReactionBody {
    #[prost(message, optional, tag = "2")]
    target: Option<Target>,
    #[prost(message, optional, tag = "3")]
    data: Option<Data>,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct Target {
    #[prost(uint32, tag = "1")]
    sequence: u32,
}

#[derive(Clone, PartialEq, Message)]
struct Data {
    #[prost(string, tag = "1")]
    code: String,
    #[prost(uint32, tag = "3")]
    count: u32,
    #[prost(string, tag = "4")]
    operator_uid: String,
    #[prost(uint32, tag = "5")]
    kind: u32,
}

#[test]
fn exact_nested_add_and_remove_shapes_decode() -> TestResult {
    let add = envelope(88, 91, "u_operator", "14", 3, 1)?;
    let reaction = decode_group_reaction(&add)?.ok_or("missing reaction")?;
    assert_eq!(reaction.group_id(), 88);
    assert_eq!(reaction.sequence(), 91);
    assert_eq!(reaction.operator_uid(), "u_operator");
    assert!(reaction.is_add());
    assert_eq!(reaction.code(), "14");
    assert_eq!(reaction.count(), 3);
    let remove = envelope(88, 91, "12345", "10001", 0, 2)?;
    let reaction = decode_group_reaction(&remove)?.ok_or("missing reaction")?;
    assert!(!reaction.is_add());
    assert_eq!(reaction.code(), "10001");
    assert_eq!(reaction.count(), 0);
    Ok(())
}

#[test]
fn contradictory_group_and_unknown_operation_fail_closed() -> TestResult {
    let mut bytes = encoded_envelope(88, 91, "u_operator", "14", 3, 1);
    bytes[0..4].copy_from_slice(&89_u32.to_be_bytes());
    assert!(decode_group_reaction(&decode(bytes)?).is_err());
    assert!(decode_group_reaction(&envelope(88, 91, "u_operator", "14", 3, 3)?).is_err());
    Ok(())
}

fn envelope(
    group_id: u32,
    sequence: u32,
    operator_uid: &str,
    code: &str,
    count: u32,
    kind: u32,
) -> Result<qq_message::MessageEnvelope, Box<dyn std::error::Error>> {
    decode(encoded_envelope(
        group_id,
        sequence,
        operator_uid,
        code,
        count,
        kind,
    ))
}

fn encoded_envelope(
    group_id: u32,
    sequence: u32,
    operator_uid: &str,
    code: &str,
    count: u32,
    kind: u32,
) -> Vec<u8> {
    let proto = Notice {
        group_id,
        kind: Some(35),
        reaction: Some(LevelZero {
            data: Some(LevelOne {
                data: Some(ReactionBody {
                    target: Some(Target { sequence }),
                    data: Some(Data {
                        code: code.to_owned(),
                        count,
                        operator_uid: operator_uid.to_owned(),
                        kind,
                    }),
                }),
            }),
        }),
    }
    .encode_to_vec();
    let mut content = Vec::with_capacity(7 + proto.len());
    content.extend_from_slice(&group_id.to_be_bytes());
    content.push(0);
    content.extend_from_slice(&u16::try_from(proto.len()).unwrap_or_default().to_be_bytes());
    content.extend_from_slice(&proto);
    content
}

fn decode(content: Vec<u8>) -> Result<qq_message::MessageEnvelope, Box<dyn std::error::Error>> {
    let body = PushBody {
        response: Some(Response {
            from_uin: 1,
            to_uin: 2,
        }),
        content: Some(Content {
            message_type: 732,
            sub_type: Some(16),
            timestamp: Some(100),
        }),
        body: Some(Body {
            content: Some(content),
        }),
    };
    let mut decoder = MessageDecoder::default();
    let MessageDisposition::New(envelope) = decoder.decode_embedded(&body.encode_to_vec())? else {
        return Err("expected new notice".into());
    };
    Ok(*envelope)
}

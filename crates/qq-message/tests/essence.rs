//! Frozen Linux NT group essence-change contract.

use prost::Message;
use qq_message::{MessageDecoder, MessageDisposition, decode_group_essence};

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
}
#[derive(Clone, PartialEq, Message)]
struct Body {
    #[prost(bytes = "vec", optional, tag = "2")]
    content: Option<Vec<u8>>,
}
#[derive(Clone, PartialEq, Message)]
struct Notice {
    #[prost(uint32, tag = "1")]
    kind: u32,
    #[prost(message, optional, tag = "33")]
    essence: Option<Essence>,
}
#[derive(Clone, Copy, PartialEq, Message)]
struct Essence {
    #[prost(uint32, tag = "1")]
    group_id: u32,
    #[prost(uint32, tag = "2")]
    sequence: u32,
    #[prost(uint32, tag = "3")]
    random: u32,
    #[prost(uint32, tag = "4")]
    operation: u32,
    #[prost(uint32, tag = "5")]
    sender_id: u32,
    #[prost(uint32, tag = "6")]
    operator_id: u32,
}

#[test]
fn add_and_delete_shapes_decode() -> TestResult {
    let add = decode_group_essence(&envelope(88, 88, 1)?)?.ok_or("missing essence")?;
    assert!(add.is_added());
    assert_eq!(add.sequence(), 91);
    assert_eq!(add.random(), 92);
    assert_eq!(add.sender_id(), 42);
    assert_eq!(add.operator_id(), 7);
    let remove = decode_group_essence(&envelope(88, 88, 2)?)?.ok_or("missing essence")?;
    assert!(!remove.is_added());
    Ok(())
}

#[test]
fn group_mismatch_and_unknown_operation_fail_closed() -> TestResult {
    assert!(decode_group_essence(&envelope(88, 89, 1)?).is_err());
    assert!(decode_group_essence(&envelope(88, 88, 3)?).is_err());
    Ok(())
}

fn envelope(
    prefix_group: u32,
    body_group: u32,
    operation: u32,
) -> Result<qq_message::MessageEnvelope, Box<dyn std::error::Error>> {
    let proto = Notice {
        kind: 27,
        essence: Some(Essence {
            group_id: body_group,
            sequence: 91,
            random: 92,
            operation,
            sender_id: 42,
            operator_id: 7,
        }),
    }
    .encode_to_vec();
    let mut payload = Vec::with_capacity(7 + proto.len());
    payload.extend_from_slice(&prefix_group.to_be_bytes());
    payload.push(0);
    payload.extend_from_slice(&u16::try_from(proto.len())?.to_be_bytes());
    payload.extend_from_slice(&proto);
    let body = PushBody {
        response: Some(Response {
            from_uin: 1,
            to_uin: 2,
        }),
        content: Some(Content {
            message_type: 732,
            sub_type: Some(21),
        }),
        body: Some(Body {
            content: Some(payload),
        }),
    };
    let mut decoder = MessageDecoder::default();
    let MessageDisposition::New(envelope) = decoder.decode_embedded(&body.encode_to_vec())? else {
        return Err("expected new notice".into());
    };
    Ok(*envelope)
}

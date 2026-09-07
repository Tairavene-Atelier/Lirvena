//! Frozen Linux NT group-name change contract.

use prost::Message;
use qq_message::{MessageDecoder, MessageDisposition, decode_group_name_change};

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
    #[prost(uint32, tag = "4")]
    group_id: u32,
    #[prost(bytes = "vec", tag = "5")]
    event_param: Vec<u8>,
    #[prost(uint32, optional, tag = "13")]
    kind: Option<u32>,
}

#[derive(Clone, PartialEq, Message)]
struct Name {
    #[prost(string, tag = "2")]
    value: String,
}

#[test]
fn exact_group_name_shape_decodes() -> TestResult {
    let envelope = envelope(88, 88, "new name", 12)?;
    let change = decode_group_name_change(&envelope)?.ok_or("missing name change")?;
    assert_eq!(change.group_id(), 88);
    assert_eq!(change.name(), "new name");
    Ok(())
}

#[test]
fn unrelated_kind_is_ignored_and_group_mismatch_fails_closed() -> TestResult {
    assert!(decode_group_name_change(&envelope(88, 88, "name", 35)?)?.is_none());
    assert!(decode_group_name_change(&envelope(88, 89, "name", 12)?).is_err());
    Ok(())
}

fn envelope(
    prefixed_group: u32,
    body_group: u32,
    name: &str,
    kind: u32,
) -> Result<qq_message::MessageEnvelope, Box<dyn std::error::Error>> {
    let proto = Notice {
        group_id: body_group,
        event_param: Name {
            value: name.to_owned(),
        }
        .encode_to_vec(),
        kind: Some(kind),
    }
    .encode_to_vec();
    let mut payload = Vec::with_capacity(7 + proto.len());
    payload.extend_from_slice(&prefixed_group.to_be_bytes());
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
            sub_type: Some(16),
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

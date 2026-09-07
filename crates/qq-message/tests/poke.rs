//! Frozen Linux NT friend and group poke contracts.

use prost::Message;
use qq_message::{MessageDecoder, MessageDisposition, PokeScope, decode_poke_notice};

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
struct Notify {
    #[prost(message, optional, tag = "26")]
    general: Option<General>,
}
#[derive(Clone, PartialEq, Message)]
struct General {
    #[prost(uint64, tag = "1")]
    business_type: u64,
    #[prost(message, repeated, tag = "7")]
    parameters: Vec<Parameter>,
}
#[derive(Clone, PartialEq, Message)]
struct Parameter {
    #[prost(string, tag = "1")]
    name: String,
    #[prost(string, tag = "2")]
    value: String,
}

#[test]
fn group_and_friend_pokes_share_the_validated_template_shape() -> TestResult {
    let group = decode_poke_notice(&envelope(732, 20, true)?)?.ok_or("missing group poke")?;
    assert_eq!(group.scope(), PokeScope::Group(88));
    assert_eq!(group.operator_id(), 42);
    assert_eq!(group.target_id(), 7);
    assert_eq!(group.action(), "poke");
    let friend = decode_poke_notice(&envelope(528, 290, false)?)?.ok_or("missing friend poke")?;
    assert_eq!(friend.scope(), PokeScope::Friend);
    assert_eq!(friend.image_url(), "https://example.invalid/poke.png");
    Ok(())
}

#[test]
fn non_numeric_identity_fails_closed() -> TestResult {
    let mut general = general();
    general.parameters[0].value = "u_operator".to_owned();
    let envelope = decoded(528, 290, general.encode_to_vec())?;
    assert!(decode_poke_notice(&envelope).is_err());
    Ok(())
}

fn envelope(
    message_type: u32,
    sub_type: u32,
    framed: bool,
) -> Result<qq_message::MessageEnvelope, Box<dyn std::error::Error>> {
    let general = general();
    let payload = if framed {
        let proto = Notify {
            general: Some(general),
        }
        .encode_to_vec();
        let mut value = Vec::with_capacity(7 + proto.len());
        value.extend_from_slice(&88_u32.to_be_bytes());
        value.push(0);
        value.extend_from_slice(&u16::try_from(proto.len())?.to_be_bytes());
        value.extend_from_slice(&proto);
        value
    } else {
        general.encode_to_vec()
    };
    decoded(message_type, sub_type, payload)
}

fn general() -> General {
    General {
        business_type: 12,
        parameters: [
            ("uin_str1", "42"),
            ("uin_str2", "7"),
            ("action_str", "poke"),
            ("suffix_str", "once"),
            ("action_img_url", "https://example.invalid/poke.png"),
        ]
        .into_iter()
        .map(|(name, value)| Parameter {
            name: name.to_owned(),
            value: value.to_owned(),
        })
        .collect(),
    }
}

fn decoded(
    message_type: u32,
    sub_type: u32,
    payload: Vec<u8>,
) -> Result<qq_message::MessageEnvelope, Box<dyn std::error::Error>> {
    let body = PushBody {
        response: Some(Response {
            from_uin: 1,
            to_uin: 2,
        }),
        content: Some(Content {
            message_type,
            sub_type: Some(sub_type),
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

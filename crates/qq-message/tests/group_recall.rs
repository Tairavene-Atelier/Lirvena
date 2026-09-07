//! Authenticated group recall codec contracts.

use prost::Message;
use qq_message::{MessageDecoder, MessageDisposition, decode_group_recalls};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn decodes_bounded_group_recall_correlations() -> TestResult {
    let envelope = envelope(88, 91)?;
    let recalls = decode_group_recalls(&envelope)?.ok_or("missing recalls")?;
    assert_eq!(recalls.len(), 1);
    assert_eq!(recalls[0].group_id(), 88);
    assert_eq!(recalls[0].sequence(), 91);
    assert_eq!(recalls[0].random(), 73);
    assert_eq!(recalls[0].author_uid(), "u_author");
    assert_eq!(recalls[0].operator_uid(), Some("u_operator"));
    assert_eq!(recalls[0].tip(), "recalled");
    Ok(())
}

#[test]
fn rejects_mismatched_prefix_and_zero_correlation() -> TestResult {
    assert!(decode_group_recalls(&envelope(89, 91)?).is_err());
    assert!(decode_group_recalls(&envelope(88, 0)?).is_err());
    Ok(())
}

fn envelope(
    prefix: u32,
    sequence: u32,
) -> Result<qq_message::MessageEnvelope, Box<dyn std::error::Error>> {
    let notice = Notice {
        group_id: 88,
        recall: Some(Recall {
            operator_uid: Some("u_operator".to_owned()),
            messages: vec![RecallMessage {
                sequence,
                random: 73,
                author_uid: "u_author".to_owned(),
            }],
            tip: Some(Tip {
                tip: Some("recalled".to_owned()),
            }),
        }),
    }
    .encode_to_vec();
    let mut framed = Vec::with_capacity(notice.len() + 7);
    framed.extend(prefix.to_be_bytes());
    framed.push(0);
    framed.extend(u16::try_from(notice.len())?.to_be_bytes());
    framed.extend(notice);
    let body = PushBody {
        response: Some(Response {
            from_uin: 1,
            to_uin: 2,
        }),
        content: Some(Content {
            message_type: 732,
            sub_type: 17,
            sequence: Some(7),
            timestamp: Some(1_800_000_000),
        }),
        body: Some(Body {
            content: Some(framed),
        }),
    };
    let mut decoder = MessageDecoder::default();
    let MessageDisposition::New(envelope) = decoder.decode_embedded(&body.encode_to_vec())? else {
        return Err("expected new notice".into());
    };
    Ok(*envelope)
}

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
    #[prost(uint32, tag = "2")]
    sub_type: u32,
    #[prost(uint64, optional, tag = "5")]
    sequence: Option<u64>,
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
    #[prost(message, optional, tag = "11")]
    recall: Option<Recall>,
}
#[derive(Clone, PartialEq, Message)]
struct Recall {
    #[prost(string, optional, tag = "1")]
    operator_uid: Option<String>,
    #[prost(message, repeated, tag = "3")]
    messages: Vec<RecallMessage>,
    #[prost(message, optional, tag = "9")]
    tip: Option<Tip>,
}
#[derive(Clone, PartialEq, Message)]
struct RecallMessage {
    #[prost(uint32, tag = "1")]
    sequence: u32,
    #[prost(uint32, tag = "3")]
    random: u32,
    #[prost(string, tag = "6")]
    author_uid: String,
}
#[derive(Clone, PartialEq, Message)]
struct Tip {
    #[prost(string, optional, tag = "2")]
    tip: Option<String>,
}

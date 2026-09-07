//! Authenticated friend recall codec contracts.

use prost::Message;
use qq_message::{MessageDecoder, MessageDisposition, decode_friend_recall};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn decodes_complete_friend_recall() -> TestResult {
    let content = Recall {
        info: Some(Info {
            from_uid: "u_friend".to_owned(),
            client_sequence: 71,
            timestamp: 72,
            random: 73,
            tip: Some(Tip {
                value: Some("recalled".to_owned()),
            }),
        }),
    }
    .encode_to_vec();
    let recall = decode_friend_recall(&envelope(content)?)?.ok_or("missing recall")?;
    assert_eq!(recall.from_uid(), "u_friend");
    assert_eq!(recall.client_sequence(), 71);
    assert_eq!(recall.timestamp(), 72);
    assert_eq!(recall.random(), 73);
    assert_eq!(recall.tip(), "recalled");
    Ok(())
}

#[test]
fn rejects_missing_correlation() -> TestResult {
    let content = Recall {
        info: Some(Info {
            from_uid: "u_friend".to_owned(),
            client_sequence: 0,
            timestamp: 72,
            random: 73,
            tip: None,
        }),
    }
    .encode_to_vec();
    assert!(decode_friend_recall(&envelope(content)?).is_err());
    Ok(())
}

fn envelope(content: Vec<u8>) -> Result<qq_message::MessageEnvelope, Box<dyn std::error::Error>> {
    let body = PushBody {
        response: Some(Response {
            from_uin: 1,
            to_uin: 2,
        }),
        content: Some(Content {
            message_type: 528,
            sub_type: 138,
            sequence: Some(7),
            timestamp: Some(1_800_000_000),
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
struct Recall {
    #[prost(message, optional, tag = "1")]
    info: Option<Info>,
}
#[derive(Clone, PartialEq, Message)]
struct Info {
    #[prost(string, tag = "1")]
    from_uid: String,
    #[prost(uint32, tag = "3")]
    client_sequence: u32,
    #[prost(uint32, tag = "5")]
    timestamp: u32,
    #[prost(uint32, tag = "6")]
    random: u32,
    #[prost(message, optional, tag = "13")]
    tip: Option<Tip>,
}
#[derive(Clone, PartialEq, Message)]
struct Tip {
    #[prost(string, optional, tag = "2")]
    value: Option<String>,
}

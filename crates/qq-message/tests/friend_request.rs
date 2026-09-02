//! Authenticated friend-request Push codec contracts.

use prost::Message;
use qq_message::{MessageDecoder, MessageDisposition, decode_friend_request_signal};

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
    #[prost(uint64, optional, tag = "5")]
    sequence: Option<u64>,
}

#[derive(Clone, PartialEq, Message)]
struct Body {
    #[prost(bytes = "vec", optional, tag = "2")]
    content: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, Message)]
struct FriendRequest {
    #[prost(message, optional, tag = "1")]
    info: Option<FriendRequestInfo>,
}

#[derive(Clone, PartialEq, Message)]
struct FriendRequestInfo {
    #[prost(string, tag = "1")]
    target_uid: String,
    #[prost(string, tag = "2")]
    source_uid: String,
    #[prost(string, tag = "10")]
    comment: String,
    #[prost(string, tag = "11")]
    source: String,
}

#[test]
fn only_evidence_backed_subtype_emits_friend_request_signal() -> TestResult {
    let request = FriendRequest {
        info: Some(FriendRequestInfo {
            target_uid: "u_self".to_owned(),
            source_uid: "u_friend".to_owned(),
            comment: "hello".to_owned(),
            source: "search".to_owned(),
        }),
    }
    .encode_to_vec();
    let accepted = envelope(35, request.clone())?;
    let signal = decode_friend_request_signal(&accepted)?.ok_or("missing friend request")?;
    assert_eq!(signal.source_uid(), "u_friend");

    let unrelated = envelope(38, request)?;
    assert!(decode_friend_request_signal(&unrelated)?.is_none());
    Ok(())
}

fn envelope(
    sub_type: u32,
    content: Vec<u8>,
) -> Result<qq_message::MessageEnvelope, Box<dyn std::error::Error>> {
    let body = PushBody {
        response: Some(Response {
            from_uin: 42,
            to_uin: 10001,
        }),
        content: Some(Content {
            message_type: 528,
            sub_type: Some(sub_type),
            sequence: Some(7),
        }),
        body: Some(Body {
            content: Some(content),
        }),
    };
    let mut decoder = MessageDecoder::default();
    let MessageDisposition::New(envelope) = decoder.decode_embedded(&body.encode_to_vec())? else {
        return Err("expected new request".into());
    };
    Ok(*envelope)
}

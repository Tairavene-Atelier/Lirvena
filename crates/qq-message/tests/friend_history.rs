//! Bounded Linux NT direct-message history contracts.

use prost::Message;
use qq_message::{decode_friend_history_response, encode_friend_history_request};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Clone, PartialEq, Message)]
struct Request {
    #[prost(string, tag = "1")]
    uid: String,
    #[prost(uint32, tag = "2")]
    timestamp: u32,
    #[prost(uint32, tag = "3")]
    random: u32,
    #[prost(uint32, tag = "4")]
    count: u32,
    #[prost(uint32, tag = "5")]
    direction: u32,
}

#[derive(Clone, PartialEq, Message)]
struct Response {
    #[prost(string, tag = "3")]
    uid: String,
    #[prost(bool, tag = "4")]
    complete: bool,
    #[prost(uint32, tag = "5")]
    timestamp: u32,
    #[prost(uint32, tag = "6")]
    random: u32,
    #[prost(message, repeated, tag = "7")]
    messages: Vec<PushBody>,
}

#[derive(Clone, PartialEq, Message)]
struct PushBody {
    #[prost(message, optional, tag = "1")]
    response: Option<PushResponse>,
    #[prost(message, optional, tag = "2")]
    content: Option<PushContent>,
    #[prost(message, optional, tag = "3")]
    body: Option<MessageBody>,
}

#[derive(Clone, PartialEq, Message)]
struct PushResponse {
    #[prost(uint32, tag = "1")]
    from_uin: u32,
    #[prost(string, optional, tag = "2")]
    from_uid: Option<String>,
    #[prost(uint32, tag = "5")]
    to_uin: u32,
    #[prost(string, optional, tag = "6")]
    to_uid: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct PushContent {
    #[prost(uint32, tag = "1")]
    message_type: u32,
    #[prost(uint64, optional, tag = "5")]
    sequence: Option<u64>,
    #[prost(int64, optional, tag = "6")]
    timestamp: Option<i64>,
    #[prost(uint32, optional, tag = "11")]
    direct_sequence: Option<u32>,
}

#[derive(Clone, PartialEq, Message)]
struct MessageBody {
    #[prost(bytes = "vec", optional, tag = "1")]
    rich_text: Option<Vec<u8>>,
}

#[test]
fn request_and_both_message_directions_match_frozen_shape() -> TestResult {
    let request = Request::decode(encode_friend_history_request("u_peer", 100, 20)?.as_slice())?;
    assert_eq!(
        request,
        Request {
            uid: "u_peer".to_owned(),
            timestamp: 100,
            random: 0,
            count: 20,
            direction: 2,
        }
    );
    let response = Response {
        uid: "u_peer".to_owned(),
        complete: false,
        timestamp: 80,
        random: 0,
        messages: vec![message(42, 10_001, 99), message(10_001, 42, 100)],
    }
    .encode_to_vec();
    let messages = decode_friend_history_response(&response, "u_peer", 42, 10_001, 100, 20)?;
    assert_eq!(messages.len(), 2);
    Ok(())
}

#[test]
fn changed_peer_excess_and_future_messages_fail_closed() {
    assert!(encode_friend_history_request("", 100, 20).is_err());
    assert!(encode_friend_history_request("u_peer", 0, 20).is_err());
    assert!(encode_friend_history_request("u_peer", 100, 101).is_err());
    let changed = response("u_other", vec![]);
    assert!(decode_friend_history_response(&changed, "u_peer", 42, 10_001, 100, 20).is_err());
    let future = response("u_peer", vec![message(42, 10_001, 101)]);
    assert!(decode_friend_history_response(&future, "u_peer", 42, 10_001, 100, 20).is_err());
}

fn response(uid: &str, messages: Vec<PushBody>) -> Vec<u8> {
    Response {
        uid: uid.to_owned(),
        complete: false,
        timestamp: 80,
        random: 0,
        messages,
    }
    .encode_to_vec()
}

fn message(from_uin: u32, to_uin: u32, timestamp: i64) -> PushBody {
    PushBody {
        response: Some(PushResponse {
            from_uin,
            from_uid: Some(if from_uin == 42 { "u_peer" } else { "u_self" }.to_owned()),
            to_uin,
            to_uid: Some(if to_uin == 42 { "u_peer" } else { "u_self" }.to_owned()),
        }),
        content: Some(PushContent {
            message_type: 166,
            sequence: Some(u64::try_from(timestamp).unwrap_or_default()),
            timestamp: Some(timestamp),
            direct_sequence: Some(u32::try_from(timestamp).unwrap_or_default()),
        }),
        body: Some(MessageBody { rich_text: None }),
    }
}

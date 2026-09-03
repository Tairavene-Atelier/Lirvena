//! Bounded Linux NT group-history request and response contracts.

use prost::Message;
use qq_message::{decode_group_history_response, encode_group_history_request};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Clone, PartialEq, Message)]
struct Request {
    #[prost(message, optional, tag = "1")]
    info: Option<Info>,
    #[prost(bool, tag = "2")]
    backwards: bool,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct Info {
    #[prost(uint32, tag = "1")]
    group_id: u32,
    #[prost(uint32, tag = "2")]
    start: u32,
    #[prost(uint32, tag = "3")]
    end: u32,
}

#[derive(Clone, PartialEq, Message)]
struct ResponseEnvelope {
    #[prost(message, optional, tag = "3")]
    body: Option<ResponseBody>,
}

#[derive(Clone, PartialEq, Message)]
struct ResponseBody {
    #[prost(uint32, tag = "1")]
    result: u32,
    #[prost(string, tag = "2")]
    message: String,
    #[prost(uint32, tag = "3")]
    group_id: u32,
    #[prost(uint32, tag = "4")]
    start: u32,
    #[prost(uint32, tag = "5")]
    end: u32,
    #[prost(message, repeated, tag = "6")]
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
    #[prost(message, optional, tag = "8")]
    group: Option<GroupRoute>,
}

#[derive(Clone, PartialEq, Message)]
struct GroupRoute {
    #[prost(uint32, tag = "1")]
    group_uin: u32,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct PushContent {
    #[prost(uint32, tag = "1")]
    message_type: u32,
    #[prost(uint64, optional, tag = "5")]
    sequence: Option<u64>,
    #[prost(int64, optional, tag = "6")]
    timestamp: Option<i64>,
}

#[derive(Clone, PartialEq, Message)]
struct MessageBody {
    #[prost(bytes = "vec", optional, tag = "1")]
    rich_text: Option<Vec<u8>>,
}

#[test]
fn request_and_embedded_response_match_frozen_shape() -> TestResult {
    let request = Request::decode(encode_group_history_request(88, 81, 100)?.as_slice())?;
    assert_eq!(
        request,
        Request {
            info: Some(Info {
                group_id: 88,
                start: 81,
                end: 100,
            }),
            backwards: true,
        }
    );

    let response = response(88, 81, 100, vec![message(88, 99), message(88, 100)]);
    let messages = decode_group_history_response(&response, 88, 81, 100)?;
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].envelope().sequence(), 99);
    assert_eq!(messages[1].envelope().sequence(), 100);
    assert!(messages.iter().all(|message| message.rich_text().is_none()));
    Ok(())
}

#[test]
fn bounds_and_response_correlations_fail_closed() {
    assert!(encode_group_history_request(0, 1, 1).is_err());
    assert!(encode_group_history_request(88, 2, 1).is_err());
    assert!(encode_group_history_request(88, 1, 101).is_err());
    assert!(encode_group_history_request(88, 0, 100).is_ok());
    assert!(encode_group_history_request(88, 0, 101).is_err());
    let wrong_group = response(89, 81, 100, vec![]);
    assert!(decode_group_history_response(&wrong_group, 88, 81, 100).is_err());
    let wrong_message = response(88, 81, 100, vec![message(89, 99)]);
    assert!(decode_group_history_response(&wrong_message, 88, 81, 100).is_err());
}

fn response(group_id: u32, start: u32, end: u32, messages: Vec<PushBody>) -> Vec<u8> {
    ResponseEnvelope {
        body: Some(ResponseBody {
            result: 0,
            message: String::new(),
            group_id,
            start,
            end,
            messages,
        }),
    }
    .encode_to_vec()
}

fn message(group_id: u32, sequence: u64) -> PushBody {
    PushBody {
        response: Some(PushResponse {
            from_uin: 42,
            from_uid: Some("u_sender".to_owned()),
            to_uin: 43,
            group: Some(GroupRoute {
                group_uin: group_id,
            }),
        }),
        content: Some(PushContent {
            message_type: 82,
            sequence: Some(sequence),
            timestamp: Some(1_800_000_000),
        }),
        body: Some(MessageBody { rich_text: None }),
    }
}

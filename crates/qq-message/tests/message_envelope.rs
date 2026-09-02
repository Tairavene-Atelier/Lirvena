//! Bounded outer message envelope and deduplication contracts.

use prost::Message;
use qq_message::{MessageClass, MessageDecoder, MessageDisposition};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Clone, PartialEq, Message)]
struct Push {
    #[prost(message, optional, tag = "1")]
    message: Option<PushBody>,
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

#[derive(Clone, PartialEq, Message)]
struct Response {
    #[prost(uint32, tag = "1")]
    from_uin: u32,
    #[prost(string, optional, tag = "2")]
    from_uid: Option<String>,
    #[prost(uint32, tag = "5")]
    to_uin: u32,
    #[prost(string, optional, tag = "6")]
    to_uid: Option<String>,
    #[prost(message, optional, tag = "8")]
    group: Option<Route>,
}

#[derive(Clone, PartialEq, Message)]
struct Route {
    #[prost(uint32, tag = "1")]
    group_uin: u32,
    #[prost(string, tag = "4")]
    member_name: String,
    #[prost(string, tag = "7")]
    group_name: String,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct Content {
    #[prost(uint32, tag = "1")]
    message_type: u32,
    #[prost(uint32, optional, tag = "2")]
    sub_type: Option<u32>,
    #[prost(int64, optional, tag = "4")]
    random: Option<i64>,
    #[prost(uint64, optional, tag = "5")]
    sequence: Option<u64>,
    #[prost(int64, optional, tag = "6")]
    timestamp: Option<i64>,
    #[prost(uint32, optional, tag = "8")]
    package_index: Option<u32>,
}

#[derive(Clone, PartialEq, Message)]
struct Body {
    #[prost(bytes = "vec", optional, tag = "1")]
    rich_text: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "2")]
    content: Option<Vec<u8>>,
}

#[test]
fn outer_and_embedded_packets_share_one_dedup_window() -> TestResult {
    let body = fixture(82, 7, b"content");
    let outer = Push {
        message: Some(body.clone()),
    }
    .encode_to_vec();
    let mut decoder = MessageDecoder::default();
    let MessageDisposition::New(message) = decoder.decode(&outer)? else {
        return Err("expected new message".into());
    };
    assert_eq!(message.class(), MessageClass::Group);
    assert_eq!(message.route().from_uin, 42);
    assert_eq!(message.route().group_uin, Some(88));
    assert_eq!(message.route().member_name.as_deref(), Some("member"));
    assert_eq!(message.sequence(), 7);
    assert_eq!(message.random(), -9);
    assert_eq!(message.timestamp(), 1_800_000_000);
    assert_eq!(message.payload().rich_text(), Some([1, 2, 3].as_slice()));
    assert_eq!(message.payload().content(), Some(b"content".as_slice()));
    assert_eq!(
        decoder.decode_embedded(&body.encode_to_vec())?,
        MessageDisposition::Duplicate
    );
    assert_eq!(decoder.retained_dedup_entries(), 1);
    Ok(())
}

#[test]
fn content_digest_and_sequence_both_participate_in_deduplication() -> TestResult {
    let mut decoder = MessageDecoder::default();
    for body in [
        fixture(166, 1, b"a"),
        fixture(166, 1, b"b"),
        fixture(166, 2, b"b"),
    ] {
        assert!(matches!(
            decoder.decode_embedded(&body.encode_to_vec())?,
            MessageDisposition::New(_)
        ));
    }
    assert_eq!(decoder.retained_dedup_entries(), 3);
    Ok(())
}

#[test]
fn dedup_window_is_fifo_and_bounded() -> TestResult {
    let mut decoder = MessageDecoder::default();
    for sequence in 1..=2_049 {
        let body = fixture(166, sequence, b"same").encode_to_vec();
        assert!(matches!(
            decoder.decode_embedded(&body)?,
            MessageDisposition::New(_)
        ));
    }
    assert_eq!(decoder.retained_dedup_entries(), 2_048);
    assert!(matches!(
        decoder.decode_embedded(&fixture(166, 1, b"same").encode_to_vec())?,
        MessageDisposition::New(_)
    ));
    Ok(())
}

#[test]
fn malformed_incomplete_and_unsafe_packets_fail_closed() {
    let mut decoder = MessageDecoder::default();
    assert!(decoder.decode(&[]).is_err());
    assert!(decoder.decode_embedded(&[0xff]).is_err());
    assert!(
        decoder
            .decode(&Push { message: None }.encode_to_vec())
            .is_err()
    );
    let mut unsafe_body = fixture(82, 1, b"content");
    unsafe_body.response = Some(Response {
        from_uin: 42,
        from_uid: Some("u_source".to_owned()),
        to_uin: 43,
        to_uid: Some("u_target".to_owned()),
        group: Some(Route {
            group_uin: 88,
            member_name: "bad\0name".to_owned(),
            group_name: "group".to_owned(),
        }),
    });
    assert!(
        decoder
            .decode_embedded(&unsafe_body.encode_to_vec())
            .is_err()
    );
    assert!(decoder.decode(&vec![0; 1024 * 1024 + 1]).is_err());
}

fn fixture(message_type: u32, sequence: u64, body: &[u8]) -> PushBody {
    PushBody {
        response: Some(Response {
            from_uin: 42,
            from_uid: Some("u_source".to_owned()),
            to_uin: 43,
            to_uid: Some("u_target".to_owned()),
            group: Some(Route {
                group_uin: 88,
                member_name: "member".to_owned(),
                group_name: "group".to_owned(),
            }),
        }),
        content: Some(Content {
            message_type,
            sub_type: Some(3),
            random: Some(-9),
            sequence: Some(sequence),
            timestamp: Some(1_800_000_000),
            package_index: Some(0),
        }),
        body: Some(Body {
            rich_text: Some(vec![1, 2, 3]),
            content: Some(body.to_vec()),
        }),
    }
}

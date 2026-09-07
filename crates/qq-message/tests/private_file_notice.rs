//! Frozen Linux NT private-file notice contract.

use prost::Message;
use qq_message::{MessageDecoder, MessageDisposition, decode_private_file_notice};

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
#[derive(Clone, PartialEq, Message)]
struct Response {
    #[prost(uint32, tag = "1")]
    from_uin: u32,
    #[prost(string, optional, tag = "2")]
    from_uid: Option<String>,
    #[prost(uint32, tag = "5")]
    to_uin: u32,
}
#[derive(Clone, Copy, PartialEq, Message)]
struct Content {
    #[prost(uint32, tag = "1")]
    message_type: u32,
}
#[derive(Clone, PartialEq, Message)]
struct Body {
    #[prost(bytes = "vec", optional, tag = "2")]
    content: Option<Vec<u8>>,
}
#[derive(Clone, PartialEq, Message)]
struct Extra {
    #[prost(message, optional, tag = "1")]
    file: Option<File>,
}
#[derive(Clone, PartialEq, Message)]
struct File {
    #[prost(string, tag = "3")]
    id: String,
    #[prost(bytes = "vec", tag = "4")]
    md5: Vec<u8>,
    #[prost(string, tag = "5")]
    name: String,
    #[prost(int64, tag = "6")]
    size: i64,
    #[prost(string, tag = "57")]
    hash: String,
}

#[test]
fn exact_private_file_shape_decodes() -> TestResult {
    let notice = decode_private_file_notice(&envelope(42, "u_sender", 12)?)?
        .ok_or("missing private file")?;
    assert_eq!(notice.sender_id(), 42);
    assert_eq!(notice.sender_uid(), "u_sender");
    assert_eq!(notice.file_id(), "file-id");
    assert_eq!(notice.name(), "report.txt");
    assert_eq!(notice.size(), 12);
    assert_eq!(notice.hash(), "file-hash");
    Ok(())
}

#[test]
fn missing_sender_and_negative_size_fail_closed() -> TestResult {
    assert!(decode_private_file_notice(&envelope(0, "u_sender", 12)?).is_err());
    assert!(decode_private_file_notice(&envelope(42, "u_sender", -1)?).is_err());
    Ok(())
}

fn envelope(
    sender: u32,
    uid: &str,
    size: i64,
) -> Result<qq_message::MessageEnvelope, Box<dyn std::error::Error>> {
    let body = PushBody {
        response: Some(Response {
            from_uin: sender,
            from_uid: Some(uid.to_owned()),
            to_uin: 10_001,
        }),
        content: Some(Content { message_type: 529 }),
        body: Some(Body {
            content: Some(
                Extra {
                    file: Some(File {
                        id: "file-id".to_owned(),
                        md5: vec![1; 16],
                        name: "report.txt".to_owned(),
                        size,
                        hash: "file-hash".to_owned(),
                    }),
                }
                .encode_to_vec(),
            ),
        }),
    };
    let mut decoder = MessageDecoder::default();
    let MessageDisposition::New(envelope) = decoder.decode_embedded(&body.encode_to_vec())? else {
        return Err("expected new file".into());
    };
    Ok(*envelope)
}

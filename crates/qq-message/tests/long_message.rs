//! Linux NT long-message codec contracts.

use std::io::{Read, Write};

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use prost::Message;
use qq_message::{
    LongMessageTarget, encode_long_message_receive, encode_long_message_send,
    parse_long_message_receive, parse_long_message_send,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn linux_receive_request_and_bounded_response_round_trip() -> TestResult {
    let request =
        RequestFixture::decode(encode_long_message_receive("u_self", "res-42")?.as_slice())?;
    let receive = request.receive.ok_or("missing receive request")?;
    assert_eq!(
        (
            receive.peer.and_then(|peer| peer.uid),
            receive.resource_id,
            receive.message_type
        ),
        (Some("u_self".to_owned()), "res-42".to_owned(), 3)
    );
    assert_eq!(
        request.attributes.map(|value| (
            value.subcommand,
            value.client_type,
            value.platform,
            value.proxy_type
        )),
        Some((2, Some(0), Some(0), Some(0)))
    );

    let messages = vec![vec![0x0a, 0x02, 0x08, 0x01], vec![0x12, 0x00]];
    let payload = gzip(
        &TransmitFixture {
            messages: Vec::new(),
            items: vec![ItemFixture {
                file_name: "MultiMsg".to_owned(),
                buffer: Some(BufferFixture {
                    messages: messages.clone(),
                }),
            }],
        }
        .encode_to_vec(),
    )?;
    let response = ResponseFixture {
        receive: Some(ReceiveResponseFixture { payload }),
        send: None,
    }
    .encode_to_vec();
    assert_eq!(parse_long_message_receive(&response)?, messages);
    Ok(())
}

#[test]
fn linux_group_send_uses_evidence_pinned_attributes_and_gzip() -> TestResult {
    let messages = vec![vec![0x0a, 0x02, 0x08, 0x01]];
    let request = RequestFixture::decode(
        encode_long_message_send(&LongMessageTarget::Group { group_uin: 42 }, &messages)?
            .as_slice(),
    )?;
    let send = request.send.ok_or("missing send request")?;
    assert_eq!(
        (
            send.message_type,
            send.peer.and_then(|peer| peer.uid),
            send.group_uin
        ),
        (3, Some("42".to_owned()), 42)
    );
    let mut decoder = GzDecoder::new(send.payload.as_slice());
    let mut expanded = Vec::new();
    decoder.read_to_end(&mut expanded)?;
    let transmit = TransmitFixture::decode(expanded.as_slice())?;
    assert_eq!(
        transmit.items[0]
            .buffer
            .as_ref()
            .ok_or("missing buffer")?
            .messages,
        messages
    );
    assert_eq!(
        request.attributes.map(|value| (
            value.subcommand,
            value.client_type,
            value.platform,
            value.proxy_type
        )),
        Some((4, Some(1), Some(6), Some(0)))
    );
    Ok(())
}

#[test]
fn responses_fail_closed_for_ambiguity_and_missing_resource_id() -> TestResult {
    let duplicate = gzip(
        &TransmitFixture {
            messages: Vec::new(),
            items: vec![
                ItemFixture {
                    file_name: "MultiMsg".to_owned(),
                    buffer: Some(BufferFixture {
                        messages: vec![vec![1]],
                    }),
                },
                ItemFixture {
                    file_name: "MultiMsg".to_owned(),
                    buffer: Some(BufferFixture {
                        messages: vec![vec![2]],
                    }),
                },
            ],
        }
        .encode_to_vec(),
    )?;
    let response = ResponseFixture {
        receive: Some(ReceiveResponseFixture { payload: duplicate }),
        send: None,
    }
    .encode_to_vec();
    assert!(parse_long_message_receive(&response).is_err());
    let missing = ResponseFixture {
        receive: None,
        send: Some(SendResponseFixture {
            resource_id: String::new(),
        }),
    }
    .encode_to_vec();
    assert!(parse_long_message_send(&missing).is_err());
    let whitespace = ResponseFixture {
        receive: None,
        send: Some(SendResponseFixture {
            resource_id: "   ".to_owned(),
        }),
    }
    .encode_to_vec();
    assert!(parse_long_message_send(&whitespace).is_err());
    let valid = ResponseFixture {
        receive: None,
        send: Some(SendResponseFixture {
            resource_id: "res-7".to_owned(),
        }),
    }
    .encode_to_vec();
    assert_eq!(parse_long_message_send(&valid)?, "res-7");
    Ok(())
}

fn gzip(input: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(input)?;
    encoder.finish()
}

#[derive(Clone, PartialEq, Message)]
struct RequestFixture {
    #[prost(message, optional, tag = "1")]
    receive: Option<ReceiveFixture>,
    #[prost(message, optional, tag = "2")]
    send: Option<SendFixture>,
    #[prost(message, optional, tag = "15")]
    attributes: Option<AttributesFixture>,
}

#[derive(Clone, PartialEq, Message)]
struct ResponseFixture {
    #[prost(message, optional, tag = "1")]
    receive: Option<ReceiveResponseFixture>,
    #[prost(message, optional, tag = "2")]
    send: Option<SendResponseFixture>,
}

#[derive(Clone, PartialEq, Message)]
struct ReceiveFixture {
    #[prost(message, optional, tag = "1")]
    peer: Option<PeerFixture>,
    #[prost(string, tag = "2")]
    resource_id: String,
    #[prost(uint32, tag = "3")]
    message_type: u32,
}

#[derive(Clone, PartialEq, Message)]
struct SendFixture {
    #[prost(uint32, tag = "1")]
    message_type: u32,
    #[prost(message, optional, tag = "2")]
    peer: Option<PeerFixture>,
    #[prost(uint64, tag = "3")]
    group_uin: u64,
    #[prost(bytes = "vec", tag = "4")]
    payload: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
struct PeerFixture {
    #[prost(string, optional, tag = "2")]
    uid: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct AttributesFixture {
    #[prost(uint32, tag = "1")]
    subcommand: u32,
    #[prost(uint32, optional, tag = "2")]
    client_type: Option<u32>,
    #[prost(uint32, optional, tag = "3")]
    platform: Option<u32>,
    #[prost(uint32, optional, tag = "4")]
    proxy_type: Option<u32>,
}

#[derive(Clone, PartialEq, Message)]
struct ReceiveResponseFixture {
    #[prost(bytes = "vec", tag = "4")]
    payload: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
struct SendResponseFixture {
    #[prost(string, tag = "3")]
    resource_id: String,
}

#[derive(Clone, PartialEq, Message)]
struct TransmitFixture {
    #[prost(bytes = "vec", repeated, tag = "1")]
    messages: Vec<Vec<u8>>,
    #[prost(message, repeated, tag = "2")]
    items: Vec<ItemFixture>,
}

#[derive(Clone, PartialEq, Message)]
struct ItemFixture {
    #[prost(string, tag = "1")]
    file_name: String,
    #[prost(message, optional, tag = "2")]
    buffer: Option<BufferFixture>,
}

#[derive(Clone, PartialEq, Message)]
struct BufferFixture {
    #[prost(bytes = "vec", repeated, tag = "1")]
    messages: Vec<Vec<u8>>,
}

//! Frozen Linux NT group-file element contract.

use prost::Message;
use qq_message::{Segment, decode_rich_text};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Clone, PartialEq, Message)]
struct Rich {
    #[prost(bytes = "vec", repeated, tag = "2")]
    elements: Vec<Vec<u8>>,
}
#[derive(Clone, PartialEq, Message)]
struct Element {
    #[prost(bytes = "vec", optional, tag = "5")]
    transfer: Option<Vec<u8>>,
}
#[derive(Clone, PartialEq, Message)]
struct Transfer {
    #[prost(int32, tag = "1")]
    kind: i32,
    #[prost(bytes = "vec", tag = "2")]
    value: Vec<u8>,
}
#[derive(Clone, PartialEq, Message)]
struct Extra {
    #[prost(message, optional, tag = "7")]
    inner: Option<Inner>,
}
#[derive(Clone, PartialEq, Message)]
struct Inner {
    #[prost(message, optional, tag = "2")]
    info: Option<Info>,
}
#[derive(Clone, PartialEq, Message)]
struct Info {
    #[prost(uint32, tag = "1")]
    bus_id: u32,
    #[prost(string, tag = "2")]
    file_id: String,
    #[prost(int64, tag = "3")]
    size: i64,
    #[prost(string, tag = "4")]
    name: String,
}

#[test]
fn exact_transfer_element_decodes() -> TestResult {
    let decoded = decode_rich_text(&encoded(24, 12)?)?;
    let Segment::File(file) = decoded.elements()[0].segment() else {
        return Err("missing group file".into());
    };
    assert_eq!(file.bus_id(), 102);
    assert_eq!(file.file_id(), "file-id");
    assert_eq!(file.name(), "report.txt");
    assert_eq!(file.size(), 12);
    Ok(())
}

#[test]
fn negative_size_fails_closed_and_other_transfer_types_are_preserved() -> TestResult {
    assert!(decode_rich_text(&encoded(24, -1)?).is_err());
    assert_eq!(
        decode_rich_text(&encoded(23, 12)?)?.elements()[0].segment(),
        &Segment::Unsupported
    );
    Ok(())
}

fn encoded(kind: i32, size: i64) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let extra = Extra {
        inner: Some(Inner {
            info: Some(Info {
                bus_id: 102,
                file_id: "file-id".to_owned(),
                size,
                name: "report.txt".to_owned(),
            }),
        }),
    }
    .encode_to_vec();
    let mut value = Vec::with_capacity(3 + extra.len());
    value.push(0);
    value.extend_from_slice(&u16::try_from(extra.len())?.to_be_bytes());
    value.extend_from_slice(&extra);
    Ok(Rich {
        elements: vec![
            Element {
                transfer: Some(Transfer { kind, value }.encode_to_vec()),
            }
            .encode_to_vec(),
        ],
    }
    .encode_to_vec())
}

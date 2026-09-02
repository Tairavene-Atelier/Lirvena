//! Conservative rich-text projection contracts.

use prost::Message;
use qq_message::{FaceKind, MentionTarget, OpaqueAttachment, Segment, decode_rich_text};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Clone, PartialEq, Message)]
struct RichFixture {
    #[prost(bytes = "vec", optional, tag = "1")]
    attributes: Option<Vec<u8>>,
    #[prost(bytes = "vec", repeated, tag = "2")]
    elements: Vec<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "3")]
    file: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "4")]
    voice: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, Message)]
struct ElementFixture {
    #[prost(bytes = "vec", optional, tag = "1")]
    text: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "2")]
    face: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "53")]
    common: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, Message)]
struct TextFixture {
    #[prost(string, optional, tag = "1")]
    text: Option<String>,
    #[prost(bytes = "vec", optional, tag = "3")]
    legacy_attributes: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "12")]
    reserve: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, Message)]
struct MentionFixture {
    #[prost(int32, tag = "3")]
    kind: i32,
    #[prost(uint32, tag = "4")]
    account: u32,
    #[prost(string, tag = "9")]
    user: String,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct FaceFixture {
    #[prost(int32, optional, tag = "1")]
    index: Option<i32>,
}

#[derive(Clone, PartialEq, Message)]
struct CommonFixture {
    #[prost(int32, tag = "1")]
    service: i32,
    #[prost(bytes = "vec", optional, tag = "2")]
    body: Option<Vec<u8>>,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct SmallFaceFixture {
    #[prost(uint32, tag = "1")]
    id: u32,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct AnimatedFaceFixture {
    #[prost(int32, optional, tag = "3")]
    id: Option<i32>,
}

#[test]
fn text_mentions_and_wire_order_are_projected() -> TestResult {
    let mut legacy = vec![0_u8; 11];
    legacy[7..11].copy_from_slice(&42_u32.to_be_bytes());
    let input = rich([
        text("hello", None, None),
        text(
            "@member",
            Some(legacy),
            Some(MentionFixture {
                kind: 2,
                account: 0,
                user: String::new(),
            }),
        ),
        text(
            "@all",
            None,
            Some(MentionFixture {
                kind: 1,
                account: 0,
                user: "all".to_owned(),
            }),
        ),
    ]);
    let decoded = decode_rich_text(&input)?;
    assert_eq!(decoded.elements().len(), 3);
    assert_eq!(
        decoded.elements()[0].segment(),
        &Segment::Text("hello".to_owned())
    );
    let Segment::Mention(member) = decoded.elements()[1].segment() else {
        return Err("expected account mention".into());
    };
    assert_eq!(member.display(), "@member");
    assert_eq!(member.target(), &MentionTarget::Account(42));
    let Segment::Mention(everyone) = decoded.elements()[2].segment() else {
        return Err("expected everyone mention".into());
    };
    assert_eq!(everyone.target(), &MentionTarget::Everyone);
    Ok(())
}

#[test]
fn legacy_standard_and_common_faces_are_projected() -> TestResult {
    let input = rich([
        element(
            None,
            Some(FaceFixture { index: Some(7) }.encode_to_vec()),
            None,
        ),
        common(33, SmallFaceFixture { id: 301 }.encode_to_vec()),
        common(37, AnimatedFaceFixture { id: Some(318) }.encode_to_vec()),
    ]);
    let decoded = decode_rich_text(&input)?;
    let expected = [
        (7, FaceKind::Standard),
        (301, FaceKind::Standard),
        (318, FaceKind::Animated),
    ];
    for (element, (id, kind)) in decoded.elements().iter().zip(expected) {
        let Segment::Face(face) = element.segment() else {
            return Err("expected face".into());
        };
        assert_eq!((face.id(), face.kind()), (id, kind));
    }
    Ok(())
}

#[test]
fn ambiguous_and_unknown_elements_remain_lossless() -> TestResult {
    let ambiguous = element(
        Some(
            TextFixture {
                text: Some("ambiguous".to_owned()),
                legacy_attributes: None,
                reserve: None,
            }
            .encode_to_vec(),
        ),
        Some(FaceFixture { index: Some(1) }.encode_to_vec()),
        None,
    );
    let unknown = common(999, vec![1, 2, 3]);
    let input = RichFixture {
        attributes: Some(vec![4]),
        elements: vec![ambiguous.clone(), unknown.clone()],
        file: Some(vec![5]),
        voice: Some(vec![6]),
    }
    .encode_to_vec();
    let decoded = decode_rich_text(&input)?;
    assert_eq!(decoded.elements()[0].segment(), &Segment::Unsupported);
    assert_eq!(decoded.elements()[0].encoded(), ambiguous);
    assert_eq!(decoded.elements()[1].segment(), &Segment::Unsupported);
    assert_eq!(decoded.elements()[1].encoded(), unknown);
    assert_eq!(
        decoded.attributes().map(OpaqueAttachment::encoded),
        Some([4].as_slice())
    );
    assert_eq!(
        decoded.file().map(OpaqueAttachment::encoded),
        Some([5].as_slice())
    );
    assert_eq!(
        decoded.voice().map(OpaqueAttachment::encoded),
        Some([6].as_slice())
    );
    Ok(())
}

#[test]
fn malformed_excessive_and_unsafe_bodies_fail_closed() {
    assert!(decode_rich_text(&[]).is_err());
    assert!(decode_rich_text(&[0xff]).is_err());
    assert!(decode_rich_text(&vec![0; 1024 * 1024 + 1]).is_err());
    let excessive = RichFixture {
        attributes: None,
        elements: vec![Vec::new(); 513],
        file: None,
        voice: None,
    }
    .encode_to_vec();
    assert!(decode_rich_text(&excessive).is_err());
    let unsafe_text = rich([text("bad\0text", None, None)]);
    assert!(decode_rich_text(&unsafe_text).is_err());
}

fn rich<const N: usize>(elements: [Vec<u8>; N]) -> Vec<u8> {
    RichFixture {
        attributes: None,
        elements: elements.into(),
        file: None,
        voice: None,
    }
    .encode_to_vec()
}

fn text(
    value: &str,
    legacy_attributes: Option<Vec<u8>>,
    mention: Option<MentionFixture>,
) -> Vec<u8> {
    element(
        Some(
            TextFixture {
                text: Some(value.to_owned()),
                legacy_attributes,
                reserve: mention.map(|value| value.encode_to_vec()),
            }
            .encode_to_vec(),
        ),
        None,
        None,
    )
}

fn common(service: i32, body: Vec<u8>) -> Vec<u8> {
    element(
        None,
        None,
        Some(
            CommonFixture {
                service,
                body: Some(body),
            }
            .encode_to_vec(),
        ),
    )
}

fn element(text: Option<Vec<u8>>, face: Option<Vec<u8>>, common: Option<Vec<u8>>) -> Vec<u8> {
    ElementFixture { text, face, common }.encode_to_vec()
}

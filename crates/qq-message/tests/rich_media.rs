//! Modern rich-media projection contracts.

use prost::Message;
use qq_message::{MediaScope, Segment, decode_rich_text};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Clone, PartialEq, Message)]
struct RichFixture {
    #[prost(bytes = "vec", repeated, tag = "2")]
    elements: Vec<Vec<u8>>,
}

#[derive(Clone, PartialEq, Message)]
struct ElementFixture {
    #[prost(bytes = "vec", optional, tag = "53")]
    common: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, Message)]
struct CommonFixture {
    #[prost(int32, tag = "1")]
    service: i32,
    #[prost(bytes = "vec", optional, tag = "2")]
    body: Option<Vec<u8>>,
    #[prost(uint32, tag = "3")]
    business: u32,
}

#[derive(Clone, PartialEq, Message)]
struct PacketFixture {
    #[prost(message, repeated, tag = "1")]
    bodies: Vec<BodyFixture>,
    #[prost(message, optional, tag = "2")]
    extension: Option<ExtensionFixture>,
}

#[derive(Clone, PartialEq, Message)]
struct BodyFixture {
    #[prost(message, optional, tag = "1")]
    index: Option<IndexFixture>,
    #[prost(message, optional, tag = "2")]
    picture: Option<PictureFixture>,
}

#[derive(Clone, PartialEq, Message)]
struct IndexFixture {
    #[prost(message, optional, tag = "1")]
    info: Option<FileFixture>,
    #[prost(string, tag = "2")]
    uuid: String,
}

#[derive(Clone, PartialEq, Message)]
struct FileFixture {
    #[prost(uint32, tag = "1")]
    size: u32,
    #[prost(string, tag = "2")]
    digest: String,
    #[prost(string, tag = "3")]
    sha1: String,
    #[prost(string, tag = "4")]
    name: String,
    #[prost(uint32, tag = "6")]
    width: u32,
    #[prost(uint32, tag = "7")]
    height: u32,
    #[prost(uint32, tag = "8")]
    duration: u32,
}

#[derive(Clone, PartialEq, Message)]
struct PictureFixture {
    #[prost(string, tag = "1")]
    remote_reference: String,
}

#[derive(Clone, PartialEq, Message)]
struct ExtensionFixture {
    #[prost(message, optional, tag = "1")]
    image: Option<ImageExtensionFixture>,
}

#[derive(Clone, PartialEq, Message)]
struct ImageExtensionFixture {
    #[prost(uint32, tag = "1")]
    subtype: u32,
    #[prost(string, tag = "2")]
    summary: String,
}

#[test]
fn group_image_projects_one_shared_media_file() -> TestResult {
    let input = rich(common(20, packet(true).encode_to_vec()));
    let decoded = decode_rich_text(&input)?;
    let Segment::Image(image) = decoded.elements()[0].segment() else {
        return Err("expected image".into());
    };
    assert_eq!(image.scope(), MediaScope::Group);
    assert_eq!(image.summary(), Some("image summary"));
    assert_eq!(image.subtype(), 1);
    let file = image.file();
    assert_eq!(file.uuid(), Some("media-uuid"));
    assert_eq!(file.name(), "media.bin");
    assert_eq!(file.digest(), Some("AABBCCDD"));
    assert_eq!(file.sha1(), Some("1122AABB"));
    assert_eq!(file.remote_reference(), Some("/download/path"));
    assert_eq!((file.size(), file.width(), file.height()), (4096, 640, 480));
    assert_eq!(file.duration_seconds(), 12);
    Ok(())
}

#[test]
fn direct_video_and_group_voice_share_the_media_shape() -> TestResult {
    let input = RichFixture {
        elements: vec![
            common(11, packet(false).encode_to_vec()),
            common(22, packet(false).encode_to_vec()),
        ],
    }
    .encode_to_vec();
    let decoded = decode_rich_text(&input)?;
    let Segment::Video(video) = decoded.elements()[0].segment() else {
        return Err("expected video".into());
    };
    assert_eq!(video.scope(), MediaScope::Direct);
    assert_eq!(video.file().name(), "media.bin");
    let Segment::Voice(voice) = decoded.elements()[1].segment() else {
        return Err("expected voice".into());
    };
    assert_eq!(voice.scope(), MediaScope::Group);
    assert_eq!(voice.file().duration_seconds(), 12);
    Ok(())
}

#[test]
fn malformed_known_media_fails_but_unknown_business_remains_lossless() -> TestResult {
    let empty = PacketFixture {
        bodies: Vec::new(),
        extension: None,
    }
    .encode_to_vec();
    assert!(decode_rich_text(&rich(common(20, empty.clone()))).is_err());
    let mut invalid = packet(false);
    if let Some(info) = invalid.bodies[0]
        .index
        .as_mut()
        .and_then(|value| value.info.as_mut())
    {
        info.digest = "not-a-digest".to_owned();
    }
    assert!(decode_rich_text(&rich(common(20, invalid.encode_to_vec()))).is_err());
    let unknown = common(999, vec![0xff]);
    let decoded = decode_rich_text(&rich(unknown.clone()))?;
    assert_eq!(decoded.elements()[0].segment(), &Segment::Unsupported);
    assert_eq!(decoded.elements()[0].encoded(), unknown);
    Ok(())
}

fn packet(image_extension: bool) -> PacketFixture {
    PacketFixture {
        bodies: vec![BodyFixture {
            index: Some(IndexFixture {
                info: Some(FileFixture {
                    size: 4096,
                    digest: "aabbccdd".to_owned(),
                    sha1: "1122aabb".to_owned(),
                    name: "media.bin".to_owned(),
                    width: 640,
                    height: 480,
                    duration: 12,
                }),
                uuid: "media-uuid".to_owned(),
            }),
            picture: Some(PictureFixture {
                remote_reference: "/download/path".to_owned(),
            }),
        }],
        extension: image_extension.then(|| ExtensionFixture {
            image: Some(ImageExtensionFixture {
                subtype: 1,
                summary: "image summary".to_owned(),
            }),
        }),
    }
}

fn common(business: u32, body: Vec<u8>) -> Vec<u8> {
    ElementFixture {
        common: Some(
            CommonFixture {
                service: 48,
                body: Some(body),
                business,
            }
            .encode_to_vec(),
        ),
    }
    .encode_to_vec()
}

fn rich(element: Vec<u8>) -> Vec<u8> {
    RichFixture {
        elements: vec![element],
    }
    .encode_to_vec()
}

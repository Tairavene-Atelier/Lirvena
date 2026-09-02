//! Compatibility media projection contracts.

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
    #[prost(bytes = "vec", optional, tag = "4")]
    direct_image: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "8")]
    group_image: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "19")]
    video: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, Message)]
struct DirectImageFixture {
    #[prost(string, tag = "1")]
    name: String,
    #[prost(uint32, tag = "2")]
    size: u32,
    #[prost(bytes = "vec", tag = "7")]
    digest: Vec<u8>,
    #[prost(uint32, tag = "8")]
    height: u32,
    #[prost(uint32, tag = "9")]
    width: u32,
    #[prost(string, tag = "15")]
    remote_reference: String,
    #[prost(message, optional, tag = "29")]
    reserve: Option<DirectReserveFixture>,
}

#[derive(Clone, PartialEq, Message)]
struct DirectReserveFixture {
    #[prost(int32, tag = "1")]
    subtype: i32,
    #[prost(string, tag = "8")]
    summary: String,
}

#[derive(Clone, PartialEq, Message)]
struct GroupImageFixture {
    #[prost(string, tag = "2")]
    name: String,
    #[prost(bytes = "vec", tag = "13")]
    digest: Vec<u8>,
    #[prost(string, tag = "16")]
    remote_reference: String,
    #[prost(int32, tag = "22")]
    width: i32,
    #[prost(int32, tag = "23")]
    height: i32,
    #[prost(uint32, tag = "25")]
    size: u32,
}

#[derive(Clone, PartialEq, Message)]
struct VideoFixture {
    #[prost(string, tag = "1")]
    uuid: String,
    #[prost(bytes = "vec", tag = "2")]
    digest: Vec<u8>,
    #[prost(string, tag = "3")]
    name: String,
    #[prost(int32, tag = "5")]
    duration: i32,
    #[prost(int32, tag = "6")]
    size: i32,
    #[prost(int32, tag = "7")]
    thumbnail_width: i32,
    #[prost(int32, tag = "8")]
    thumbnail_height: i32,
    #[prost(int32, tag = "16")]
    width: i32,
    #[prost(int32, tag = "17")]
    height: i32,
}

#[test]
fn direct_and_group_images_use_the_shared_media_model() -> TestResult {
    let input = rich(vec![
        ElementFixture {
            direct_image: Some(
                DirectImageFixture {
                    name: "direct.jpg".to_owned(),
                    size: 10,
                    digest: vec![0xaa, 0xbb],
                    height: 20,
                    width: 30,
                    remote_reference: "/direct".to_owned(),
                    reserve: Some(DirectReserveFixture {
                        subtype: 1,
                        summary: "direct image".to_owned(),
                    }),
                }
                .encode_to_vec(),
            ),
            group_image: None,
            video: None,
        }
        .encode_to_vec(),
        ElementFixture {
            direct_image: None,
            group_image: Some(
                GroupImageFixture {
                    name: "group.jpg".to_owned(),
                    digest: vec![0x11, 0x22],
                    remote_reference: "/group".to_owned(),
                    width: 40,
                    height: 50,
                    size: 60,
                }
                .encode_to_vec(),
            ),
            video: None,
        }
        .encode_to_vec(),
    ]);
    let decoded = decode_rich_text(&input)?;
    let Segment::Image(direct) = decoded.elements()[0].segment() else {
        return Err("expected direct image".into());
    };
    assert_eq!(direct.scope(), MediaScope::Direct);
    assert_eq!(direct.file().digest(), Some("AABB"));
    assert_eq!(direct.summary(), Some("direct image"));
    let Segment::Image(group) = decoded.elements()[1].segment() else {
        return Err("expected group image".into());
    };
    assert_eq!(group.scope(), MediaScope::Group);
    assert_eq!((group.file().width(), group.file().height()), (40, 50));
    Ok(())
}

#[test]
fn compatibility_video_does_not_invent_a_scope() -> TestResult {
    let video = VideoFixture {
        uuid: "video-uuid".to_owned(),
        digest: vec![0xde, 0xad],
        name: "video.mp4".to_owned(),
        duration: 12,
        size: 100,
        thumbnail_width: 320,
        thumbnail_height: 180,
        width: 0,
        height: 0,
    };
    let input = rich(vec![
        ElementFixture {
            direct_image: None,
            group_image: None,
            video: Some(video.encode_to_vec()),
        }
        .encode_to_vec(),
    ]);
    let decoded = decode_rich_text(&input)?;
    let Segment::Video(video) = decoded.elements()[0].segment() else {
        return Err("expected video".into());
    };
    assert_eq!(video.scope(), MediaScope::Unknown);
    assert_eq!(video.file().uuid(), Some("video-uuid"));
    assert_eq!((video.file().width(), video.file().height()), (320, 180));
    Ok(())
}

#[test]
fn negative_compatibility_dimensions_fail_closed() {
    let invalid = GroupImageFixture {
        name: "bad.jpg".to_owned(),
        digest: vec![1],
        remote_reference: String::new(),
        width: -1,
        height: 1,
        size: 1,
    };
    let input = rich(vec![
        ElementFixture {
            direct_image: None,
            group_image: Some(invalid.encode_to_vec()),
            video: None,
        }
        .encode_to_vec(),
    ]);
    assert!(decode_rich_text(&input).is_err());
}

fn rich(elements: Vec<Vec<u8>>) -> Vec<u8> {
    RichFixture { elements }.encode_to_vec()
}

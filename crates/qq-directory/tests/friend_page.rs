//! Friend-directory request and response contract tests.

use prost::Message;
use qq_directory::{encode_friend_page_request, parse_friend_page};

#[test]
fn request_is_bounded_and_pagination_changes_only_the_cursor() {
    let first = encode_friend_page_request(None);
    let second = encode_friend_page_request(Some(42));
    assert!(!first.is_empty());
    assert!(first.len() < 512);
    assert!(second.len() > first.len());
    assert!(
        second
            .windows(4)
            .any(|value| value == [0x2a, 0x02, 0x08, 0x2a])
    );
}

#[test]
fn response_preserves_uin_uid_and_properties() -> Result<(), Box<dyn std::error::Error>> {
    let response = Envelope {
        error_code: 0,
        body: Some(Body {
            next: Some(Next { uin: 99 }),
            friends: vec![Friend {
                uid: "uid-42".to_owned(),
                uin: 42,
                additional: vec![Additional {
                    kind: 1,
                    layer: Some(Layer {
                        properties: vec![
                            Property {
                                code: 20_002,
                                value: "nickname".to_owned(),
                            },
                            Property {
                                code: 103,
                                value: "remark".to_owned(),
                            },
                        ],
                    }),
                }],
            }],
        }),
    };
    let page = parse_friend_page(&response.encode_to_vec())?;
    assert_eq!(page.next_uin, Some(99));
    assert_eq!(page.friends.len(), 1);
    assert_eq!(page.friends[0].uin, 42);
    assert_eq!(page.friends[0].uid, "uid-42");
    assert_eq!(page.friends[0].nickname, "nickname");
    assert_eq!(page.friends[0].remark, "remark");
    Ok(())
}

#[derive(Clone, PartialEq, Message)]
struct Envelope {
    #[prost(uint32, tag = "3")]
    error_code: u32,
    #[prost(message, optional, tag = "4")]
    body: Option<Body>,
}

#[derive(Clone, PartialEq, Message)]
struct Body {
    #[prost(message, optional, tag = "2")]
    next: Option<Next>,
    #[prost(message, repeated, tag = "101")]
    friends: Vec<Friend>,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct Next {
    #[prost(uint32, tag = "1")]
    uin: u32,
}

#[derive(Clone, PartialEq, Message)]
struct Friend {
    #[prost(string, tag = "1")]
    uid: String,
    #[prost(uint32, tag = "3")]
    uin: u32,
    #[prost(message, repeated, tag = "10001")]
    additional: Vec<Additional>,
}

#[derive(Clone, PartialEq, Message)]
struct Additional {
    #[prost(uint32, tag = "1")]
    kind: u32,
    #[prost(message, optional, tag = "2")]
    layer: Option<Layer>,
}

#[derive(Clone, PartialEq, Message)]
struct Layer {
    #[prost(message, repeated, tag = "2")]
    properties: Vec<Property>,
}

#[derive(Clone, PartialEq, Message)]
struct Property {
    #[prost(uint32, tag = "1")]
    code: u32,
    #[prost(string, tag = "2")]
    value: String,
}

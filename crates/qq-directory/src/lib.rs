//! QQ friend and group directory codecs for Lirvena.
#![forbid(unsafe_code)]

mod friend;

pub use friend::{
    FriendDirectoryError, FriendEntry, FriendPage, encode_friend_page_request, parse_friend_page,
};

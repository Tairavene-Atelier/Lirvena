//! QQ friend and group directory codecs for Lirvena.
#![forbid(unsafe_code)]

mod friend;
mod group;

pub use friend::{
    FriendDirectoryError, FriendEntry, FriendPage, encode_friend_page_request, parse_friend_page,
};
pub use group::{GroupDirectoryError, GroupEntry, encode_group_list_request, parse_group_list};

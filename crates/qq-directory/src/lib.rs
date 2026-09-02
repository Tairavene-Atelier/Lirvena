//! QQ friend and group directory codecs for Lirvena.
#![forbid(unsafe_code)]

mod fields;
mod friend;
mod group;
mod member;

pub use friend::{
    FriendDirectoryError, FriendEntry, FriendPage, encode_friend_page_request, parse_friend_page,
};
pub use group::{GroupDirectoryError, GroupEntry, encode_group_list_request, parse_group_list};
pub use member::{
    GroupMember, GroupMemberPage, GroupMemberRole, encode_group_member_page_request,
    parse_group_member_page,
};

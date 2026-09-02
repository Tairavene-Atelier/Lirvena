//! QQ friend and group directory codecs for Lirvena.
#![forbid(unsafe_code)]

mod fields;
mod friend;
mod friend_request;
mod group;
mod member;
mod request;
mod user;

pub use friend::{
    FriendDirectoryError, FriendEntry, FriendPage, encode_friend_page_request, parse_friend_page,
};
pub use friend_request::{
    FriendRequestDirectoryError, FriendRequestRecord, encode_friend_request_list_request,
    parse_friend_request_list,
};
pub use group::{GroupDirectoryError, GroupEntry, encode_group_list_request, parse_group_list};
pub use member::{
    GroupMember, GroupMemberPage, GroupMemberRole, encode_group_member_page_request,
    parse_group_member_page,
};
pub use request::{
    GroupRequestDirectoryError, GroupRequestKind, GroupRequestRecord,
    encode_group_request_list_request, parse_group_request_list,
};
pub use user::{UserDirectoryError, encode_user_lookup_request, parse_user_lookup};

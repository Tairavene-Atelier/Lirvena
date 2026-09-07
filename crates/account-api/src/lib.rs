#![forbid(unsafe_code)]
//! Shared, bounded account event boundary used by every Lirvena adapter.

mod action;
mod event;
mod friend_recall;
mod friend_request;
mod group;
mod group_mute;
mod group_name;
mod group_recall;
mod hub;
mod reaction;
mod request;

pub use action::{
    AccountActionError, AccountActionHandle, AccountActionReceiver, AccountActionRequest,
    PendingAccountAction, account_action_channel,
};
pub use event::{AccountEvent, AccountIdentity, InboundMessage};
pub use friend_recall::ResolvedFriendRecall;
pub use friend_request::{FriendRequestReference, ResolvedFriendRequest};
pub use group::{ResolvedGroupNotice, ResolvedGroupNoticeKind};
pub use group_mute::ResolvedGroupMute;
pub use group_name::ResolvedGroupNameChange;
pub use group_recall::ResolvedGroupRecall;
pub use hub::{AccountEventHub, AccountEventPublisher, AccountEventSubscription, EventHubError};
pub use reaction::ResolvedGroupReaction;
pub use request::{GroupRequestKind, GroupRequestReference, ResolvedGroupRequest};

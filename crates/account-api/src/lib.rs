#![forbid(unsafe_code)]
//! Shared, bounded account event boundary used by every Lirvena adapter.

mod action;
mod event;
mod hub;

pub use action::{
    AccountActionError, AccountActionHandle, AccountActionReceiver, AccountActionRequest,
    PendingAccountAction, account_action_channel,
};
pub use event::{AccountEvent, AccountIdentity, InboundMessage};
pub use hub::{AccountEventHub, AccountEventPublisher, AccountEventSubscription, EventHubError};

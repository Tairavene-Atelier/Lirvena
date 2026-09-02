#![forbid(unsafe_code)]
//! Shared, bounded account event boundary used by every Lirvena adapter.

mod event;
mod hub;

pub use event::{AccountEvent, AccountIdentity, InboundMessage};
pub use hub::{AccountEventHub, AccountEventPublisher, AccountEventSubscription, EventHubError};

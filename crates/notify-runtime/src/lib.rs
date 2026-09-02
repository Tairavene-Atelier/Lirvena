//! User notification boundary for Lirvena.

mod adapter;
mod error;
mod id;
mod model;
mod runtime;
mod store;

pub use adapter::{
    AdapterError, BarkAdapter, BarkConfig, BarkLevel, NotificationAdapter, SmtpAdapter, SmtpConfig,
    SmtpSecurity, WebhookAdapter, WebhookConfig,
};
pub use error::NotificationError;
pub use id::{DedupeKey, DeliveryId, DestinationId, EventId};
pub use model::{
    EventCategory, EventSource, EventState, NotificationEvent, NotificationText, Severity,
    StateTransition,
};
pub use runtime::{
    DeliverySweep, NotificationHandle, NotificationRuntime, NotificationRuntimeConfig,
    spawn_notification_runtime,
};
pub use store::{Delivery, FailureDisposition, NotificationStore};

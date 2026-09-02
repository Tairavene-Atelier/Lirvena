//! Durable, local-only aggregation for Lirvena Community telemetry.
//!
//! Exact counters never leave this crate. Callers can obtain only frozen coarse buckets for a
//! completed UTC day. The store contains no QQ identifiers, message bodies, names, host identity,
//! network address, or device-profile fields.

mod error;
mod model;
mod schema;
mod store;

pub use error::TelemetryStoreError;
pub use model::CompletedDay;
pub use store::CommunityTelemetryStore;

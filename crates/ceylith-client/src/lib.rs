//! Public Ceylith client contract boundary for Lirvena.

mod action;
mod connection;
mod error;
mod handshake;
mod identity;
mod installation;
mod opaque;
mod profile;
mod runtime;
mod tcp;

pub use action::{
    ActionDirective, ActionFlowUpdate, ActionMark, action_flow_inputs, decode_action_flow_update,
};
pub use ceylith_protocol::{ActionFlowContext, ActionObservation, ActionObservationKind};
pub use connection::{ClientConnection, RequestedAccess};
pub use error::ClientError;
pub use handshake::PendingHandshake;
pub use identity::{AccessToken, InstallationIdentity};
pub use installation::{
    InstallationClient, InstallationClientRuntime, InstallationWatch, InstallationWatchRuntime,
    spawn_installation_client, spawn_installation_watch,
};
pub use opaque::{OpaqueExchangeContext, OpaqueExchangeResult, decode_opaque_exchange_response};
pub use profile::ProfileVerifier;
pub use runtime::{Architecture, Platform, RuntimeDescriptor};
pub use tcp::CeylithTcpClient;

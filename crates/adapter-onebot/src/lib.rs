//! `OneBot` 11 protocol core shared by every Lirvena `OneBot` transport.
#![forbid(unsafe_code)]

mod action;
mod backend;
mod dispatch;
mod event;
mod forward;
mod id;
mod message;
mod quick;
mod response;
mod transport;

pub use action::{ActionMode, ActionRequest};
pub use backend::{AccountChannelBackend, BackendCall, BackendError, OneBotBackend};
pub use dispatch::{DispatcherConfig, OneBotDispatcher};
pub use event::{
    EventProjectionError, project_account_event, project_forward_node, project_history_message,
    project_message_record,
};
pub use forward::{ForwardNode, parse_forward_nodes};
pub use id::IdFormat;
pub use message::{MessageParseError, MessageSegment, parse_message};
pub use response::ActionResponse;
pub use transport::{
    EventBusError, ForwardServerConfig, HttpEventReporter, HttpEventReporterConfig, OneBotEventBus,
    OneBotForwardServer, OneBotForwardServerError, OutboundTransportError, ReverseWebSocket,
    ReverseWebSocketConfig,
};

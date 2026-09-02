#![doc = "Bounded public wire contract shared by Ceylith and Lirvena."]

mod action;
mod bounds;
mod error;
mod frame;
mod ids;
mod inner;
mod opaque;
mod profile;
mod session;
mod watch;

/// Types generated from the public `ceylith.v2` schema.
#[allow(missing_docs, clippy::doc_markdown, clippy::must_use_candidate)]
pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/ceylith.v2.rs"));
}

pub use action::{
    ActionFlowContext, ActionObservation, ActionObservationKind, action_flow_binding_digest,
    action_observation_binding_digest,
};
pub use bounds::{
    DEFAULT_INNER_FRAME_LEN, DEFAULT_SECURE_CIPHERTEXT_LEN, HARD_MAX_OUTER_FRAME_LEN,
    MAX_ACCESS_TOKEN_LEN, MAX_ACTION_BODY_LEN, MAX_ACTION_DELAY_MS, MAX_ACTION_MARK_AGGREGATE_LEN,
    MAX_ACTION_MARK_LEN, MAX_ACTION_MARKS, MAX_ACTION_PAYLOAD_LEN, MAX_ACTION_ROUTE_LEN,
    MAX_ACTION_TIMEOUT_MS, MAX_MANIFEST_LEN, MAX_OPAQUE_AGGREGATE_LEN, MAX_OPAQUE_SLOT_LEN,
    MAX_OPAQUE_SLOTS, MAX_RUNTIME_LEASE_LEN, MAX_WATCH_PAYLOAD_LEN, MAX_WATCH_WAIT_MS,
};
pub use error::{CodecError, FrameKind, LengthKind, OpaqueError, ProfileError};
pub use frame::{
    CURRENT_WIRE_VERSION, HANDSHAKE_HEADER_LEN, HANDSHAKE_MAGIC, HandshakeEnvelope, HandshakeStep,
    SECURE_FRAME_HEADER_LEN, SECURE_FRAME_MAGIC, SecureFrame, WireLimits,
    decode_handshake_envelope, decode_secure_frame, encode_handshake_envelope, encode_secure_frame,
    encode_secure_frame_header,
};
pub use ids::{
    AccountSlotId, ActionFlowId, ActionId, Digest32, ExchangeId, FixedBytesLengthError, IncidentId,
    InstallationId, ProfileId, RequestId, SessionId,
};
pub use inner::{CURRENT_INNER_CONTRACT, decode_inner_frame, encode_inner_frame};
pub use opaque::{OpaqueSlot, OpaqueSlotId, OpaqueSlots, opaque_binding_digest};
pub use profile::{ProfileOutcome, ReadyProfile, decode_profile_outcome};
pub use session::{
    GrantClass, SessionAdmission, decode_session_welcome, profile_decision_signing_transcript,
    session_hello_signing_transcript, validate_client_runtime, validate_session_hello,
};
pub use watch::{
    RenewalState, WatchEvent, WatchEventKind, WatchGrantSnapshot, WatchOutcome,
    decode_watch_response,
};

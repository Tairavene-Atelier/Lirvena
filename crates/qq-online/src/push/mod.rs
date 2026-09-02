mod config;
mod info_sync;
mod jce;
mod model;
mod multi_video;
mod protobuf;

pub use info_sync::{InfoSyncPushOutcome, InfoSyncPushState, InfoSyncPushSummary};
pub use model::{ProtectiveNotice, PushOutcome};

use qq_profile::{PushBehavior, PushPlanEntry};

use crate::{OnlinePacketError, OnlineSyncState};

/// Stateful executor for signed-Profile-selected compiled Push primitives.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PushProcessor {
    info_sync: InfoSyncPushState,
}

impl PushProcessor {
    /// Returns the bounded synchronization state retained for the current generation.
    #[must_use]
    pub const fn info_sync_state(&self) -> &InfoSyncPushState {
        &self.info_sync
    }

    /// Applies one authenticated Push without exceeding the caller's message queue capacity.
    ///
    /// # Errors
    ///
    /// Returns an error when the body exceeds its Profile bound, the queue has insufficient
    /// capacity, or the selected compiled codec rejects the body.
    pub fn apply(
        &mut self,
        entry: &PushPlanEntry,
        body: &[u8],
        sync: &mut OnlineSyncState,
        available_message_slots: usize,
        available_message_bytes: usize,
    ) -> Result<PushOutcome, OnlinePacketError> {
        if body.len() > usize::try_from(entry.maximum_body_len()).map_err(|_| OnlinePacketError)? {
            return Err(OnlinePacketError);
        }
        match entry.behavior() {
            PushBehavior::EchoBody => Ok(PushOutcome::Ack {
                route: response_route(entry)?.to_owned(),
                body: body.to_vec(),
            }),
            PushBehavior::ProtobufPairAck => Ok(PushOutcome::Ack {
                route: response_route(entry)?.to_owned(),
                body: protobuf::pair_ack(body, entry.parameter())?,
            }),
            PushBehavior::ConfigAck => Ok(PushOutcome::Ack {
                route: response_route(entry)?.to_owned(),
                body: config::build_ack(body)?,
            }),
            PushBehavior::Observe => Ok(PushOutcome::Observed),
            PushBehavior::ProtectiveOffline => Ok(PushOutcome::ProtectiveOffline(
                protobuf::protective_notice(body)?,
            )),
            PushBehavior::Message
                if available_message_slots != 0 && body.len() <= available_message_bytes =>
            {
                Ok(PushOutcome::Message(body.to_vec()))
            }
            PushBehavior::Message => Err(OnlinePacketError),
            PushBehavior::LegacyVideoAck => match multi_video::build_ack(body)? {
                Some(body) => Ok(PushOutcome::Ack {
                    route: response_route(entry)?.to_owned(),
                    body,
                }),
                None => Ok(PushOutcome::Observed),
            },
            PushBehavior::InfoSyncState => Ok(PushOutcome::InfoSync(info_sync::apply(
                body,
                &mut self.info_sync,
                sync,
                available_message_slots,
                available_message_bytes,
            )?)),
        }
    }
}

fn response_route(entry: &PushPlanEntry) -> Result<&str, OnlinePacketError> {
    entry.response_route().ok_or(OnlinePacketError)
}

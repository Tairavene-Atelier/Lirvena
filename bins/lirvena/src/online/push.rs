use std::collections::VecDeque;
use std::io;

use qq_login::CredentialLogin;
use qq_online::{PushOutcome, PushProcessor};
use qq_profile::{LinuxNtProfile, PushPlan, decode_push_plan};
use qq_session::AuthenticatedSession;
use tokio::net::TcpStream;

use super::packets::PacketRuntime;

const MAX_QUEUED_MESSAGES: usize = 256;
const MAX_QUEUED_MESSAGE_BYTES: usize = 16 * 1024 * 1024;

pub(super) struct PushRuntime {
    plan: PushPlan,
    processor: PushProcessor,
    messages: VecDeque<Vec<u8>>,
    queued_message_bytes: usize,
}

impl PushRuntime {
    pub(super) fn new(profile: &LinuxNtProfile) -> Result<Self, qq_profile::PushPlanError> {
        Ok(Self {
            plan: decode_push_plan(profile)?,
            processor: PushProcessor::default(),
            messages: VecDeque::new(),
            queued_message_bytes: 0,
        })
    }

    pub(super) const fn plan(&self) -> &PushPlan {
        &self.plan
    }

    pub(super) fn admits(&self, route: &str) -> bool {
        self.plan.find(route).is_some()
    }

    pub(super) async fn drain(
        &mut self,
        qq: &mut AuthenticatedSession<TcpStream>,
        packets: &mut PacketRuntime,
        profile: &LinuxNtProfile,
        credential: &CredentialLogin,
        uin: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        while let Some(push) = qq.pop_push() {
            self.handle(qq, packets, profile, credential, uin, push)
                .await?;
        }
        Ok(())
    }

    pub(super) async fn handle(
        &mut self,
        qq: &mut AuthenticatedSession<TcpStream>,
        packets: &mut PacketRuntime,
        profile: &LinuxNtProfile,
        credential: &CredentialLogin,
        uin: u64,
        push: qq_envelope::SsoResponse,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let entry = self
            .plan
            .find(push.command())
            .ok_or_else(|| io::Error::other("authenticated QQ Push route is not admitted"))?;
        let available_message_slots = MAX_QUEUED_MESSAGES.saturating_sub(self.messages.len());
        let available_message_bytes =
            MAX_QUEUED_MESSAGE_BYTES.saturating_sub(self.queued_message_bytes);
        match self.processor.apply(
            entry,
            push.payload(),
            packets.sync_state_mut(),
            available_message_slots,
            available_message_bytes,
        )? {
            PushOutcome::Ack { route, body } => {
                let key = crate::qq::session_key(credential)?;
                let auth = crate::qq::authenticated(uin, credential, &key)?;
                packets
                    .acknowledge_push(qq, profile, &auth, &route, &body)
                    .await?;
            }
            PushOutcome::Observed => {}
            PushOutcome::ProtectiveOffline(notice) => {
                eprintln!(
                    "QQ requested protective offline: {} ({})",
                    notice.title, notice.detail
                );
                return Err(io::Error::other("QQ requested protective offline").into());
            }
            PushOutcome::Message(body) => {
                self.queued_message_bytes = self.queued_message_bytes.saturating_add(body.len());
                self.messages.push_back(body);
            }
            PushOutcome::InfoSync(outcome) => {
                self.queued_message_bytes = self
                    .queued_message_bytes
                    .saturating_add(outcome.summary().delivered_message_bytes);
                self.messages.extend(outcome.into_messages());
            }
        }
        Ok(())
    }
}

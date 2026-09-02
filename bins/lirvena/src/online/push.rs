use std::collections::VecDeque;
use std::io;

use qq_login::CredentialLogin;
use qq_message::{
    GroupNotice, MessageDecoder, MessageDisposition, MessageEnvelope, RichTextMessage,
    decode_group_notice, decode_rich_text,
};
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
    decoder: MessageDecoder,
    events: VecDeque<DecodedPush>,
    queued_message_bytes: usize,
}

/// One authenticated message admitted by the current QQ transport generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DecodedMessage {
    envelope: MessageEnvelope,
    rich_text: Option<RichTextMessage>,
}

/// One authenticated QQ event admitted by the current transport generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum DecodedPush {
    Message(Box<DecodedMessage>),
    GroupNotice {
        notice: GroupNotice,
        occurred_at: u64,
        encoded_len: usize,
    },
}

impl DecodedPush {
    fn encoded_len(&self) -> usize {
        match self {
            Self::Message(message) => encoded_message_len(message),
            Self::GroupNotice { encoded_len, .. } => *encoded_len,
        }
    }
}

impl DecodedMessage {
    pub(super) const fn envelope(&self) -> &MessageEnvelope {
        &self.envelope
    }

    pub(super) const fn rich_text(&self) -> Option<&RichTextMessage> {
        self.rich_text.as_ref()
    }

    pub(super) fn into_parts(self) -> (MessageEnvelope, Option<RichTextMessage>) {
        (self.envelope, self.rich_text)
    }
}

impl PushRuntime {
    pub(super) fn new(profile: &LinuxNtProfile) -> Result<Self, qq_profile::PushPlanError> {
        Ok(Self {
            plan: decode_push_plan(profile)?,
            processor: PushProcessor::default(),
            decoder: MessageDecoder::default(),
            events: VecDeque::new(),
            queued_message_bytes: 0,
        })
    }

    pub(super) const fn plan(&self) -> &PushPlan {
        &self.plan
    }

    pub(super) fn admits(&self, route: &str) -> bool {
        self.plan.find(route).is_some()
    }

    pub(super) fn pop_event(&mut self) -> Option<DecodedPush> {
        let event = self.events.pop_front()?;
        self.queued_message_bytes = self
            .queued_message_bytes
            .saturating_sub(event.encoded_len());
        Some(event)
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
        let available_message_slots = MAX_QUEUED_MESSAGES.saturating_sub(self.events.len());
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
                self.admit_message(&body, false)?;
            }
            PushOutcome::InfoSync(outcome) => {
                for body in outcome.into_messages() {
                    self.admit_message(&body, true)?;
                }
            }
        }
        Ok(())
    }

    fn admit_message(
        &mut self,
        body: &[u8],
        embedded: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let disposition = if embedded {
            self.decoder.decode_embedded(body)?
        } else {
            self.decoder.decode(body)?
        };
        let MessageDisposition::New(envelope) = disposition else {
            return Ok(());
        };
        let encoded_len = encoded_envelope_len(&envelope);
        if let Some(notice) = decode_group_notice(&envelope)? {
            self.queued_message_bytes = self
                .queued_message_bytes
                .checked_add(encoded_len)
                .ok_or_else(|| io::Error::other("message queue byte count overflow"))?;
            self.events.push_back(DecodedPush::GroupNotice {
                notice,
                occurred_at: u64::try_from(envelope.timestamp()).unwrap_or_default(),
                encoded_len,
            });
            return Ok(());
        }
        let rich_text = envelope
            .payload()
            .rich_text()
            .map(decode_rich_text)
            .transpose()?;
        let message = DecodedMessage {
            envelope: *envelope,
            rich_text,
        };
        self.queued_message_bytes = self
            .queued_message_bytes
            .checked_add(encoded_message_len(&message))
            .ok_or_else(|| io::Error::other("message queue byte count overflow"))?;
        self.events
            .push_back(DecodedPush::Message(Box::new(message)));
        Ok(())
    }
}

fn encoded_message_len(message: &DecodedMessage) -> usize {
    encoded_envelope_len(message.envelope())
}

fn encoded_envelope_len(envelope: &MessageEnvelope) -> usize {
    let payload = envelope.payload();
    payload.rich_text().map_or(0, <[u8]>::len)
        + payload.content().map_or(0, <[u8]>::len)
        + payload.encrypted_content().map_or(0, <[u8]>::len)
}

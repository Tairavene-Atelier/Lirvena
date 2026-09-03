use account_runtime::{AccountLocalId, AccountPhase, ProtectiveReason};
use qq_message::{MessageEnvelope, RichTextMessage};

/// Public identity established only after QQ accepts a login generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountIdentity {
    local_id: AccountLocalId,
    qq_id: u64,
    nickname: String,
}

impl AccountIdentity {
    /// Creates a bounded account identity.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero QQ identifier, empty nickname, embedded NUL, or a nickname
    /// longer than 512 UTF-8 bytes.
    pub fn new(
        local_id: AccountLocalId,
        qq_id: u64,
        nickname: String,
    ) -> Result<Self, EventHubError> {
        if qq_id == 0 || nickname.is_empty() || nickname.len() > 512 || nickname.contains('\0') {
            return Err(EventHubError::InvalidEvent);
        }
        Ok(Self {
            local_id,
            qq_id,
            nickname,
        })
    }

    /// Returns the installation-local account identifier.
    #[must_use]
    pub const fn local_id(&self) -> AccountLocalId {
        self.local_id
    }

    /// Returns the authenticated QQ identifier.
    #[must_use]
    pub const fn qq_id(&self) -> u64 {
        self.qq_id
    }

    /// Returns the authenticated display nickname.
    #[must_use]
    pub fn nickname(&self) -> &str {
        &self.nickname
    }
}

/// One authenticated incoming message projected by the shared QQ decoder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InboundMessage {
    account: AccountIdentity,
    message_id: u32,
    envelope: MessageEnvelope,
    rich_text: Option<RichTextMessage>,
    reply_ids: Box<[Option<u32>]>,
}

impl InboundMessage {
    /// Creates an immutable adapter-facing message event.
    #[must_use]
    pub fn new(
        account: AccountIdentity,
        message_id: u32,
        envelope: MessageEnvelope,
        rich_text: Option<RichTextMessage>,
    ) -> Self {
        Self {
            account,
            message_id,
            envelope,
            rich_text,
            reply_ids: Box::new([]),
        }
    }

    /// Attaches account-local identifiers resolved for incoming reply elements.
    ///
    /// # Errors
    ///
    /// Returns an error unless the mapping is aligned with every decoded rich
    /// element and contains only non-zero identifiers.
    pub fn with_reply_ids(mut self, reply_ids: Vec<Option<u32>>) -> Result<Self, EventHubError> {
        let expected = self
            .rich_text
            .as_ref()
            .map_or(0, |rich| rich.elements().len());
        if reply_ids.len() != expected || reply_ids.iter().flatten().any(|value| *value == 0) {
            return Err(EventHubError::InvalidEvent);
        }
        self.reply_ids = reply_ids.into_boxed_slice();
        Ok(self)
    }

    /// Returns the account-local `OneBot` message identifier.
    #[must_use]
    pub const fn message_id(&self) -> u32 {
        self.message_id
    }

    /// Returns the authenticated receiving account.
    #[must_use]
    pub const fn account(&self) -> &AccountIdentity {
        &self.account
    }

    /// Returns the validated QQ envelope.
    #[must_use]
    pub const fn envelope(&self) -> &MessageEnvelope {
        &self.envelope
    }

    /// Returns decoded rich text when the QQ body carried it.
    #[must_use]
    pub const fn rich_text(&self) -> Option<&RichTextMessage> {
        self.rich_text.as_ref()
    }

    /// Returns the locally resolved source identifier for one rich element.
    #[must_use]
    pub fn reply_id(&self, element_index: usize) -> Option<u32> {
        self.reply_ids.get(element_index).copied().flatten()
    }
}

/// Adapter-facing event emitted by one account generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccountEvent {
    /// QQ identity became available for the current generation.
    IdentityReady(AccountIdentity),
    /// A durable account lifecycle transition was committed.
    Lifecycle {
        /// Installation-local account identifier.
        local_id: AccountLocalId,
        /// New durable phase.
        phase: AccountPhase,
        /// Protective reason required only for protective offline.
        protective_reason: Option<ProtectiveReason>,
        /// Unix event time in milliseconds.
        occurred_at_ms: u64,
    },
    /// One authenticated, deduplicated message.
    Message(Box<InboundMessage>),
    /// One authenticated group-system notice with resolved numeric identities.
    GroupNotice(Box<crate::ResolvedGroupNotice>),
    /// One authenticated group request with an actionable, versioned reference.
    GroupRequest(Box<crate::ResolvedGroupRequest>),
    /// One authenticated friend request with an actionable, versioned reference.
    FriendRequest(Box<crate::ResolvedFriendRequest>),
    /// One outbound message was accepted by QQ for this account.
    OutboundMessageAccepted {
        /// Installation-local account identifier.
        local_id: AccountLocalId,
        /// Local observation time in Unix milliseconds.
        occurred_at_ms: u64,
    },
    /// One full group-list synchronization completed for this account.
    GroupCountObserved {
        /// Installation-local account identifier.
        local_id: AccountLocalId,
        /// Exact per-account count retained only inside local aggregation.
        count: u64,
        /// Local observation time in Unix milliseconds.
        occurred_at_ms: u64,
    },
}

use crate::EventHubError;

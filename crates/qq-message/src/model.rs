/// Closed outer classification of an authenticated QQ message Push.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageClass {
    /// Direct user message.
    Private,
    /// Group message.
    Group,
    /// Temporary-session message.
    Temporary,
    /// Direct voice-record message.
    PrivateRecord,
    /// Direct file message.
    PrivateFile,
    /// Group join request notification.
    GroupJoinRequest,
    /// Group invitation request notification.
    GroupInvitationRequest,
    /// Invitation for the current account to join a group.
    GroupInvite,
    /// Group administrator change notification.
    GroupAdministratorChange,
    /// Group member increase notification.
    GroupMemberIncrease,
    /// Group member decrease notification.
    GroupMemberDecrease,
    /// Friend-related system event whose subtype requires a compiled decoder.
    FriendEvent,
    /// Group-related system event whose subtype requires a compiled decoder.
    GroupEvent,
    /// An authenticated outer type not yet mapped to a compiled decoder.
    Unknown(u32),
}

impl MessageClass {
    pub(super) const fn from_wire(value: u32) -> Self {
        match value {
            166 => Self::Private,
            82 => Self::Group,
            141 => Self::Temporary,
            208 => Self::PrivateRecord,
            529 => Self::PrivateFile,
            84 => Self::GroupJoinRequest,
            525 => Self::GroupInvitationRequest,
            87 => Self::GroupInvite,
            44 => Self::GroupAdministratorChange,
            33 => Self::GroupMemberIncrease,
            34 => Self::GroupMemberDecrease,
            528 => Self::FriendEvent,
            732 => Self::GroupEvent,
            other => Self::Unknown(other),
        }
    }
}

/// Ordinary sender, recipient and optional group routing metadata.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MessageRoute {
    /// Sender numeric identifier.
    pub from_uin: u32,
    /// Sender string identifier when present.
    pub from_uid: Option<String>,
    /// Recipient numeric identifier.
    pub to_uin: u32,
    /// Recipient string identifier when present.
    pub to_uid: Option<String>,
    /// Group numeric identifier when present.
    pub group_uin: Option<u32>,
    /// Bounded member display name when present.
    pub member_name: Option<String>,
    /// Bounded group display name when present.
    pub group_name: Option<String>,
    /// Bounded direct-contact display name when present.
    pub friend_name: Option<String>,
}

/// Bounded raw sub-payloads awaiting their compiled message or notice decoder.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MessagePayload {
    rich_text: Option<Vec<u8>>,
    content: Option<Vec<u8>>,
    encrypted_content: Option<Vec<u8>>,
}

impl MessagePayload {
    pub(super) const fn new(
        rich_text: Option<Vec<u8>>,
        content: Option<Vec<u8>>,
        encrypted_content: Option<Vec<u8>>,
    ) -> Self {
        Self {
            rich_text,
            content,
            encrypted_content,
        }
    }

    /// Returns the encoded rich-text message when present.
    #[must_use]
    pub fn rich_text(&self) -> Option<&[u8]> {
        self.rich_text.as_deref()
    }

    /// Returns the raw notice or file content when present.
    #[must_use]
    pub fn content(&self) -> Option<&[u8]> {
        self.content.as_deref()
    }

    /// Returns the opaque encrypted message content when present.
    #[must_use]
    pub fn encrypted_content(&self) -> Option<&[u8]> {
        self.encrypted_content.as_deref()
    }
}

/// Validated outer message envelope and bounded undecoded payloads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageEnvelope {
    class: MessageClass,
    route: MessageRoute,
    payload: MessagePayload,
    sub_type: u32,
    sequence: u64,
    random: i64,
    timestamp: i64,
    package_count: i64,
    package_index: u32,
    division_sequence: u32,
    direct_message_sequence: u32,
    message_uid: u64,
}

impl MessageEnvelope {
    pub(super) const fn new(
        class: MessageClass,
        route: MessageRoute,
        payload: MessagePayload,
        metadata: MessageMetadata,
    ) -> Self {
        Self {
            class,
            route,
            payload,
            sub_type: metadata.sub_type,
            sequence: metadata.sequence,
            random: metadata.random,
            timestamp: metadata.timestamp,
            package_count: metadata.package_count,
            package_index: metadata.package_index,
            division_sequence: metadata.division_sequence,
            direct_message_sequence: metadata.direct_message_sequence,
            message_uid: metadata.message_uid,
        }
    }

    /// Returns the closed outer classification.
    #[must_use]
    pub const fn class(&self) -> MessageClass {
        self.class
    }

    /// Returns ordinary routing metadata.
    #[must_use]
    pub const fn route(&self) -> &MessageRoute {
        &self.route
    }

    /// Returns bounded raw sub-payloads.
    #[must_use]
    pub const fn payload(&self) -> &MessagePayload {
        &self.payload
    }

    /// Returns the outer message subtype, or zero when absent.
    #[must_use]
    pub const fn sub_type(&self) -> u32 {
        self.sub_type
    }

    /// Returns the outer message sequence, or zero when absent.
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Returns the signed outer random value, or zero when absent.
    #[must_use]
    pub const fn random(&self) -> i64 {
        self.random
    }

    /// Returns the signed Unix timestamp, or zero when absent.
    #[must_use]
    pub const fn timestamp(&self) -> i64 {
        self.timestamp
    }

    /// Returns the declared package count, or zero when absent.
    #[must_use]
    pub const fn package_count(&self) -> i64 {
        self.package_count
    }

    /// Returns the package index, or zero when absent.
    #[must_use]
    pub const fn package_index(&self) -> u32 {
        self.package_index
    }

    /// Returns the multipart division sequence, or zero when absent.
    #[must_use]
    pub const fn division_sequence(&self) -> u32 {
        self.division_sequence
    }

    /// Returns the direct-message sequence, or zero when absent.
    #[must_use]
    pub const fn direct_message_sequence(&self) -> u32 {
        self.direct_message_sequence
    }

    /// Returns the message unique identifier, or zero when absent.
    #[must_use]
    pub const fn message_uid(&self) -> u64 {
        self.message_uid
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct MessageMetadata {
    pub(super) sub_type: u32,
    pub(super) sequence: u64,
    pub(super) random: i64,
    pub(super) timestamp: i64,
    pub(super) package_count: i64,
    pub(super) package_index: u32,
    pub(super) division_sequence: u32,
    pub(super) direct_message_sequence: u32,
    pub(super) message_uid: u64,
}

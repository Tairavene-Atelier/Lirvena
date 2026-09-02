use crate::{AccountIdentity, EventHubError};

const FLAG_PREFIX: &str = "lr1_";
const FLAG_HEX_BYTES: usize = 32;
const MAX_COMMENT_BYTES: usize = 4_096;

/// Adapter-neutral group request kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GroupRequestKind {
    /// A user requested membership.
    Join,
    /// A user was invited by a group member.
    Invitation,
    /// The receiving account was invited by a group member.
    SelfInvitation,
}

/// Versioned opaque reference used to answer one group request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GroupRequestReference {
    sequence: u64,
    event_type: u32,
    group_id: u32,
}

impl GroupRequestReference {
    /// Creates a validated request reference.
    ///
    /// # Errors
    ///
    /// Returns an error for missing identity or an unsupported event type.
    pub fn new(sequence: u64, event_type: u32, group_id: u32) -> Result<Self, EventHubError> {
        if sequence == 0 || group_id == 0 || !matches!(event_type, 1 | 2 | 22) {
            return Err(EventHubError::InvalidEvent);
        }
        Ok(Self {
            sequence,
            event_type,
            group_id,
        })
    }

    /// Parses the canonical opaque `OneBot` flag.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown version or malformed reference.
    pub fn parse(flag: &str) -> Result<Self, EventHubError> {
        let hex = flag
            .strip_prefix(FLAG_PREFIX)
            .ok_or(EventHubError::InvalidEvent)?;
        if hex.len() != FLAG_HEX_BYTES {
            return Err(EventHubError::InvalidEvent);
        }
        let mut bytes = [0_u8; 16];
        for (index, slot) in bytes.iter_mut().enumerate() {
            let offset = index * 2;
            *slot = u8::from_str_radix(&hex[offset..offset + 2], 16)
                .map_err(|_error| EventHubError::InvalidEvent)?;
        }
        Self::new(
            u64::from_be_bytes(
                bytes[..8]
                    .try_into()
                    .map_err(|_error| EventHubError::InvalidEvent)?,
            ),
            u32::from_be_bytes(
                bytes[8..12]
                    .try_into()
                    .map_err(|_error| EventHubError::InvalidEvent)?,
            ),
            u32::from_be_bytes(
                bytes[12..]
                    .try_into()
                    .map_err(|_error| EventHubError::InvalidEvent)?,
            ),
        )
    }

    /// Returns the canonical opaque `OneBot` flag.
    #[must_use]
    pub fn flag(self) -> String {
        let mut bytes = [0_u8; 16];
        bytes[..8].copy_from_slice(&self.sequence.to_be_bytes());
        bytes[8..12].copy_from_slice(&self.event_type.to_be_bytes());
        bytes[12..].copy_from_slice(&self.group_id.to_be_bytes());
        let mut flag = String::with_capacity(FLAG_PREFIX.len() + FLAG_HEX_BYTES);
        flag.push_str(FLAG_PREFIX);
        for byte in bytes {
            use core::fmt::Write as _;
            let _written = write!(flag, "{byte:02x}");
        }
        flag
    }

    #[must_use]
    /// Returns QQ's server-issued request sequence.
    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    #[must_use]
    /// Returns the evidence-backed numeric request kind.
    pub const fn event_type(self) -> u32 {
        self.event_type
    }

    #[must_use]
    /// Returns the numeric QQ group identifier.
    pub const fn group_id(self) -> u32 {
        self.group_id
    }
}

/// One actionable group request after all Linux NT UIDs were resolved.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedGroupRequest {
    account: AccountIdentity,
    reference: GroupRequestReference,
    kind: GroupRequestKind,
    user_id: u64,
    inviter_id: Option<u64>,
    comment: String,
    occurred_at: u64,
}

impl ResolvedGroupRequest {
    /// Creates a bounded adapter-facing request.
    ///
    /// # Errors
    ///
    /// Returns an error for missing identity or unsafe comment text.
    pub fn new(
        account: AccountIdentity,
        reference: GroupRequestReference,
        kind: GroupRequestKind,
        user_id: u64,
        inviter_id: Option<u64>,
        comment: String,
        occurred_at: u64,
    ) -> Result<Self, EventHubError> {
        if user_id == 0
            || inviter_id == Some(0)
            || comment.len() > MAX_COMMENT_BYTES
            || comment.chars().any(char::is_control)
        {
            return Err(EventHubError::InvalidEvent);
        }
        Ok(Self {
            account,
            reference,
            kind,
            user_id,
            inviter_id,
            comment,
            occurred_at,
        })
    }

    #[must_use]
    /// Returns the receiving account identity.
    pub const fn account(&self) -> &AccountIdentity {
        &self.account
    }
    #[must_use]
    /// Returns the actionable request reference.
    pub const fn reference(&self) -> GroupRequestReference {
        self.reference
    }
    #[must_use]
    /// Returns the adapter-neutral request kind.
    pub const fn kind(&self) -> GroupRequestKind {
        self.kind
    }
    #[must_use]
    /// Returns the numeric QQ user identifier projected for `OneBot`.
    pub const fn user_id(&self) -> u64 {
        self.user_id
    }
    #[must_use]
    /// Returns the resolved inviter when QQ supplied it.
    pub const fn inviter_id(&self) -> Option<u64> {
        self.inviter_id
    }
    #[must_use]
    /// Returns the bounded QQ request comment.
    pub fn comment(&self) -> &str {
        &self.comment
    }
    #[must_use]
    /// Returns the QQ-supplied Unix event time in seconds, or zero when absent.
    pub const fn occurred_at(&self) -> u64 {
        self.occurred_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_flag_round_trips_and_rejects_other_versions() -> Result<(), EventHubError> {
        let reference = GroupRequestReference::new(77, 22, 12345)?;
        assert_eq!(GroupRequestReference::parse(&reference.flag())?, reference);
        assert!(GroupRequestReference::parse("lr2_00000000000000000000000000000000").is_err());
        Ok(())
    }
}

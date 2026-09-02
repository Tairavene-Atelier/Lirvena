use crate::{AccountIdentity, EventHubError};

const FLAG_PREFIX: &str = "lf1_";
const MAX_UID_BYTES: usize = 128;
const MAX_COMMENT_BYTES: usize = 4_096;

/// Versioned opaque reference used to answer one friend request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FriendRequestReference {
    source_uid: String,
}

impl FriendRequestReference {
    /// Creates a validated friend-request reference.
    ///
    /// # Errors
    ///
    /// Returns an error for an unsafe Linux NT UID.
    pub fn new(source_uid: String) -> Result<Self, EventHubError> {
        validate_uid(&source_uid)?;
        Ok(Self { source_uid })
    }

    /// Parses the canonical opaque `OneBot` flag.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown version, malformed hex, or unsafe UID.
    pub fn parse(flag: &str) -> Result<Self, EventHubError> {
        let encoded = flag
            .strip_prefix(FLAG_PREFIX)
            .ok_or(EventHubError::InvalidEvent)?;
        if encoded.is_empty() || encoded.len() % 2 != 0 || encoded.len() > MAX_UID_BYTES * 2 {
            return Err(EventHubError::InvalidEvent);
        }
        let mut bytes = Vec::with_capacity(encoded.len() / 2);
        for offset in (0..encoded.len()).step_by(2) {
            bytes.push(
                u8::from_str_radix(&encoded[offset..offset + 2], 16)
                    .map_err(|_error| EventHubError::InvalidEvent)?,
            );
        }
        let source_uid = String::from_utf8(bytes).map_err(|_error| EventHubError::InvalidEvent)?;
        Self::new(source_uid)
    }

    /// Returns the canonical opaque `OneBot` flag.
    #[must_use]
    pub fn flag(&self) -> String {
        let mut flag = String::with_capacity(FLAG_PREFIX.len() + self.source_uid.len() * 2);
        flag.push_str(FLAG_PREFIX);
        for byte in self.source_uid.as_bytes() {
            use core::fmt::Write as _;
            let _written = write!(flag, "{byte:02x}");
        }
        flag
    }

    /// Returns the authenticated Linux NT UID needed by QQ's decision route.
    #[must_use]
    pub fn source_uid(&self) -> &str {
        &self.source_uid
    }
}

/// One actionable friend request recovered from QQ's authenticated directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFriendRequest {
    account: AccountIdentity,
    reference: FriendRequestReference,
    user_id: u64,
    comment: String,
    occurred_at: u64,
}

impl ResolvedFriendRequest {
    /// Creates a bounded adapter-facing friend request.
    ///
    /// # Errors
    ///
    /// Returns an error for missing identity or unsafe text.
    pub fn new(
        account: AccountIdentity,
        reference: FriendRequestReference,
        user_id: u64,
        comment: String,
        occurred_at: u64,
    ) -> Result<Self, EventHubError> {
        if user_id == 0
            || comment.len() > MAX_COMMENT_BYTES
            || comment.chars().any(char::is_control)
        {
            return Err(EventHubError::InvalidEvent);
        }
        Ok(Self {
            account,
            reference,
            user_id,
            comment,
            occurred_at,
        })
    }

    /// Returns the receiving account identity.
    #[must_use]
    pub const fn account(&self) -> &AccountIdentity {
        &self.account
    }

    /// Returns the actionable request reference.
    #[must_use]
    pub const fn reference(&self) -> &FriendRequestReference {
        &self.reference
    }

    /// Returns the resolved numeric QQ identifier.
    #[must_use]
    pub const fn user_id(&self) -> u64 {
        self.user_id
    }

    /// Returns the bounded request comment.
    #[must_use]
    pub fn comment(&self) -> &str {
        &self.comment
    }

    /// Returns the QQ-supplied Unix event time in seconds, or zero when absent.
    #[must_use]
    pub const fn occurred_at(&self) -> u64 {
        self.occurred_at
    }
}

fn validate_uid(uid: &str) -> Result<(), EventHubError> {
    if uid.is_empty() || uid.len() > MAX_UID_BYTES || uid.chars().any(char::is_control) {
        Err(EventHubError::InvalidEvent)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_round_trips_without_exposing_raw_uid() -> Result<(), EventHubError> {
        let reference = FriendRequestReference::new("u_friend".to_owned())?;
        let flag = reference.flag();
        assert!(!flag.contains("u_friend"));
        assert_eq!(FriendRequestReference::parse(&flag)?, reference);
        assert!(FriendRequestReference::parse("lf2_00").is_err());
        Ok(())
    }
}

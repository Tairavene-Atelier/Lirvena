use prost::Message;

use crate::MediaError;

/// QQ account or group avatar upload destination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AvatarTarget {
    /// The authenticated QQ account.
    Account,
    /// One non-zero group number.
    Group(u32),
}

/// Frozen Highway inputs for one avatar upload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AvatarUpload {
    command_id: u32,
    extension: Vec<u8>,
}

impl AvatarUpload {
    /// Audited Highway command identifier.
    #[must_use]
    pub const fn command_id(&self) -> u32 {
        self.command_id
    }

    /// Audited command-specific extension.
    #[must_use]
    pub fn extension(&self) -> &[u8] {
        &self.extension
    }
}

/// Builds the frozen Linux NTQQ avatar-upload inputs.
///
/// # Errors
///
/// Returns an error for a zero group number.
pub fn avatar_upload(target: AvatarTarget) -> Result<AvatarUpload, MediaError> {
    match target {
        AvatarTarget::Account => Ok(AvatarUpload {
            command_id: 90,
            extension: Vec::new(),
        }),
        AvatarTarget::Group(group_uin) if group_uin != 0 => Ok(AvatarUpload {
            command_id: 3_000,
            extension: GroupAvatarExtension {
                kind: 101,
                group_uin,
                options: Some(GroupAvatarOptions { enabled: 1 }),
                field_five: 3,
                field_six: 1,
            }
            .encode_to_vec(),
        }),
        AvatarTarget::Group(_) => Err(MediaError::ReferenceRejected),
    }
}

#[derive(Clone, Copy, PartialEq, Message)]
struct GroupAvatarExtension {
    #[prost(uint32, tag = "1")]
    kind: u32,
    #[prost(uint32, tag = "2")]
    group_uin: u32,
    #[prost(message, optional, tag = "3")]
    options: Option<GroupAvatarOptions>,
    #[prost(uint32, tag = "5")]
    field_five: u32,
    #[prost(uint32, tag = "6")]
    field_six: u32,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct GroupAvatarOptions {
    #[prost(uint32, tag = "1")]
    enabled: u32,
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::{AvatarTarget, GroupAvatarExtension, avatar_upload};

    #[test]
    fn account_and_group_inputs_match_frozen_highway_shape() -> Result<(), crate::MediaError> {
        let account = avatar_upload(AvatarTarget::Account)?;
        assert_eq!(account.command_id(), 90);
        assert!(account.extension().is_empty());

        let group = avatar_upload(AvatarTarget::Group(123_456))?;
        assert_eq!(group.command_id(), 3_000);
        let extension = GroupAvatarExtension::decode(group.extension())
            .map_err(|_error| crate::MediaError::ReferenceRejected)?;
        let options = extension
            .options
            .ok_or(crate::MediaError::ReferenceRejected)?;
        assert_eq!(extension.kind, 101);
        assert_eq!(extension.group_uin, 123_456);
        assert_eq!(options.enabled, 1);
        assert_eq!((extension.field_five, extension.field_six), (3, 1));
        Ok(())
    }

    #[test]
    fn zero_group_fails_closed() {
        assert!(avatar_upload(AvatarTarget::Group(0)).is_err());
    }
}

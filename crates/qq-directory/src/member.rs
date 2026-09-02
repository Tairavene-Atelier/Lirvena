use prost::Message;

use crate::{GroupDirectoryError, fields::ProtobufBoolFields};

const MAX_MEMBERS_PER_PAGE: usize = 5_000;
const MAX_TOKEN_BYTES: usize = 4_096;
const MAX_UID_BYTES: usize = 128;
const MAX_TEXT_BYTES: usize = 4_096;

/// QQ group member role exposed through the account API.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GroupMemberRole {
    /// Ordinary group member.
    Member,
    /// Group owner.
    Owner,
    /// Group administrator.
    Admin,
}

/// One bounded QQ group member directory entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupMember {
    /// Numeric QQ identifier.
    pub uin: u32,
    /// Linux NT UID used by member-targeted operations.
    pub uid: String,
    /// Current display nickname.
    pub nickname: String,
    /// Group-specific card.
    pub card: String,
    /// Group level number.
    pub level: u32,
    /// Member role.
    pub role: GroupMemberRole,
    /// Special title.
    pub special_title: String,
    /// Unix join timestamp.
    pub joined_at: u32,
    /// Unix timestamp of the last observed message.
    pub last_message_at: u32,
    /// Unix timestamp until which the member is muted.
    pub muted_until: u32,
}

/// One validated member-directory page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupMemberPage {
    /// Members in QQ server order.
    pub members: Vec<GroupMember>,
    /// Opaque continuation token.
    pub next_token: Option<String>,
}

/// Encodes one Linux NT group-member page request.
///
/// # Errors
///
/// Returns an error for a zero group identifier or an unsafe continuation token.
pub fn encode_group_member_page_request(
    group_id: u32,
    token: Option<&str>,
) -> Result<Vec<u8>, GroupDirectoryError> {
    if group_id == 0 || token.is_some_and(|value| !valid_text(value, MAX_TOKEN_BYTES)) {
        return Err(GroupDirectoryError);
    }
    Ok(OidbEnvelope {
        command: 0x0fe7,
        subcommand: 3,
        error_code: 0,
        body: Some(MemberRequest {
            group_id,
            field2: 5,
            field3: 2,
            fields: Some(member_fields()),
            token: token.map(str::to_owned),
        }),
    }
    .encode_to_vec())
}

/// Parses one Linux NT group-member page response.
///
/// # Errors
///
/// Returns an error for rejected, malformed, excessive, duplicate or unsafe member data.
pub fn parse_group_member_page(input: &[u8]) -> Result<GroupMemberPage, GroupDirectoryError> {
    let envelope = OidbResponse::decode(input).map_err(|_error| GroupDirectoryError)?;
    if envelope.error_code != 0 {
        return Err(GroupDirectoryError);
    }
    let body = envelope.body.ok_or(GroupDirectoryError)?;
    if body.members.len() > MAX_MEMBERS_PER_PAGE
        || body
            .token
            .as_deref()
            .is_some_and(|value| !valid_text(value, MAX_TOKEN_BYTES))
    {
        return Err(GroupDirectoryError);
    }
    let mut seen = std::collections::BTreeSet::new();
    let members = body
        .members
        .into_iter()
        .map(|raw| {
            let identity = raw.identity.ok_or(GroupDirectoryError)?;
            let card = raw.card.map(|value| value.value).unwrap_or_default();
            let role = match raw.permission {
                0 => GroupMemberRole::Member,
                1 => GroupMemberRole::Owner,
                2 => GroupMemberRole::Admin,
                _ => return Err(GroupDirectoryError),
            };
            if identity.uin == 0
                || !seen.insert(identity.uin)
                || identity.uid.is_empty()
                || !valid_text(&identity.uid, MAX_UID_BYTES)
                || !valid_text(&raw.nickname, MAX_TEXT_BYTES)
                || !valid_text(&card, MAX_TEXT_BYTES)
                || !valid_text(
                    raw.special_title.as_deref().unwrap_or_default(),
                    MAX_TEXT_BYTES,
                )
            {
                return Err(GroupDirectoryError);
            }
            Ok(GroupMember {
                uin: identity.uin,
                uid: identity.uid,
                nickname: raw.nickname,
                card,
                level: raw.level.map(|value| value.value).unwrap_or_default(),
                role,
                special_title: raw.special_title.unwrap_or_default(),
                joined_at: raw.joined_at,
                last_message_at: raw.last_message_at,
                muted_until: raw.muted_until.unwrap_or_default(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(GroupMemberPage {
        members,
        next_token: body.token.filter(|value| !value.is_empty()),
    })
}

fn valid_text(value: &str, max: usize) -> bool {
    value.len() <= max && !value.chars().any(char::is_control)
}

#[derive(Clone, PartialEq, Message)]
struct OidbEnvelope {
    #[prost(uint32, tag = "1")]
    command: u32,
    #[prost(uint32, tag = "2")]
    subcommand: u32,
    #[prost(uint32, tag = "3")]
    error_code: u32,
    #[prost(message, optional, tag = "4")]
    body: Option<MemberRequest>,
}

#[derive(Clone, PartialEq, Message)]
struct MemberRequest {
    #[prost(uint32, tag = "1")]
    group_id: u32,
    #[prost(uint32, tag = "2")]
    field2: u32,
    #[prost(uint32, tag = "3")]
    field3: u32,
    #[prost(message, optional, tag = "4")]
    fields: Option<ProtobufBoolFields>,
    #[prost(string, optional, tag = "15")]
    token: Option<String>,
}

fn member_fields() -> ProtobufBoolFields {
    ProtobufBoolFields::enabled([
        10, 11, 12, 13, 16, 17, 18, 20, 21, 100, 101, 102, 103, 104, 105, 106, 107, 200, 201,
    ])
}

#[derive(Clone, PartialEq, Message)]
struct OidbResponse {
    #[prost(uint32, tag = "3")]
    error_code: u32,
    #[prost(message, optional, tag = "4")]
    body: Option<MemberResponse>,
}

#[derive(Clone, PartialEq, Message)]
struct MemberResponse {
    #[prost(message, repeated, tag = "2")]
    members: Vec<RawMember>,
    #[prost(string, optional, tag = "15")]
    token: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
struct RawMember {
    #[prost(message, optional, tag = "1")]
    identity: Option<MemberIdentity>,
    #[prost(string, tag = "10")]
    nickname: String,
    #[prost(message, optional, tag = "11")]
    card: Option<MemberCard>,
    #[prost(message, optional, tag = "12")]
    level: Option<MemberLevel>,
    #[prost(string, optional, tag = "17")]
    special_title: Option<String>,
    #[prost(uint32, tag = "100")]
    joined_at: u32,
    #[prost(uint32, tag = "101")]
    last_message_at: u32,
    #[prost(uint32, optional, tag = "102")]
    muted_until: Option<u32>,
    #[prost(uint32, tag = "107")]
    permission: u32,
}

#[derive(Clone, PartialEq, Message)]
struct MemberIdentity {
    #[prost(string, tag = "2")]
    uid: String,
    #[prost(uint32, tag = "4")]
    uin: u32,
}

#[derive(Clone, PartialEq, Message)]
struct MemberCard {
    #[prost(string, tag = "2")]
    value: String,
}

#[derive(Clone, PartialEq, Message)]
struct MemberLevel {
    #[prost(uint32, tag = "2")]
    value: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_carries_group_token_and_all_fields() -> Result<(), Box<dyn std::error::Error>> {
        let bytes = encode_group_member_page_request(12_345, Some("next-page"))?;
        let envelope = OidbEnvelope::decode(bytes.as_slice())?;
        assert_eq!((envelope.command, envelope.subcommand), (0x0fe7, 3));
        let body = envelope.body.ok_or(GroupDirectoryError)?;
        assert_eq!(body.group_id, 12_345);
        assert_eq!(body.token.as_deref(), Some("next-page"));
        assert_eq!(body.fields, Some(member_fields()));
        Ok(())
    }

    #[test]
    fn response_maps_member_identity_and_role() {
        let response = OidbResponse {
            error_code: 0,
            body: Some(MemberResponse {
                members: vec![RawMember {
                    identity: Some(MemberIdentity {
                        uid: "u_test".to_owned(),
                        uin: 10_001,
                    }),
                    nickname: "member".to_owned(),
                    card: Some(MemberCard {
                        value: "card".to_owned(),
                    }),
                    level: Some(MemberLevel { value: 6 }),
                    special_title: Some("title".to_owned()),
                    joined_at: 11,
                    last_message_at: 12,
                    muted_until: Some(13),
                    permission: 2,
                }],
                token: Some("next".to_owned()),
            }),
        }
        .encode_to_vec();

        assert_eq!(
            parse_group_member_page(&response),
            Ok(GroupMemberPage {
                members: vec![GroupMember {
                    uin: 10_001,
                    uid: "u_test".to_owned(),
                    nickname: "member".to_owned(),
                    card: "card".to_owned(),
                    level: 6,
                    role: GroupMemberRole::Admin,
                    special_title: "title".to_owned(),
                    joined_at: 11,
                    last_message_at: 12,
                    muted_until: 13,
                }],
                next_token: Some("next".to_owned()),
            })
        );
    }
}

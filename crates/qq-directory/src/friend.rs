use prost::Message;

const MAX_FRIENDS_PER_PAGE: usize = 300;
const REQUEST_FRIEND_COUNT: u32 = 300;
const MAX_UID_BYTES: usize = 128;
const MAX_TEXT_BYTES: usize = 1_024;

/// One bounded QQ friend directory entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FriendEntry {
    /// Numeric QQ identifier.
    pub uin: u32,
    /// Current Linux NT UID used by direct-message routing.
    pub uid: String,
    /// Display nickname.
    pub nickname: String,
    /// User-defined remark.
    pub remark: String,
    /// Public signature.
    pub signature: String,
    /// Optional QID.
    pub qid: String,
}

/// One validated page of the QQ friend directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FriendPage {
    /// Entries returned in server order.
    pub friends: Vec<FriendEntry>,
    /// Continuation UIN when another page is available.
    pub next_uin: Option<u32>,
}

/// Encodes one Linux NT friend-directory page request.
#[must_use]
pub fn encode_friend_page_request(next_uin: Option<u32>) -> Vec<u8> {
    OidbEnvelope {
        command: 0x0fd4,
        subcommand: 1,
        error_code: 0,
        body: Some(FriendRequest {
            friend_count: REQUEST_FRIEND_COUNT,
            next: next_uin.map(|uin| NextUin { uin }),
            field6: 1,
            field7: i32::MAX as u32,
            properties: vec![
                PropertyRequest {
                    kind: 1,
                    numbers: Some(PropertyNumbers {
                        values: vec![103, 102, 20_002, 27_394],
                    }),
                },
                PropertyRequest {
                    kind: 4,
                    numbers: Some(PropertyNumbers {
                        values: vec![100, 101, 102],
                    }),
                },
            ],
            field10002: vec![13_578, 13_579, 13_573, 13_572, 13_568],
            field10003: 4_051,
        }),
    }
    .encode_to_vec()
}

/// Parses one Linux NT friend-directory page response.
///
/// # Errors
///
/// Returns an error for a rejected OIDB response, malformed or excessive entries, duplicate UINs,
/// invalid UID/text fields, or missing required property layer 1.
pub fn parse_friend_page(input: &[u8]) -> Result<FriendPage, FriendDirectoryError> {
    let envelope = OidbResponseEnvelope::decode(input).map_err(|_error| FriendDirectoryError)?;
    if envelope.error_code != 0 {
        return Err(FriendDirectoryError);
    }
    let body = envelope.body.ok_or(FriendDirectoryError)?;
    if body.friends.len() > MAX_FRIENDS_PER_PAGE {
        return Err(FriendDirectoryError);
    }
    let mut seen = std::collections::BTreeSet::new();
    let friends = body
        .friends
        .into_iter()
        .map(|raw| {
            if raw.uin == 0
                || !seen.insert(raw.uin)
                || !valid_text(&raw.uid, MAX_UID_BYTES)
                || raw.uid.is_empty()
            {
                return Err(FriendDirectoryError);
            }
            let layer = raw
                .additional
                .into_iter()
                .find(|value| value.kind == 1)
                .and_then(|value| value.layer)
                .ok_or(FriendDirectoryError)?;
            let mut properties = std::collections::BTreeMap::new();
            for property in layer.properties {
                if !valid_text(&property.value, MAX_TEXT_BYTES)
                    || properties.insert(property.code, property.value).is_some()
                {
                    return Err(FriendDirectoryError);
                }
            }
            Ok(FriendEntry {
                uin: raw.uin,
                uid: raw.uid,
                nickname: property_or_default(&properties, 20_002, raw.uin.to_string()),
                remark: property_or_default(&properties, 103, String::new()),
                signature: property_or_default(&properties, 102, String::new()),
                qid: property_or_default(&properties, 27_394, String::new()),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(FriendPage {
        friends,
        next_uin: body.next.map(|value| value.uin).filter(|value| *value != 0),
    })
}

fn property_or_default(
    properties: &std::collections::BTreeMap<u32, String>,
    code: u32,
    default: String,
) -> String {
    properties.get(&code).cloned().unwrap_or(default)
}

fn valid_text(value: &str, max: usize) -> bool {
    value.len() <= max && !value.chars().any(char::is_control)
}

/// Opaque friend-directory codec error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FriendDirectoryError;

impl core::fmt::Display for FriendDirectoryError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("QQ friend directory data is invalid")
    }
}

impl std::error::Error for FriendDirectoryError {}

#[derive(Clone, PartialEq, Message)]
struct OidbEnvelope {
    #[prost(uint32, tag = "1")]
    command: u32,
    #[prost(uint32, tag = "2")]
    subcommand: u32,
    #[prost(uint32, tag = "3")]
    error_code: u32,
    #[prost(message, optional, tag = "4")]
    body: Option<FriendRequest>,
}

#[derive(Clone, PartialEq, Message)]
struct FriendRequest {
    #[prost(uint32, tag = "2")]
    friend_count: u32,
    #[prost(message, optional, tag = "5")]
    next: Option<NextUin>,
    #[prost(uint32, tag = "6")]
    field6: u32,
    #[prost(uint32, tag = "7")]
    field7: u32,
    #[prost(message, repeated, tag = "10001")]
    properties: Vec<PropertyRequest>,
    #[prost(uint32, repeated, packed = "true", tag = "10002")]
    field10002: Vec<u32>,
    #[prost(uint32, tag = "10003")]
    field10003: u32,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct NextUin {
    #[prost(uint32, tag = "1")]
    uin: u32,
}

#[derive(Clone, PartialEq, Message)]
struct PropertyRequest {
    #[prost(uint32, tag = "1")]
    kind: u32,
    #[prost(message, optional, tag = "2")]
    numbers: Option<PropertyNumbers>,
}

#[derive(Clone, PartialEq, Message)]
struct PropertyNumbers {
    #[prost(uint32, repeated, packed = "true", tag = "1")]
    values: Vec<u32>,
}

#[derive(Clone, PartialEq, Message)]
struct OidbResponseEnvelope {
    #[prost(uint32, tag = "3")]
    error_code: u32,
    #[prost(message, optional, tag = "4")]
    body: Option<FriendResponse>,
}

#[derive(Clone, PartialEq, Message)]
struct FriendResponse {
    #[prost(message, optional, tag = "2")]
    next: Option<NextUin>,
    #[prost(message, repeated, tag = "101")]
    friends: Vec<RawFriend>,
}

#[derive(Clone, PartialEq, Message)]
struct RawFriend {
    #[prost(string, tag = "1")]
    uid: String,
    #[prost(uint32, tag = "3")]
    uin: u32,
    #[prost(message, repeated, tag = "10001")]
    additional: Vec<FriendAdditional>,
}

#[derive(Clone, PartialEq, Message)]
struct FriendAdditional {
    #[prost(uint32, tag = "1")]
    kind: u32,
    #[prost(message, optional, tag = "2")]
    layer: Option<FriendLayer>,
}

#[derive(Clone, PartialEq, Message)]
struct FriendLayer {
    #[prost(message, repeated, tag = "2")]
    properties: Vec<FriendProperty>,
}

#[derive(Clone, PartialEq, Message)]
struct FriendProperty {
    #[prost(uint32, tag = "1")]
    code: u32,
    #[prost(string, tag = "2")]
    value: String,
}

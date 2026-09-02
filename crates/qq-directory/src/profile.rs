use prost::Message;

use crate::UserDirectoryError;
use crate::user_wire::{UserSelector, decode_user_response, encode_user_request};

const NICKNAME: u32 = 20_002;
const GENDER: u32 = 20_009;
const AGE: u32 = 20_037;
const QID: u32 = 27_394;
const SIGNATURE: u32 = 102;
const LEVEL: u32 = 105;
const REGISTERED_AT: u32 = 20_026;
const AVATAR: u32 = 101;
const PROFILE_KEYS: [u32; 8] = [
    NICKNAME,
    GENDER,
    AGE,
    QID,
    SIGNATURE,
    LEVEL,
    REGISTERED_AT,
    AVATAR,
];

/// Public QQ gender value projected into `OneBot`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UserGender {
    /// QQ reports male.
    Male,
    /// QQ reports female.
    Female,
    /// QQ omitted the field or returned another value.
    Unknown,
}

/// Bounded public QQ profile used by directory actions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserProfile {
    /// Numeric QQ identifier returned by the directory.
    pub uin: u32,
    /// Current public nickname.
    pub nickname: String,
    /// Public gender value.
    pub gender: UserGender,
    /// Public age value, or zero when omitted.
    pub age: u32,
    /// Optional public QID.
    pub qid: Option<String>,
    /// Optional public signature.
    pub signature: Option<String>,
    /// Public QQ level, or zero when omitted.
    pub level: u32,
    /// Registration Unix timestamp, or zero when omitted.
    pub registered_at: u32,
    /// Optional public avatar URL.
    pub avatar_url: Option<String>,
}

/// Encodes one bounded public-profile lookup by numeric QQ identifier.
///
/// # Errors
///
/// Returns an error for a zero identifier or an invalid compiled property set.
pub fn encode_user_profile_request(uin: u32) -> Result<Vec<u8>, UserDirectoryError> {
    encode_user_request(UserSelector::Uin(uin), &PROFILE_KEYS)
}

/// Parses one bounded public-profile response.
///
/// # Errors
///
/// Returns an error for a rejected or malformed response, missing identity/nickname, duplicate
/// properties, invalid UTF-8, or excessive public values.
pub fn parse_user_profile(input: &[u8]) -> Result<UserProfile, UserDirectoryError> {
    let mut raw = decode_user_response(input)?;
    let nickname = take_text(&mut raw.bytes, NICKNAME, 512, false)?.ok_or(UserDirectoryError)?;
    let qid = take_text(&mut raw.bytes, QID, 64, true)?;
    let signature = take_text(&mut raw.bytes, SIGNATURE, 1_024, true)?;
    let avatar_url = raw
        .bytes
        .remove(&AVATAR)
        .map(|encoded| parse_avatar(&encoded))
        .transpose()?;
    let age = raw.numbers.get(&AGE).copied().unwrap_or(0);
    if age > 200 {
        return Err(UserDirectoryError);
    }
    let gender = match raw.numbers.get(&GENDER).copied().unwrap_or(0) {
        1 => UserGender::Male,
        2 => UserGender::Female,
        _ => UserGender::Unknown,
    };
    Ok(UserProfile {
        uin: raw.uin,
        nickname,
        gender,
        age,
        qid,
        signature,
        level: raw.numbers.get(&LEVEL).copied().unwrap_or(0),
        registered_at: raw.numbers.get(&REGISTERED_AT).copied().unwrap_or(0),
        avatar_url,
    })
}

fn take_text(
    values: &mut std::collections::BTreeMap<u32, Vec<u8>>,
    key: u32,
    maximum: usize,
    allow_empty: bool,
) -> Result<Option<String>, UserDirectoryError> {
    let Some(bytes) = values.remove(&key) else {
        return Ok(None);
    };
    if bytes.len() > maximum {
        return Err(UserDirectoryError);
    }
    let value = String::from_utf8(bytes).map_err(|_error| UserDirectoryError)?;
    if (!allow_empty && value.is_empty()) || value.chars().any(|character| character == '\0') {
        return Err(UserDirectoryError);
    }
    if value.is_empty() {
        Ok(None)
    } else {
        Ok(Some(value))
    }
}

fn parse_avatar(input: &[u8]) -> Result<String, UserDirectoryError> {
    let avatar = Avatar::decode(input).map_err(|_error| UserDirectoryError)?;
    let base = avatar.url.ok_or(UserDirectoryError)?;
    if base.is_empty() || base.len() > 2_048 || base.chars().any(char::is_control) {
        return Err(UserDirectoryError);
    }
    Ok(format!("{base}640"))
}

#[derive(Clone, PartialEq, Message)]
struct Avatar {
    #[prost(string, optional, tag = "5")]
    url: Option<String>,
}

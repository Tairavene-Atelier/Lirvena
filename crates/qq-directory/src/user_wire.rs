use std::collections::{BTreeMap, BTreeSet};

use prost::Message;
use qq_wire::{decode_oidb_response, encode_oidb_request};

use crate::UserDirectoryError;

const MAX_UID_BYTES: usize = 128;
const MAX_PROPERTIES: usize = 128;
const MAX_PROPERTY_BYTES: usize = 4 * 1024;
const MAX_TOTAL_PROPERTY_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy)]
pub(crate) enum UserSelector<'a> {
    Uid(&'a str),
    Uin(u32),
}

pub(crate) struct RawUser {
    pub(crate) uin: u32,
    pub(crate) numbers: BTreeMap<u32, u32>,
    pub(crate) bytes: BTreeMap<u32, Vec<u8>>,
}

pub(crate) fn encode_user_request(
    selector: UserSelector<'_>,
    property_keys: &[u32],
) -> Result<Vec<u8>, UserDirectoryError> {
    let keys = checked_keys(property_keys)?;
    let (body, reserved) = match selector {
        UserSelector::Uid(uid)
            if !uid.is_empty()
                && uid.len() <= MAX_UID_BYTES
                && !uid.chars().any(char::is_control) =>
        {
            (
                UserRequestByUid {
                    uid: uid.to_owned(),
                    field2: 0,
                    properties: keys,
                }
                .encode_to_vec(),
                0,
            )
        }
        UserSelector::Uin(uin) if uin != 0 => (
            UserRequestByUin {
                uin,
                field2: 0,
                properties: keys,
            }
            .encode_to_vec(),
            1,
        ),
        UserSelector::Uid(_) | UserSelector::Uin(_) => return Err(UserDirectoryError),
    };
    encode_oidb_request(0x0fe1, 2, &body, reserved).map_err(|_error| UserDirectoryError)
}

pub(crate) fn decode_user_response(input: &[u8]) -> Result<RawUser, UserDirectoryError> {
    let outer = decode_oidb_response(input).map_err(|_error| UserDirectoryError)?;
    if outer.error_code() != 0 {
        return Err(UserDirectoryError);
    }
    let response = UserResponse::decode(outer.body()).map_err(|_error| UserDirectoryError)?;
    let user = response.user.ok_or(UserDirectoryError)?;
    if user.uin == 0 {
        return Err(UserDirectoryError);
    }
    let properties = user.properties.unwrap_or_default();
    if properties.numbers.len() > MAX_PROPERTIES || properties.bytes.len() > MAX_PROPERTIES {
        return Err(UserDirectoryError);
    }
    let mut numbers = BTreeMap::new();
    for property in properties.numbers {
        if property.key == 0 || numbers.insert(property.key, property.value).is_some() {
            return Err(UserDirectoryError);
        }
    }
    let mut bytes = BTreeMap::new();
    let mut total_bytes = 0usize;
    for property in properties.bytes {
        total_bytes = total_bytes
            .checked_add(property.value.len())
            .ok_or(UserDirectoryError)?;
        if property.key == 0
            || property.value.len() > MAX_PROPERTY_BYTES
            || total_bytes > MAX_TOTAL_PROPERTY_BYTES
            || bytes.insert(property.key, property.value).is_some()
        {
            return Err(UserDirectoryError);
        }
    }
    if numbers.keys().any(|key| bytes.contains_key(key)) {
        return Err(UserDirectoryError);
    }
    Ok(RawUser {
        uin: user.uin,
        numbers,
        bytes,
    })
}

fn checked_keys(property_keys: &[u32]) -> Result<Vec<PropertyRequest>, UserDirectoryError> {
    if property_keys.is_empty() || property_keys.len() > MAX_PROPERTIES {
        return Err(UserDirectoryError);
    }
    let mut seen = BTreeSet::new();
    property_keys
        .iter()
        .copied()
        .map(|key| {
            if key == 0 || !seen.insert(key) {
                return Err(UserDirectoryError);
            }
            Ok(PropertyRequest { key })
        })
        .collect()
}

#[derive(Clone, PartialEq, Message)]
struct UserRequestByUid {
    #[prost(string, tag = "1")]
    uid: String,
    #[prost(uint32, tag = "2")]
    field2: u32,
    #[prost(message, repeated, tag = "3")]
    properties: Vec<PropertyRequest>,
}

#[derive(Clone, PartialEq, Message)]
struct UserRequestByUin {
    #[prost(uint32, tag = "1")]
    uin: u32,
    #[prost(uint32, tag = "2")]
    field2: u32,
    #[prost(message, repeated, tag = "3")]
    properties: Vec<PropertyRequest>,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct PropertyRequest {
    #[prost(uint32, tag = "1")]
    key: u32,
}

#[derive(Clone, PartialEq, Message)]
struct UserResponse {
    #[prost(message, optional, tag = "1")]
    user: Option<UserResponseBody>,
}

#[derive(Clone, PartialEq, Message)]
struct UserResponseBody {
    #[prost(message, optional, tag = "2")]
    properties: Option<UserProperties>,
    #[prost(uint32, tag = "3")]
    uin: u32,
}

#[derive(Clone, PartialEq, Message)]
struct UserProperties {
    #[prost(message, repeated, tag = "1")]
    numbers: Vec<NumberProperty>,
    #[prost(message, repeated, tag = "2")]
    bytes: Vec<BytesProperty>,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct NumberProperty {
    #[prost(uint32, tag = "1")]
    key: u32,
    #[prost(uint32, tag = "2")]
    value: u32,
}

#[derive(Clone, PartialEq, Message)]
struct BytesProperty {
    #[prost(uint32, tag = "1")]
    key: u32,
    #[prost(bytes = "vec", tag = "2")]
    value: Vec<u8>,
}

use prost::Message;

use crate::MessageDecodeError;

const MAX_EMOJI_ID_BYTES: usize = 128;
const MAX_KEY_BYTES: usize = 4 * 1024;
const MAX_SUMMARY_BYTES: usize = 1024;

/// Evidence-backed QQ marketplace-face metadata.
#[derive(Clone, Eq, PartialEq)]
pub struct MarketFaceSegment {
    emoji_id: String,
    package_id: i32,
    key: String,
    summary: String,
}

impl core::fmt::Debug for MarketFaceSegment {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("MarketFaceSegment")
            .field("emoji_id_len", &self.emoji_id.len())
            .field("package_id", &self.package_id)
            .field("key_len", &self.key.len())
            .field("summary", &self.summary)
            .finish()
    }
}

impl MarketFaceSegment {
    /// Lowercase hexadecimal marketplace emoji identifier.
    #[must_use]
    pub fn emoji_id(&self) -> &str {
        &self.emoji_id
    }

    /// QQ marketplace package identifier.
    #[must_use]
    pub const fn package_id(&self) -> i32 {
        self.package_id
    }

    /// QQ-provided marketplace lookup key.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Human-readable marketplace face summary.
    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }

    /// Lagrange-compatible public asset URL derived from the validated emoji identifier.
    #[must_use]
    pub fn asset_url(&self) -> String {
        format!(
            "https://gxh.vip.qq.com/club/item/parcel/item/{}/{}/raw300.gif",
            &self.emoji_id[..2],
            self.emoji_id
        )
    }
}

pub(crate) fn encode_market_face(
    emoji_id: &str,
    package_id: i32,
    key: &str,
    summary: &str,
) -> Result<Vec<u8>, MessageDecodeError> {
    let face_id = decode_hex(emoji_id)?;
    if package_id <= 0 || !valid_text(key, MAX_KEY_BYTES) || !valid_text(summary, MAX_SUMMARY_BYTES)
    {
        return Err(MessageDecodeError);
    }
    Ok(MarketFaceWire {
        summary: summary.to_owned(),
        item_type: 6,
        info: 1,
        face_id,
        package_id,
        subtype: 3,
        key: key.to_owned(),
        width: 300,
        height: 300,
        reserve: Some(MarketFaceReserveWire { field8: 1 }),
    }
    .encode_to_vec())
}

pub(crate) fn decode_market_face(input: &[u8]) -> Result<MarketFaceSegment, MessageDecodeError> {
    let face = MarketFaceWire::decode(input).map_err(|_error| MessageDecodeError)?;
    if face.face_id.is_empty()
        || face.face_id.len() * 2 > MAX_EMOJI_ID_BYTES
        || face.package_id <= 0
        || !valid_text(&face.key, MAX_KEY_BYTES)
        || !valid_text(&face.summary, MAX_SUMMARY_BYTES)
    {
        return Err(MessageDecodeError);
    }
    Ok(MarketFaceSegment {
        emoji_id: hex::encode(&face.face_id),
        package_id: face.package_id,
        key: face.key,
        summary: face.summary,
    })
}

fn decode_hex(value: &str) -> Result<Vec<u8>, MessageDecodeError> {
    if value.is_empty() || value.len() > MAX_EMOJI_ID_BYTES || !value.len().is_multiple_of(2) {
        return Err(MessageDecodeError);
    }
    hex::decode(value).map_err(|_error| MessageDecodeError)
}

fn valid_text(value: &str, limit: usize) -> bool {
    !value.is_empty() && value.len() <= limit && !value.chars().any(|character| character == '\0')
}

#[derive(Clone, PartialEq, Message)]
struct MarketFaceWire {
    #[prost(string, tag = "1")]
    summary: String,
    #[prost(int32, tag = "2")]
    item_type: i32,
    #[prost(int32, tag = "3")]
    info: i32,
    #[prost(bytes = "vec", tag = "4")]
    face_id: Vec<u8>,
    #[prost(int32, tag = "5")]
    package_id: i32,
    #[prost(int32, tag = "6")]
    subtype: i32,
    #[prost(string, tag = "7")]
    key: String,
    #[prost(int32, tag = "10")]
    width: i32,
    #[prost(int32, tag = "11")]
    height: i32,
    #[prost(message, optional, tag = "13")]
    reserve: Option<MarketFaceReserveWire>,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct MarketFaceReserveWire {
    #[prost(int32, tag = "8")]
    field8: i32,
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::{MarketFaceWire, decode_market_face, encode_market_face};

    #[test]
    fn frozen_market_face_shape_round_trips() -> Result<(), Box<dyn std::error::Error>> {
        let encoded = encode_market_face("012aFe", 42, "do-not-log-this", "[表情]")?;
        let wire = MarketFaceWire::decode(encoded.as_slice())?;
        assert_eq!((wire.item_type, wire.info, wire.subtype), (6, 1, 3));
        assert_eq!((wire.width, wire.height), (300, 300));
        assert_eq!(wire.reserve.ok_or("reserve missing")?.field8, 1);

        let face = decode_market_face(&encoded)?;
        assert_eq!(face.emoji_id(), "012afe");
        assert_eq!(face.package_id(), 42);
        assert_eq!(face.key(), "do-not-log-this");
        assert_eq!(face.summary(), "[表情]");
        assert!(face.asset_url().contains("/01/012afe/"));
        assert!(!format!("{face:?}").contains("do-not-log-this"));
        Ok(())
    }

    #[test]
    fn malformed_market_face_material_fails_closed() {
        assert!(encode_market_face("xyz", 42, "key", "[表情]").is_err());
        assert!(encode_market_face("012a", 0, "key", "[表情]").is_err());
        assert!(decode_market_face(&[]).is_err());
    }
}

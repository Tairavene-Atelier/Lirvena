use prost::Message;

use super::model::{
    FaceKind, FaceSegment, MentionSegment, MentionTarget, RichTextElement, RichTextMessage, Segment,
};
use super::proto::{
    AnimatedFaceWire, CommonWire, ElementWire, FaceWire, MentionWire, RichTextWire,
    StandardFaceWire, TextWire,
};
use crate::MessageDecodeError;

const MAX_RICH_TEXT_LEN: usize = 1024 * 1024;
const MAX_ELEMENTS: usize = 512;
const MAX_ELEMENT_LEN: usize = 256 * 1024;
const MAX_TEXT_LEN: usize = 64 * 1024;
const MAX_USER_LEN: usize = 128;

/// Decodes a bounded rich-text body while preserving every original element.
///
/// # Errors
///
/// Returns an error for an empty, oversized, malformed or structurally excessive body.
pub fn decode_rich_text(input: &[u8]) -> Result<RichTextMessage, MessageDecodeError> {
    if input.is_empty() || input.len() > MAX_RICH_TEXT_LEN {
        return Err(MessageDecodeError);
    }
    let rich = RichTextWire::decode(input).map_err(|_error| MessageDecodeError)?;
    if rich.elements.len() > MAX_ELEMENTS
        || rich
            .elements
            .iter()
            .any(|value| value.len() > MAX_ELEMENT_LEN)
    {
        return Err(MessageDecodeError);
    }
    let elements = rich
        .elements
        .into_iter()
        .map(decode_element)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RichTextMessage::new(
        elements,
        rich.attributes,
        rich.file,
        rich.voice,
    ))
}

fn decode_element(encoded: Vec<u8>) -> Result<RichTextElement, MessageDecodeError> {
    if encoded.is_empty() {
        return Ok(RichTextElement::new(Segment::Unsupported, encoded));
    }
    let wire = ElementWire::decode(encoded.as_slice()).map_err(|_error| MessageDecodeError)?;
    let projections = [
        wire.text.as_deref().map(decode_text).transpose()?.flatten(),
        wire.face.as_deref().map(decode_face).transpose()?.flatten(),
        wire.common
            .as_deref()
            .map(decode_common)
            .transpose()?
            .flatten(),
        wire.direct_image
            .as_deref()
            .map(super::media_legacy::decode_direct_image)
            .transpose()?,
        wire.group_image
            .as_deref()
            .map(super::media_legacy::decode_group_image)
            .transpose()?,
        wire.video
            .as_deref()
            .map(super::media_legacy::decode_video)
            .transpose()?,
    ];
    let mut supported = projections.into_iter().flatten();
    let first = supported.next();
    let segment = if supported.next().is_some() {
        Segment::Unsupported
    } else {
        first.unwrap_or(Segment::Unsupported)
    };
    Ok(RichTextElement::new(segment, encoded))
}

fn decode_text(input: &[u8]) -> Result<Option<Segment>, MessageDecodeError> {
    let wire = TextWire::decode(input).map_err(|_error| MessageDecodeError)?;
    let Some(text) = wire.text else {
        return Ok(None);
    };
    if !valid_text(&text, MAX_TEXT_LEN) {
        return Err(MessageDecodeError);
    }
    let mention = wire
        .reserve
        .as_deref()
        .and_then(|value| MentionWire::decode(value).ok());
    let Some(mention) = mention.filter(|value| value.mention_type == 1 || value.mention_type == 2)
    else {
        return Ok(Some(Segment::Text(text)));
    };
    if !valid_text(&mention.user, MAX_USER_LEN) {
        return Err(MessageDecodeError);
    }
    let target = mention_target(&mention, wire.legacy_attributes.as_deref());
    Ok(Some(Segment::Mention(MentionSegment::new(text, target))))
}

fn mention_target(wire: &MentionWire, legacy: Option<&[u8]>) -> MentionTarget {
    if wire.mention_type == 1 || wire.user == "all" {
        return MentionTarget::Everyone;
    }
    if wire.account != 0 {
        return MentionTarget::Account(wire.account);
    }
    if let Some(value) = legacy_account(legacy) {
        return MentionTarget::Account(value);
    }
    if !wire.user.is_empty() {
        return MentionTarget::User(wire.user.clone());
    }
    MentionTarget::Unresolved
}

fn legacy_account(value: Option<&[u8]>) -> Option<u32> {
    let value = value?;
    let bytes: [u8; 4] = value.get(7..11)?.try_into().ok()?;
    let account = u32::from_be_bytes(bytes);
    (account != 0).then_some(account)
}

fn decode_face(input: &[u8]) -> Result<Option<Segment>, MessageDecodeError> {
    let wire = FaceWire::decode(input).map_err(|_error| MessageDecodeError)?;
    let id = wire.index.and_then(|value| u32::try_from(value).ok());
    Ok(id.map(|id| Segment::Face(FaceSegment::new(id, FaceKind::Standard))))
}

fn decode_common(input: &[u8]) -> Result<Option<Segment>, MessageDecodeError> {
    let wire = CommonWire::decode(input).map_err(|_error| MessageDecodeError)?;
    let Some(body) = wire.body else {
        return Ok(None);
    };
    let face = match wire.service_type {
        33 => StandardFaceWire::decode(body.as_slice())
            .ok()
            .map(|value| FaceSegment::new(value.face_id, FaceKind::Standard)),
        37 => AnimatedFaceWire::decode(body.as_slice())
            .ok()
            .and_then(|value| {
                value
                    .face_id
                    .and_then(|id| u32::try_from(id).ok())
                    .map(|id| FaceSegment::new(id, FaceKind::Animated))
            }),
        _ => None,
    };
    if let Some(face) = face {
        return Ok(Some(Segment::Face(face)));
    }
    if wire.service_type == 48 {
        return super::media_decode::decode(wire.business_type, &body);
    }
    Ok(None)
}

fn valid_text(value: &str, maximum: usize) -> bool {
    value.len() <= maximum && !value.contains('\0')
}

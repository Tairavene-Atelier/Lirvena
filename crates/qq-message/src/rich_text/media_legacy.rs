use prost::Message;

use super::media::{ImageSegment, MediaFile, MediaFileSpec, MediaScope, VideoSegment};
use super::model::Segment;
use super::proto::{DirectImageWire, GroupImageWire, LegacyVideoWire};
use crate::MessageDecodeError;

const MAX_NAME_LEN: usize = 1024;
const MAX_ID_LEN: usize = 256;
const MAX_REMOTE_REFERENCE_LEN: usize = 8192;
const MAX_SUMMARY_LEN: usize = 512;
const MAX_BINARY_DIGEST_LEN: usize = 64;

pub(super) fn decode_direct_image(input: &[u8]) -> Result<Segment, MessageDecodeError> {
    let wire = DirectImageWire::decode(input).map_err(|_error| MessageDecodeError)?;
    let summary = wire.reserve.as_ref().map(|value| value.summary.as_str());
    let subtype = wire
        .reserve
        .as_ref()
        .and_then(|value| u32::try_from(value.subtype).ok())
        .unwrap_or_default();
    Ok(Segment::Image(ImageSegment::new(
        media_file(LegacyMediaSpec {
            uuid: None,
            name: wire.name,
            digest: wire.digest,
            remote_reference: wire.remote_reference,
            size: wire.size,
            width: wire.width,
            height: wire.height,
            duration_seconds: 0,
        })?,
        MediaScope::Direct,
        optional_text(summary, MAX_SUMMARY_LEN)?,
        subtype,
    )))
}

pub(super) fn decode_group_image(input: &[u8]) -> Result<Segment, MessageDecodeError> {
    let wire = GroupImageWire::decode(input).map_err(|_error| MessageDecodeError)?;
    let summary = wire.reserve.as_ref().map(|value| value.summary.as_str());
    let subtype = wire
        .reserve
        .as_ref()
        .and_then(|value| u32::try_from(value.subtype).ok())
        .unwrap_or_default();
    Ok(Segment::Image(ImageSegment::new(
        media_file(LegacyMediaSpec {
            uuid: None,
            name: wire.name,
            digest: wire.digest,
            remote_reference: wire.remote_reference,
            size: wire.size,
            width: nonnegative(wire.width)?,
            height: nonnegative(wire.height)?,
            duration_seconds: 0,
        })?,
        MediaScope::Group,
        optional_text(summary, MAX_SUMMARY_LEN)?,
        subtype,
    )))
}

pub(super) fn decode_video(input: &[u8]) -> Result<Segment, MessageDecodeError> {
    let wire = LegacyVideoWire::decode(input).map_err(|_error| MessageDecodeError)?;
    let width = nonnegative(if wire.width == 0 {
        wire.thumbnail_width
    } else {
        wire.width
    })?;
    let height = nonnegative(if wire.height == 0 {
        wire.thumbnail_height
    } else {
        wire.height
    })?;
    let file = media_file(LegacyMediaSpec {
        uuid: Some(wire.uuid),
        name: wire.name,
        digest: wire.digest,
        remote_reference: String::new(),
        size: nonnegative(wire.size)?,
        width,
        height,
        duration_seconds: nonnegative(wire.duration_seconds)?,
    })?;
    Ok(Segment::Video(VideoSegment::new(file, MediaScope::Unknown)))
}

struct LegacyMediaSpec {
    uuid: Option<String>,
    name: String,
    digest: Vec<u8>,
    remote_reference: String,
    size: u32,
    width: u32,
    height: u32,
    duration_seconds: u32,
}

fn media_file(spec: LegacyMediaSpec) -> Result<MediaFile, MessageDecodeError> {
    require_text(&spec.name, MAX_NAME_LEN)?;
    if let Some(value) = spec.uuid.as_deref() {
        require_text(value, MAX_ID_LEN)?;
    }
    let remote_reference = if spec.remote_reference.is_empty() {
        None
    } else {
        require_text(&spec.remote_reference, MAX_REMOTE_REFERENCE_LEN)?;
        Some(spec.remote_reference)
    };
    let digest = if spec.digest.is_empty() {
        None
    } else {
        if spec.digest.len() > MAX_BINARY_DIGEST_LEN {
            return Err(MessageDecodeError);
        }
        Some(hex(&spec.digest))
    };
    Ok(MediaFile::new(MediaFileSpec {
        uuid: spec.uuid.filter(|value| !value.is_empty()),
        name: spec.name,
        digest,
        sha1: None,
        remote_reference,
        size: spec.size,
        width: spec.width,
        height: spec.height,
        duration_seconds: spec.duration_seconds,
    }))
}

fn optional_text(
    value: Option<&str>,
    maximum: usize,
) -> Result<Option<String>, MessageDecodeError> {
    value
        .filter(|text| !text.is_empty())
        .map(|text| {
            require_text(text, maximum)?;
            Ok(text.to_owned())
        })
        .transpose()
}

fn require_text(value: &str, maximum: usize) -> Result<(), MessageDecodeError> {
    if value.len() <= maximum && !value.contains('\0') {
        Ok(())
    } else {
        Err(MessageDecodeError)
    }
}

fn nonnegative(value: i32) -> Result<u32, MessageDecodeError> {
    u32::try_from(value).map_err(|_| MessageDecodeError)
}

fn hex(value: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789ABCDEF";
    let mut output = String::with_capacity(value.len() * 2);
    for byte in value {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

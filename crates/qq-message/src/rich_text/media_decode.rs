use prost::Message;

use super::media::{
    ImageSegment, MediaFile, MediaFileSpec, MediaScope, VideoSegment, VoiceSegment,
};
use super::model::Segment;
use super::proto::MediaInfoWire;
use crate::MessageDecodeError;

const MAX_MEDIA_BODIES: usize = 8;
const MAX_NAME_LEN: usize = 1024;
const MAX_ID_LEN: usize = 256;
const MAX_DIGEST_LEN: usize = 128;
const MAX_REMOTE_REFERENCE_LEN: usize = 8192;
const MAX_SUMMARY_LEN: usize = 512;

pub(super) fn decode(
    business_type: u32,
    input: &[u8],
) -> Result<Option<Segment>, MessageDecodeError> {
    let Some((kind, scope)) = media_kind(business_type) else {
        return Ok(None);
    };
    let wire = MediaInfoWire::decode(input).map_err(|_error| MessageDecodeError)?;
    if wire.bodies.is_empty() || wire.bodies.len() > MAX_MEDIA_BODIES {
        return Err(MessageDecodeError);
    }
    let body = &wire.bodies[0];
    let index = body.index.as_ref().ok_or(MessageDecodeError)?;
    let info = index.info.as_ref().ok_or(MessageDecodeError)?;
    require_text(&index.uuid, MAX_ID_LEN)?;
    require_text(&info.name, MAX_NAME_LEN)?;
    let digest = normalized_digest(&info.digest)?;
    let sha1 = normalized_digest(&info.sha1)?;
    let remote_reference = body
        .picture
        .as_ref()
        .map(|value| value.remote_reference.as_str())
        .filter(|value| !value.is_empty())
        .map(|value| {
            require_text(value, MAX_REMOTE_REFERENCE_LEN)?;
            Ok::<String, MessageDecodeError>(value.to_owned())
        })
        .transpose()?;
    let file = MediaFile::new(MediaFileSpec {
        uuid: (!index.uuid.is_empty()).then(|| index.uuid.clone()),
        name: info.name.clone(),
        digest,
        sha1,
        remote_reference,
        size: info.size,
        width: info.width,
        height: info.height,
        duration_seconds: info.duration_seconds,
    });
    match kind {
        MediaKind::Image => {
            let image = wire.extension.and_then(|value| value.image);
            let summary = image
                .as_ref()
                .map(|value| value.summary.as_str())
                .filter(|value| !value.is_empty())
                .map(|value| {
                    require_text(value, MAX_SUMMARY_LEN)?;
                    Ok::<String, MessageDecodeError>(value.to_owned())
                })
                .transpose()?;
            let subtype = image.map_or(0, |value| value.subtype);
            Ok(Some(Segment::Image(ImageSegment::new(
                file, scope, summary, subtype,
            ))))
        }
        MediaKind::Video => Ok(Some(Segment::Video(VideoSegment::new(file, scope)))),
        MediaKind::Voice => Ok(Some(Segment::Voice(VoiceSegment::new(file, scope)))),
    }
}

#[derive(Clone, Copy)]
enum MediaKind {
    Image,
    Video,
    Voice,
}

const fn media_kind(business_type: u32) -> Option<(MediaKind, MediaScope)> {
    match business_type {
        10 => Some((MediaKind::Image, MediaScope::Direct)),
        20 => Some((MediaKind::Image, MediaScope::Group)),
        11 => Some((MediaKind::Video, MediaScope::Direct)),
        21 => Some((MediaKind::Video, MediaScope::Group)),
        12 => Some((MediaKind::Voice, MediaScope::Direct)),
        22 => Some((MediaKind::Voice, MediaScope::Group)),
        _ => None,
    }
}

fn normalized_digest(value: &str) -> Result<Option<String>, MessageDecodeError> {
    if value.is_empty() {
        return Ok(None);
    }
    if value.len() > MAX_DIGEST_LEN
        || !value.len().is_multiple_of(2)
        || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(MessageDecodeError);
    }
    Ok(Some(value.to_ascii_uppercase()))
}

fn require_text(value: &str, maximum: usize) -> Result<(), MessageDecodeError> {
    if value.len() <= maximum && !value.contains('\0') {
        Ok(())
    } else {
        Err(MessageDecodeError)
    }
}

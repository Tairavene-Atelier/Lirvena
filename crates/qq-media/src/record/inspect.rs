use md5::{Digest as _, Md5};
use sha1::Sha1;

use super::{PreparedRecord, RecordDescriptor, RecordFormat};
use crate::MediaError;

const MAX_RECORD_BYTES: usize = 50 * 1024 * 1024;
const SILK_HEADER: &[u8] = b"#!SILK_V3";
const TENCENT_SILK_HEADER: &[u8] = b"\x02#!SILK_V3";

/// Validates and normalizes an already QQ-compatible audio stream.
///
/// Standard SILK v3 is converted to Tencent framing by adding the Tencent byte and removing its
/// terminal marker. MP3 and AMR remain byte-identical. Other formats require an external codec
/// stage and are rejected here rather than mislabeled.
///
/// # Errors
///
/// Returns an error for empty, excessive, malformed, or unsupported audio.
pub fn prepare_record(input: &[u8]) -> Result<PreparedRecord, MediaError> {
    if input.is_empty() || input.len() > MAX_RECORD_BYTES {
        return Err(MediaError::SizeLimit);
    }
    let (bytes, format, duration_seconds) = if input.starts_with(TENCENT_SILK_HEADER) {
        (
            input.to_vec(),
            RecordFormat::TencentSilkV3,
            silk_duration(input, TENCENT_SILK_HEADER.len(), false)?,
        )
    } else if input.starts_with(SILK_HEADER) && input.ends_with(&[0xff, 0xff]) {
        let mut normalized = Vec::with_capacity(input.len() - 1);
        normalized.push(2);
        normalized.extend_from_slice(&input[..input.len() - 2]);
        let duration = silk_duration(input, SILK_HEADER.len(), true)?;
        (normalized, RecordFormat::TencentSilkV3, duration)
    } else if input.starts_with(b"#!AMR\n") {
        let seconds =
            u32::try_from(input.len().div_ceil(1_607)).map_err(|_error| MediaError::SizeLimit)?;
        (input.to_vec(), RecordFormat::Amr, seconds)
    } else if is_mp3(input) {
        (input.to_vec(), RecordFormat::Mp3, 0)
    } else {
        return Err(MediaError::ReferenceRejected);
    };
    let size = u32::try_from(bytes.len()).map_err(|_error| MediaError::SizeLimit)?;
    let descriptor = RecordDescriptor {
        size,
        duration_seconds,
        format,
        md5: Md5::digest(&bytes).into(),
        sha1: Sha1::digest(&bytes).into(),
    };
    Ok(PreparedRecord {
        bytes: bytes.into_boxed_slice(),
        descriptor,
    })
}

fn is_mp3(input: &[u8]) -> bool {
    input.starts_with(b"ID3") || matches!(input.get(..2), Some([0xff, 0xf2 | 0xf3 | 0xfb]))
}

fn silk_duration(
    input: &[u8],
    mut cursor: usize,
    terminal_marker: bool,
) -> Result<u32, MediaError> {
    let mut frames = 0_u32;
    while cursor < input.len() {
        let length_bytes = input
            .get(cursor..cursor + 2)
            .ok_or(MediaError::ReferenceRejected)?;
        let length = u16::from_le_bytes([length_bytes[0], length_bytes[1]]);
        if terminal_marker && length == u16::MAX {
            if cursor + 2 != input.len() {
                return Err(MediaError::ReferenceRejected);
            }
            break;
        }
        cursor = cursor
            .checked_add(2 + usize::from(length))
            .ok_or(MediaError::ReferenceRejected)?;
        if cursor > input.len() {
            return Err(MediaError::ReferenceRejected);
        }
        frames = frames.checked_add(1).ok_or(MediaError::SizeLimit)?;
    }
    if frames == 0 || (terminal_marker && cursor == input.len()) {
        return Err(MediaError::ReferenceRejected);
    }
    Ok(frames.div_ceil(50))
}

#[cfg(test)]
mod tests {
    use super::prepare_record;
    use crate::RecordFormat;

    #[test]
    fn standard_silk_is_normalized_once_and_duration_is_bounded()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut input = b"#!SILK_V3".to_vec();
        input.extend_from_slice(&[1, 0, 0xaa, 1, 0, 0xbb, 0xff, 0xff]);
        let record = prepare_record(&input)?;
        assert_eq!(record.descriptor().format(), RecordFormat::TencentSilkV3);
        assert!(record.bytes().starts_with(b"\x02#!SILK_V3"));
        assert_eq!(record.descriptor().duration_seconds(), 1);
        Ok(())
    }

    #[test]
    fn malformed_silk_never_falls_through_as_an_opaque_upload() {
        let truncated = b"\x02#!SILK_V3\x04\x00\xaa";
        assert!(prepare_record(truncated).is_err());

        let mut missing_terminal_marker = b"#!SILK_V3".to_vec();
        missing_terminal_marker.extend_from_slice(&[1, 0, 0xaa]);
        assert!(prepare_record(&missing_terminal_marker).is_err());
    }
}

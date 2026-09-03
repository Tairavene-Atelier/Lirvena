use md5::{Digest as _, Md5};
use sha1::Sha1;

use super::VideoDescriptor;
use crate::MediaError;

const MAX_VIDEO_BYTES: usize = 256 * 1024 * 1024;
const DEFAULT_THUMBNAIL: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x08, 0xd7, 0x63, 0xf8, 0xcf, 0xc0, 0xf0,
    0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

/// Validates the bounded MP4 container envelope without decoding media content.
///
/// # Errors
///
/// Returns an error for empty, excessive, or non-MP4 input.
pub fn analyze_video(input: &[u8]) -> Result<VideoDescriptor, MediaError> {
    if input.len() < 12 || input.len() > MAX_VIDEO_BYTES || input.get(4..8) != Some(b"ftyp") {
        return Err(MediaError::ReferenceRejected);
    }
    let declared = u32::from_be_bytes(
        input[..4]
            .try_into()
            .map_err(|_error| MediaError::ReferenceRejected)?,
    );
    if declared < 8 || usize::try_from(declared).map_or(true, |size| size > input.len()) {
        return Err(MediaError::ReferenceRejected);
    }
    let size = u32::try_from(input.len()).map_err(|_error| MediaError::SizeLimit)?;
    Ok(VideoDescriptor {
        size,
        md5: Md5::digest(input).into(),
        sha1: Sha1::digest(input).into(),
    })
}

/// Returns a small valid PNG used when `OneBot` does not provide a video thumbnail.
#[must_use]
pub const fn default_video_thumbnail() -> &'static [u8] {
    DEFAULT_THUMBNAIL
}

#[cfg(test)]
mod tests {
    use super::{analyze_video, default_video_thumbnail};
    use crate::{ImageFormat, analyze_image};

    #[test]
    fn mp4_envelope_and_default_thumbnail_are_bounded() -> Result<(), Box<dyn std::error::Error>> {
        let video = analyze_video(&[0, 0, 0, 12, b'f', b't', b'y', b'p', b'i', b's', b'o', b'm'])?;
        assert_eq!(video.size(), 12);
        let thumbnail = analyze_image(default_video_thumbnail())?;
        assert_eq!(thumbnail.format(), ImageFormat::Png);
        assert_eq!((thumbnail.width(), thumbnail.height()), (1, 1));
        Ok(())
    }
}

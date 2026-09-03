use md5::{Digest as _, Md5};
use sha1::Sha1;

use super::{ImageDescriptor, ImageFormat};
use crate::MediaError;

const MAX_IMAGE_BYTES: usize = 50 * 1024 * 1024;
const JPEG_2000_SIGNATURE: [u8; 8] = [0, 0, 0, 12, 0x6a, 0x50, 0x20, 0x20];

/// Inspects bounded image bytes without decoding pixel content.
///
/// # Errors
///
/// Returns an error for empty, excessive, unsupported, or malformed images.
pub fn analyze_image(bytes: &[u8]) -> Result<ImageDescriptor, MediaError> {
    if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
        return Err(MediaError::SizeLimit);
    }
    let (format, width, height) = image_shape(bytes)?;
    let size = u32::try_from(bytes.len()).map_err(|_error| MediaError::SizeLimit)?;
    Ok(ImageDescriptor {
        size,
        width,
        height,
        format,
        md5: Md5::digest(bytes).into(),
        sha1: Sha1::digest(bytes).into(),
    })
}

fn image_shape(bytes: &[u8]) -> Result<(ImageFormat, u32, u32), MediaError> {
    if bytes.len() >= 56 && bytes[..8] == JPEG_2000_SIGNATURE {
        let width = u32::from_be_bytes(
            bytes[48..52]
                .try_into()
                .map_err(|_error| MediaError::ReferenceRejected)?,
        );
        let height = u32::from_be_bytes(
            bytes[52..56]
                .try_into()
                .map_err(|_error| MediaError::ReferenceRejected)?,
        );
        return dimensions(ImageFormat::Jpeg2000, width, height);
    }
    let format =
        match imagesize::image_type(bytes).map_err(|_error| MediaError::ReferenceRejected)? {
            imagesize::ImageType::Jpeg => ImageFormat::Jpeg,
            imagesize::ImageType::Png => ImageFormat::Png,
            imagesize::ImageType::Webp => ImageFormat::Webp,
            imagesize::ImageType::Bmp => ImageFormat::Bmp,
            imagesize::ImageType::Tiff => ImageFormat::Tiff,
            imagesize::ImageType::Gif => ImageFormat::Gif,
            _ => return Err(MediaError::ReferenceRejected),
        };
    let size = imagesize::blob_size(bytes).map_err(|_error| MediaError::ReferenceRejected)?;
    dimensions(
        format,
        u32::try_from(size.width).map_err(|_error| MediaError::ReferenceRejected)?,
        u32::try_from(size.height).map_err(|_error| MediaError::ReferenceRejected)?,
    )
}

fn dimensions(
    format: ImageFormat,
    width: u32,
    height: u32,
) -> Result<(ImageFormat, u32, u32), MediaError> {
    if width == 0 || height == 0 {
        Err(MediaError::ReferenceRejected)
    } else {
        Ok((format, width, height))
    }
}

#[cfg(test)]
mod tests {
    use super::analyze_image;
    use crate::ImageFormat;

    #[test]
    fn png_dimensions_and_hashes_are_stable() -> Result<(), Box<dyn std::error::Error>> {
        let png = [
            0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, b'I', b'H', b'D', b'R', 0,
            0, 0, 2, 0, 0, 0, 3, 8, 6, 0, 0, 0,
        ];
        let image = analyze_image(&png)?;
        assert_eq!(image.format(), ImageFormat::Png);
        assert_eq!((image.width(), image.height()), (2, 3));
        assert_eq!(image.size(), 29);
        assert_ne!(image.md5(), [0; 16]);
        assert_ne!(image.sha1(), [0; 20]);
        Ok(())
    }
}

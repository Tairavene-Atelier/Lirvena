mod inspect;
mod model;
mod request;
mod response;

pub use inspect::analyze_image;
pub use model::{ImageDescriptor, ImageFormat, ImageMetadataRequest};
pub use request::encode_image_metadata_request;
pub use response::parse_image_metadata_response;

const MAX_UID_BYTES: usize = 128;

pub(crate) fn valid_uid(uid: &str) -> bool {
    !uid.is_empty() && uid.len() <= MAX_UID_BYTES && !uid.chars().any(char::is_control)
}

pub(crate) fn upper_hex(input: &[u8]) -> String {
    use core::fmt::Write as _;

    let mut output = String::with_capacity(input.len() * 2);
    for byte in input {
        let _ignored = write!(output, "{byte:02X}");
    }
    output
}

pub(crate) fn decode_hex<const N: usize>(input: &str) -> Result<[u8; N], crate::MediaError> {
    if input.len() != N * 2 {
        return Err(crate::MediaError::RemoteRejected);
    }
    let mut output = [0_u8; N];
    for (index, pair) in input.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_nibble(pair[0]).ok_or(crate::MediaError::RemoteRejected)?;
        let low = hex_nibble(pair[1]).ok_or(crate::MediaError::RemoteRejected)?;
        output[index] = (high << 4) | low;
    }
    Ok(output)
}

const fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

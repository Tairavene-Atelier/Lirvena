use md5::{Digest, Md5};
use prost::Message;

use crate::HighwayError;
use crate::proto::{
    BaseHeadWire, LoginHeadWire, RequestHeadWire, ResponseHeadWire, SegmentHeadWire,
};

const START: u8 = 0x28;
const END: u8 = 0x29;
const MAX_HEAD_BYTES: usize = 64 * 1024;
const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;
const MAX_SECRET_BYTES: usize = 8 * 1024;
const MAX_EXTENSION_BYTES: usize = 64 * 1024;

/// Borrowed inputs for one authenticated upload block.
pub struct UploadBlock<'a> {
    /// QQ account number associated with the authenticated session.
    pub uin: u64,
    /// Monotonic per-client block sequence.
    pub sequence: u32,
    /// Profile-provided sub-application identifier.
    pub sub_app_id: u32,
    /// Profile-provided application identifier.
    pub app_id: u32,
    /// Audited media command identifier.
    pub command_id: u32,
    /// Total byte length of the source object.
    pub file_size: u64,
    /// Offset of this block in the source object.
    pub offset: u64,
    /// Opaque service ticket from QQ metadata or session negotiation.
    pub ticket: &'a [u8],
    /// MD5 digest of the complete object.
    pub file_md5: &'a [u8; 16],
    /// Optional opaque metadata response extension.
    pub extension: &'a [u8],
    /// Bounded block payload.
    pub body: &'a [u8],
    /// In-memory login signature; never sourced from user configuration.
    pub login_signature: &'a [u8],
}

/// Correlated result of one QQ upload block.
#[derive(Clone, Eq, PartialEq)]
pub struct UploadResponse {
    sequence: Option<u32>,
    offset: Option<u64>,
    data_length: Option<u32>,
    extension: Vec<u8>,
}

impl core::fmt::Debug for UploadResponse {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("UploadResponse")
            .field("sequence", &self.sequence)
            .field("offset", &self.offset)
            .field("data_length", &self.data_length)
            .field("extension_bytes", &self.extension.len())
            .finish()
    }
}

impl UploadResponse {
    /// Returns the echoed block sequence.
    #[must_use]
    pub const fn sequence(&self) -> Option<u32> {
        self.sequence
    }

    /// Returns the acknowledged source offset.
    #[must_use]
    pub const fn offset(&self) -> Option<u64> {
        self.offset
    }

    /// Returns the acknowledged byte length.
    #[must_use]
    pub const fn data_length(&self) -> Option<u32> {
        self.data_length
    }

    /// Returns the bounded opaque response extension.
    #[must_use]
    pub fn extension(&self) -> &[u8] {
        &self.extension
    }
}

/// Encodes one audited 52194 Highway upload frame.
///
/// # Errors
///
/// Returns an error when lengths, offsets, identifiers, or secrets are invalid.
pub fn encode_upload_block(block: &UploadBlock<'_>) -> Result<Vec<u8>, HighwayError> {
    validate_block(block)?;
    let block_md5: [u8; 16] = Md5::digest(block.body).into();
    let head = RequestHeadWire {
        base: Some(BaseHeadWire {
            version: 1,
            uin: block.uin.to_string(),
            command: "PicUp.DataUp".to_owned(),
            sequence: block.sequence,
            retry_times: 0,
            app_id: block.sub_app_id,
            data_flag: 16,
            command_id: block.command_id,
        }),
        segment: Some(SegmentHeadWire {
            service_id: 0,
            file_size: block.file_size,
            offset: block.offset,
            data_length: u32::try_from(block.body.len()).map_err(|_| HighwayError::InvalidInput)?,
            return_code: 0,
            ticket: block.ticket.to_vec(),
            block_md5: block_md5.to_vec(),
            file_md5: block.file_md5.to_vec(),
        }),
        extension: block.extension.to_vec(),
        timestamp: 0,
        login: Some(LoginHeadWire {
            signature_type: 8,
            signature: block.login_signature.to_vec(),
            app_id: block.app_id,
        }),
    }
    .encode_to_vec();
    if head.len() > MAX_HEAD_BYTES {
        return Err(HighwayError::InvalidInput);
    }
    let head_len = u32::try_from(head.len()).map_err(|_| HighwayError::InvalidInput)?;
    let body_len = u32::try_from(block.body.len()).map_err(|_| HighwayError::InvalidInput)?;
    let mut output = Vec::with_capacity(10 + head.len() + block.body.len());
    output.push(START);
    output.extend_from_slice(&head_len.to_be_bytes());
    output.extend_from_slice(&body_len.to_be_bytes());
    output.extend_from_slice(&head);
    output.extend_from_slice(block.body);
    output.push(END);
    Ok(output)
}

/// Decodes one bounded Highway upload response.
///
/// # Errors
///
/// Returns an error for malformed framing, nonzero QQ result codes, or a
/// response carrying a nonzero QQ result code.
pub fn decode_upload_response(input: &[u8]) -> Result<UploadResponse, HighwayError> {
    if input.len() < 10 || input[0] != START || input[input.len() - 1] != END {
        return Err(HighwayError::MalformedFrame);
    }
    let head_len = u32::from_be_bytes(
        input[1..5]
            .try_into()
            .map_err(|_| HighwayError::MalformedFrame)?,
    ) as usize;
    let body_len = u32::from_be_bytes(
        input[5..9]
            .try_into()
            .map_err(|_| HighwayError::MalformedFrame)?,
    ) as usize;
    if head_len == 0 || head_len > MAX_HEAD_BYTES || body_len > MAX_BODY_BYTES {
        return Err(HighwayError::MalformedFrame);
    }
    let expected_total = 10usize
        .checked_add(head_len)
        .and_then(|length| length.checked_add(body_len))
        .ok_or(HighwayError::MalformedFrame)?;
    if input.len() != expected_total {
        return Err(HighwayError::MalformedFrame);
    }
    let head = ResponseHeadWire::decode(&input[9..9 + head_len])
        .map_err(|_| HighwayError::MalformedFrame)?;
    if head.error_code != 0 {
        return Err(HighwayError::RemoteRejected);
    }
    if head.extension.len() > MAX_EXTENSION_BYTES {
        return Err(HighwayError::RemoteRejected);
    }
    let sequence = head.base.map(|base| base.sequence);
    let (offset, data_length) = head.segment.map_or((None, None), |segment| {
        (Some(segment.offset), Some(segment.data_length))
    });
    Ok(UploadResponse {
        sequence,
        offset,
        data_length,
        extension: head.extension,
    })
}

fn validate_block(block: &UploadBlock<'_>) -> Result<(), HighwayError> {
    let length = u64::try_from(block.body.len()).map_err(|_| HighwayError::InvalidInput)?;
    if block.uin == 0
        || block.sequence == 0
        || block.app_id == 0
        || block.sub_app_id == 0
        || block.command_id == 0
        || block.file_size == 0
        || block.body.is_empty()
        || block.body.len() > MAX_BODY_BYTES
        || block.ticket.is_empty()
        || block.ticket.len() > MAX_SECRET_BYTES
        || block.login_signature.is_empty()
        || block.login_signature.len() > MAX_SECRET_BYTES
        || block.extension.len() > MAX_EXTENSION_BYTES
        || block.offset.checked_add(length).is_none()
        || block.offset + length > block.file_size
    {
        return Err(HighwayError::InvalidInput);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use md5::{Digest, Md5};
    use prost::Message;

    use super::{UploadBlock, decode_upload_response, encode_upload_block};
    use crate::HighwayError;
    use crate::proto::{BaseHeadWire, ResponseHeadWire, SegmentHeadWire};

    #[test]
    fn request_frame_carries_correlated_digests_and_no_trailing_bytes()
    -> Result<(), Box<dyn std::error::Error>> {
        let body = b"media";
        let file_md5: [u8; 16] = Md5::digest(body).into();
        let frame = encode_upload_block(&UploadBlock {
            uin: 42,
            sequence: 7,
            sub_app_id: 100,
            app_id: 200,
            command_id: 1_004,
            file_size: body.len() as u64,
            offset: 0,
            ticket: b"ticket",
            file_md5: &file_md5,
            extension: &[],
            body,
            login_signature: b"login",
        })?;
        assert_eq!(frame[0], 0x28);
        assert_eq!(frame[frame.len() - 1], 0x29);
        let head_len = u32::from_be_bytes(frame[1..5].try_into()?) as usize;
        let head = crate::proto::RequestHeadWire::decode(&frame[9..9 + head_len])?;
        let base = head.base.ok_or(HighwayError::MalformedFrame)?;
        let segment = head.segment.ok_or(HighwayError::MalformedFrame)?;
        assert_eq!(base.command, "PicUp.DataUp");
        assert_eq!(base.command_id, 1_004);
        assert_eq!(segment.file_md5, file_md5);
        assert_eq!(segment.block_md5, file_md5);
        Ok(())
    }

    #[test]
    fn response_exposes_optional_acknowledgement_fields() -> Result<(), Box<dyn std::error::Error>>
    {
        let head = ResponseHeadWire {
            base: Some(BaseHeadWire {
                sequence: 8,
                ..BaseHeadWire::default()
            }),
            segment: Some(SegmentHeadWire {
                offset: 10,
                data_length: 5,
                ..SegmentHeadWire::default()
            }),
            error_code: 0,
            allow_retry: 0,
            extension: vec![],
        }
        .encode_to_vec();
        let mut frame = vec![0x28];
        let head_len = u32::try_from(head.len()).map_err(|_| HighwayError::InvalidInput)?;
        frame.extend_from_slice(&head_len.to_be_bytes());
        frame.extend_from_slice(&0_u32.to_be_bytes());
        frame.extend_from_slice(&head);
        frame.push(0x29);
        let response = decode_upload_response(&frame)?;
        assert_eq!(response.sequence(), Some(8));
        assert_eq!(response.offset(), Some(10));
        assert_eq!(response.data_length(), Some(5));
        Ok(())
    }
}

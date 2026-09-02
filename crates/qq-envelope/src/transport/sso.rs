use qq_wire::{LengthPrefix, WireReader, WireWriter};

use crate::transport::{EnvelopeError, MAX_PACKET_LEN};

const FIXED_HEADER_VALUE: [u8; 12] = [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
const MAX_COMMAND_LEN: usize = 128;
const MAX_VERSION_LEN: usize = 64;
const MAX_RESPONSE_EXTENSION_LEN: usize = 4 * 1024;
const GUID_HEX_LEN: usize = 32;

/// Borrowed ordinary fields for one signed SSO request.
#[derive(Clone, Copy, Debug)]
pub struct SsoRequestParts<'a> {
    /// SSO sequence number.
    pub sequence: u32,
    /// Ordinary sub-application identifier.
    pub sub_app_id: u32,
    /// Locale identifier.
    pub locale_id: u32,
    /// Existing session ticket, empty during QR login.
    pub tgt: &'a [u8],
    /// Ordinary QQ command.
    pub command: &'a str,
    /// Device GUID rendered as exactly 32 hexadecimal ASCII bytes.
    pub device_guid_hex: &'a [u8],
    /// Upstream client version.
    pub client_version: &'a str,
    /// Serialized reserve returned by the selected signing service.
    pub reserve: &'a [u8],
    /// Signed command body, or a service-provided exact replacement body.
    pub payload: &'a [u8],
}

/// Decoded inbound SSO response with opaque reserve bytes.
#[derive(Clone, Eq, PartialEq)]
pub struct SsoResponse {
    sequence: u32,
    return_code: i32,
    command: String,
    extra: String,
    reserve: Box<[u8]>,
    extension: Box<[u8]>,
    payload: Box<[u8]>,
}

impl SsoResponse {
    /// Returns the response sequence.
    #[must_use]
    pub const fn sequence(&self) -> u32 {
        self.sequence
    }

    /// Returns the ordinary QQ command.
    #[must_use]
    pub fn command(&self) -> &str {
        &self.command
    }

    /// Returns the remote result code.
    #[must_use]
    pub const fn return_code(&self) -> i32 {
        self.return_code
    }

    /// Returns the remote public diagnostic text.
    #[must_use]
    pub fn extra(&self) -> &str {
        &self.extra
    }

    /// Borrows the opaque response reserve.
    #[must_use]
    pub fn reserve(&self) -> &[u8] {
        &self.reserve
    }

    /// Borrows opaque response-header extension bytes.
    #[must_use]
    pub fn extension(&self) -> &[u8] {
        &self.extension
    }

    /// Borrows the ordinary command response body.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

impl core::fmt::Debug for SsoResponse {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("SsoResponse")
            .field("sequence", &self.sequence)
            .field("return_code", &self.return_code)
            .field("command", &self.command)
            .field("extra", &self.extra)
            .field("reserve_len", &self.reserve.len())
            .field("extension_len", &self.extension.len())
            .field("payload_len", &self.payload.len())
            .finish()
    }
}

/// Encodes one protocol-12 SSO request without interpreting reserve bytes.
///
/// # Errors
///
/// Returns an error for invalid ordinary fields or exceeded packet bounds.
pub fn encode_sso_request(parts: SsoRequestParts<'_>) -> Result<Vec<u8>, EnvelopeError> {
    validate(&parts)?;
    let mut header = WireWriter::new(MAX_PACKET_LEN);
    header.put_u32(parts.sequence)?;
    header.put_u32(parts.sub_app_id)?;
    header.put_u32(parts.locale_id)?;
    header.put_bytes(&FIXED_HEADER_VALUE)?;
    header.put_prefixed_bytes(LengthPrefix::U32Inclusive, parts.tgt)?;
    header.put_prefixed_bytes(LengthPrefix::U32Inclusive, parts.command.as_bytes())?;
    header.put_prefixed_bytes(LengthPrefix::U32Inclusive, &[])?;
    header.put_prefixed_bytes(LengthPrefix::U32Inclusive, parts.device_guid_hex)?;
    header.put_prefixed_bytes(LengthPrefix::U32Inclusive, &[])?;
    header.put_prefixed_bytes(LengthPrefix::U16Inclusive, parts.client_version.as_bytes())?;
    header.put_prefixed_bytes(LengthPrefix::U32Inclusive, parts.reserve)?;

    let mut output = WireWriter::new(MAX_PACKET_LEN);
    output.put_prefixed_bytes(LengthPrefix::U32Inclusive, &header.finish())?;
    output.put_prefixed_bytes(LengthPrefix::U32Inclusive, parts.payload)?;
    Ok(output.finish())
}

/// Decodes one uncompressed inbound SSO response.
///
/// # Errors
///
/// Returns an error for malformed fields, invalid UTF-8 or unsupported compression.
pub fn decode_sso_response(encoded: &[u8]) -> Result<SsoResponse, EnvelopeError> {
    let mut reader = WireReader::new(encoded);
    let header = reader.read_prefixed_bytes(LengthPrefix::U32Inclusive, MAX_PACKET_LEN)?;
    let payload = reader
        .read_prefixed_bytes(LengthPrefix::U32Inclusive, MAX_PACKET_LEN)?
        .to_vec();
    reader.finish()?;
    let mut header = WireReader::new(header);
    let sequence = header.read_u32()?;
    let return_code = header.read_i32()?;
    let extra = read_text(&mut header, 4 * 1024)?;
    let command = read_text(&mut header, MAX_COMMAND_LEN)?;
    let _message_cookie = header.read_prefixed_bytes(LengthPrefix::U32Inclusive, 4 * 1024)?;
    let compression = header.read_i32()?;
    let reserve = header
        .read_prefixed_bytes(LengthPrefix::U32Inclusive, 64 * 1024)?
        .to_vec()
        .into_boxed_slice();
    if header.remaining() > MAX_RESPONSE_EXTENSION_LEN {
        return Err(EnvelopeError::InvalidField);
    }
    let extension = header
        .read_bytes(header.remaining())?
        .to_vec()
        .into_boxed_slice();
    header.finish()?;
    if sequence == 0 || command.is_empty() {
        return Err(EnvelopeError::InvalidField);
    }
    if !matches!(compression, 0 | 4) {
        return Err(EnvelopeError::Unsupported);
    }
    Ok(SsoResponse {
        sequence,
        return_code,
        command,
        extra,
        reserve,
        extension,
        payload: payload.into_boxed_slice(),
    })
}

fn read_text(reader: &mut WireReader<'_>, maximum: usize) -> Result<String, EnvelopeError> {
    let bytes = reader.read_prefixed_bytes(LengthPrefix::U32Inclusive, maximum)?;
    core::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_error| EnvelopeError::InvalidField)
}

fn validate(parts: &SsoRequestParts<'_>) -> Result<(), EnvelopeError> {
    if parts.sequence == 0
        || parts.sub_app_id == 0
        || parts.locale_id == 0
        || parts.command.is_empty()
        || parts.command.len() > MAX_COMMAND_LEN
        || !parts.command.bytes().all(|byte| byte.is_ascii_graphic())
        || parts.device_guid_hex.len() != GUID_HEX_LEN
        || !parts.device_guid_hex.iter().all(u8::is_ascii_hexdigit)
        || parts.client_version.is_empty()
        || parts.client_version.len() > MAX_VERSION_LEN
        || !parts
            .client_version
            .bytes()
            .all(|byte| byte.is_ascii_graphic())
        || parts.payload.is_empty()
    {
        Err(EnvelopeError::InvalidField)
    } else {
        Ok(())
    }
}

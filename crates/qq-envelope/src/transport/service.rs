use qq_wire::{LengthPrefix, WireReader, WireWriter};

use crate::transport::{EnvelopeError, MAX_PACKET_LEN};
use crate::{QqTeaKey, decrypt_qq_tea, encrypt_qq_tea};

const PROTOCOL_VERSION: u32 = 12;

/// Borrowed fields for one outer protocol-12 service frame.
#[derive(Clone, Copy, Debug)]
pub struct ServiceFrameParts<'a> {
    /// Numeric QQ account identifier, zero during QR fetch.
    pub uin: u32,
    /// Existing D2 ticket, empty during QR fetch.
    pub d2: &'a [u8],
    /// D2 TEA key, or an all-zero key during QR fetch.
    pub d2_key: &'a QqTeaKey,
    /// Encoded SSO payload.
    pub sso: &'a [u8],
}

/// Decrypted inbound protocol-12 service payload.
#[derive(Clone, Eq, PartialEq)]
pub struct ServiceResponse {
    uin: String,
    payload: Box<[u8]>,
}

impl ServiceResponse {
    /// Returns the numeric account string carried by the frame.
    #[must_use]
    pub fn uin(&self) -> &str {
        &self.uin
    }

    /// Borrows the decrypted SSO response.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

impl core::fmt::Debug for ServiceResponse {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ServiceResponse")
            .field("uin", &self.uin)
            .field("payload_len", &self.payload.len())
            .finish()
    }
}

/// Encrypts and encodes one length-delimited protocol-12 service frame.
///
/// # Errors
///
/// Returns an error for empty SSO data, QQ TEA failure or exceeded bounds.
pub fn encode_service_frame(parts: ServiceFrameParts<'_>) -> Result<Vec<u8>, EnvelopeError> {
    if parts.sso.is_empty() {
        return Err(EnvelopeError::InvalidField);
    }
    let zero_key = QqTeaKey::new([0; QqTeaKey::LENGTH]);
    let (auth_flag, encryption_key) = if parts.d2.is_empty() {
        (2, &zero_key)
    } else {
        (1, parts.d2_key)
    };
    let encrypted = encrypt_qq_tea(parts.sso, encryption_key)?;
    let mut body = WireWriter::new(MAX_PACKET_LEN);
    body.put_u32(PROTOCOL_VERSION)?;
    body.put_u8(auth_flag)?;
    body.put_prefixed_bytes(LengthPrefix::U32Inclusive, parts.d2)?;
    body.put_u8(0)?;
    body.put_prefixed_bytes(LengthPrefix::U32Inclusive, parts.uin.to_string().as_bytes())?;
    body.put_bytes(&encrypted)?;

    let mut output = WireWriter::new(MAX_PACKET_LEN);
    output.put_prefixed_bytes(LengthPrefix::U32Inclusive, &body.finish())?;
    Ok(output.finish())
}

/// Decodes one inbound protocol-12 service frame.
///
/// # Errors
///
/// Returns an error for malformed framing, invalid account text or missing keys.
pub fn decode_service_response(
    encoded: &[u8],
    d2_key: Option<&QqTeaKey>,
) -> Result<ServiceResponse, EnvelopeError> {
    let mut outer = WireReader::new(encoded);
    let body = outer.read_prefixed_bytes(LengthPrefix::U32Inclusive, MAX_PACKET_LEN)?;
    outer.finish()?;
    let mut reader = WireReader::new(body);
    if reader.read_u32()? != PROTOCOL_VERSION {
        return Err(EnvelopeError::Unsupported);
    }
    let auth_flag = reader.read_u8()?;
    let _flag = reader.read_u8()?;
    let uin_bytes = reader.read_prefixed_bytes(LengthPrefix::U32Inclusive, 32)?;
    if uin_bytes.is_empty() || !uin_bytes.iter().all(u8::is_ascii_digit) {
        return Err(EnvelopeError::InvalidField);
    }
    let uin = core::str::from_utf8(uin_bytes)
        .map_err(|_error| EnvelopeError::InvalidField)?
        .to_owned();
    let encrypted = reader.read_bytes(reader.remaining())?;
    reader.finish()?;
    let payload = match auth_flag {
        0 => encrypted.to_vec(),
        1 => decrypt_qq_tea(encrypted, d2_key.ok_or(EnvelopeError::InvalidField)?)?,
        2 => decrypt_qq_tea(encrypted, &QqTeaKey::new([0; 16]))?,
        _ => return Err(EnvelopeError::Unsupported),
    };
    if payload.is_empty() {
        return Err(EnvelopeError::InvalidField);
    }
    Ok(ServiceResponse {
        uin,
        payload: payload.into_boxed_slice(),
    })
}

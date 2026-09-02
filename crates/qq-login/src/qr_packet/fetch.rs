use qq_envelope::{QqTeaKey, encrypt_qq_tea};
use qq_profile::LinuxNtProfile;
use qq_wire::{LengthPrefix, WireWriter};

use crate::qr_packet::packet::{build_request_data, build_transaction, build_wtlogin_packet};
use crate::qr_packet::tlv::build_fetch_tlvs;
use crate::{QqKeyAgreement, QrDevice, QrPacketError};

const COMMAND: &str = "wtlogin.trans_emp";
const QR_FETCH_COMMAND: u16 = 0x31;
const MAX_LOGIN_PACKET_LEN: usize = 64 * 1024;

/// Borrowed inputs for one QR fetch command body.
#[derive(Clone, Copy)]
pub struct QrFetchContext<'a> {
    /// Selected validated Linux profile.
    pub profile: &'a LinuxNtProfile,
    /// Stable local device values.
    pub device: &'a QrDevice,
    /// SSO request sequence.
    pub sso_sequence: u32,
    /// Current Unix timestamp in seconds.
    pub unix_seconds: u32,
    /// Per-installation random login header value.
    pub random_key: &'a QqTeaKey,
    /// Ephemeral login key agreement.
    pub key_agreement: &'a dyn QqKeyAgreement,
}

/// Unsigned ordinary request presented to the Ceylith signing boundary.
#[derive(Clone, Eq, PartialEq)]
pub struct QrUnsignedRequest {
    sequence: u32,
    payload: Box<[u8]>,
}

impl QrUnsignedRequest {
    pub(super) fn new(sequence: u32, payload: Vec<u8>) -> Self {
        Self {
            sequence,
            payload: payload.into_boxed_slice(),
        }
    }

    /// Returns the ordinary QQ command.
    #[must_use]
    pub const fn command(&self) -> &'static str {
        COMMAND
    }

    /// Returns the SSO sequence.
    #[must_use]
    pub const fn sequence(&self) -> u32 {
        self.sequence
    }

    /// Borrows the exact command body to sign.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

impl core::fmt::Debug for QrUnsignedRequest {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("QrUnsignedRequest")
            .field("sequence", &self.sequence)
            .field("payload_len", &self.payload.len())
            .finish()
    }
}

/// Builds the encrypted `WtLogin` QR-fetch body that must be signed before SSO framing.
///
/// # Errors
///
/// Returns an error for zero sequences, invalid key width, bounds or encryption failure.
pub fn build_qr_fetch(context: QrFetchContext<'_>) -> Result<QrUnsignedRequest, QrPacketError> {
    if context.sso_sequence == 0
        || context.unix_seconds == 0
        || context.key_agreement.public_key().len() != 25
    {
        return Err(QrPacketError::InvalidField);
    }
    let body = build_fetch_body(context.profile, context.device)?;
    let transaction = build_transaction(QR_FETCH_COMMAND, &body)?;
    let data = build_request_data(context.profile, context.unix_seconds, &transaction)?;
    let encrypted = encrypt_qq_tea(&data, context.key_agreement.tea_key())?;
    let payload = build_wtlogin_packet(
        context.profile,
        context.random_key,
        context.key_agreement.public_key(),
        &encrypted,
    )?;
    Ok(QrUnsignedRequest::new(context.sso_sequence, payload))
}

fn build_fetch_body(profile: &LinuxNtProfile, device: &QrDevice) -> Result<Vec<u8>, QrPacketError> {
    let tlvs = build_fetch_tlvs(profile, device)?;
    let mut body = WireWriter::new(MAX_LOGIN_PACKET_LEN);
    body.put_u16(0)?;
    body.put_u32(profile.app_id())?;
    body.put_u64(0)?;
    body.put_u8(0)?;
    body.put_prefixed_bytes(LengthPrefix::U16Payload, &[])?;
    body.put_bytes(&tlvs)?;
    Ok(body.finish())
}

use crate::QqTeaKey;
use crate::transport::{
    EnvelopeError, ServiceFrameParts, SsoRequestParts, SsoResponse, decode_service_response,
    decode_sso_response, encode_service_frame, encode_sso_request,
};

/// Borrowed authentication state for one QQ transport generation.
#[derive(Clone, Copy)]
pub enum SessionAuth<'a> {
    /// Login-stage envelope without D2 authentication.
    Anonymous {
        /// Numeric account identifier, or zero before it is known.
        uin: u32,
    },
    /// Post-login envelope authenticated with issued session material.
    Authenticated {
        /// Numeric account identifier bound to the session.
        uin: u32,
        /// TGT carried inside the SSO header.
        tgt: &'a [u8],
        /// D2 carried by the outer service frame.
        d2: &'a [u8],
        /// D2 key used for outer-frame encryption.
        d2_key: &'a QqTeaKey,
    },
}

impl<'a> SessionAuth<'a> {
    /// Creates login-stage unauthenticated envelope state.
    #[must_use]
    pub const fn anonymous(uin: u32) -> Self {
        Self::Anonymous { uin }
    }

    /// Creates validated post-login envelope state.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero account or missing session ticket.
    pub fn authenticated(
        uin: u32,
        tgt: &'a [u8],
        d2: &'a [u8],
        d2_key: &'a QqTeaKey,
    ) -> Result<Self, EnvelopeError> {
        if uin == 0 || tgt.is_empty() || d2.is_empty() {
            return Err(EnvelopeError::InvalidField);
        }
        Ok(Self::Authenticated {
            uin,
            tgt,
            d2,
            d2_key,
        })
    }

    const fn uin(&self) -> u32 {
        match self {
            Self::Anonymous { uin } | Self::Authenticated { uin, .. } => *uin,
        }
    }

    const fn tgt(&self) -> &[u8] {
        match self {
            Self::Anonymous { .. } => &[],
            Self::Authenticated { tgt, .. } => tgt,
        }
    }

    const fn d2(&self) -> &[u8] {
        match self {
            Self::Anonymous { .. } => &[],
            Self::Authenticated { d2, .. } => d2,
        }
    }

    const fn d2_key(&self) -> Option<&QqTeaKey> {
        match self {
            Self::Anonymous { .. } => None,
            Self::Authenticated { d2_key, .. } => Some(d2_key),
        }
    }
}

impl core::fmt::Debug for SessionAuth<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Anonymous { .. } => "SessionAuth::Anonymous(<redacted>)",
            Self::Authenticated { .. } => "SessionAuth::Authenticated(<redacted>)",
        })
    }
}

/// Borrowed fields for one complete SSO-over-service request.
#[derive(Clone, Copy)]
pub struct SessionRequestParts<'a> {
    /// Current transport authentication state.
    pub auth: &'a SessionAuth<'a>,
    /// SSO sequence number.
    pub sequence: u32,
    /// Ordinary sub-application identifier.
    pub sub_app_id: u32,
    /// Locale identifier supplied by the active Profile.
    pub locale_id: u32,
    /// Ordinary QQ command supplied by the active Profile or compiled login flow.
    pub command: &'a str,
    /// Device GUID rendered as hexadecimal ASCII.
    pub device_guid_hex: &'a [u8],
    /// Upstream client version.
    pub client_version: &'a str,
    /// Opaque reserve bytes.
    pub reserve: &'a [u8],
    /// Ordinary request body.
    pub payload: &'a [u8],
}

/// Expected correlation fields for one complete inbound response.
#[derive(Clone, Copy)]
pub struct ExpectedSsoResponse<'a> {
    /// Current transport authentication state.
    pub auth: &'a SessionAuth<'a>,
    /// Request sequence that must be echoed by QQ.
    pub sequence: u32,
    /// Request command that must be echoed by QQ.
    pub command: &'a str,
}

/// Encodes one complete request without duplicating anonymous/authenticated paths.
///
/// # Errors
///
/// Returns an error for invalid fields, exceeded bounds or encryption failure.
pub fn encode_session_request(parts: SessionRequestParts<'_>) -> Result<Vec<u8>, EnvelopeError> {
    let sso = encode_sso_request(SsoRequestParts {
        sequence: parts.sequence,
        sub_app_id: parts.sub_app_id,
        locale_id: parts.locale_id,
        tgt: parts.auth.tgt(),
        command: parts.command,
        device_guid_hex: parts.device_guid_hex,
        client_version: parts.client_version,
        reserve: parts.reserve,
        payload: parts.payload,
    })?;
    let zero_key = QqTeaKey::new([0; QqTeaKey::LENGTH]);
    encode_service_frame(ServiceFrameParts {
        uin: parts.auth.uin(),
        d2: parts.auth.d2(),
        d2_key: parts.auth.d2_key().unwrap_or(&zero_key),
        sso: &sso,
    })
}

/// Decodes one complete authenticated frame and checks its account binding.
///
/// # Errors
///
/// Returns an error for malformed encryption or a mismatched account binding.
pub fn decode_session_frame(
    encoded: &[u8],
    auth: &SessionAuth<'_>,
) -> Result<SsoResponse, EnvelopeError> {
    let service = decode_service_response(encoded, auth.d2_key())?;
    if service.uin() != auth.uin().to_string() {
        return Err(EnvelopeError::InvalidField);
    }
    decode_sso_response(service.payload())
}

/// Decodes one complete response and checks account, sequence and command binding.
///
/// # Errors
///
/// Returns an error for malformed encryption or a mismatched response binding.
pub fn decode_session_response(
    encoded: &[u8],
    expected: ExpectedSsoResponse<'_>,
) -> Result<SsoResponse, EnvelopeError> {
    let sso = decode_session_frame(encoded, expected.auth)?;
    if sso.sequence() != expected.sequence || sso.command() != expected.command {
        return Err(EnvelopeError::InvalidField);
    }
    Ok(sso)
}

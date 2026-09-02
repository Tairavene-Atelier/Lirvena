use core::fmt;

use qrcode::QrCode;
use sha2::{Digest, Sha256};

use crate::QrArtifactError;

const MAX_QR_URL_LEN: usize = 2_048;
const MAX_QR_PNG_LEN: usize = 1024 * 1024;
const MAX_QR_LIFETIME_SECONDS: u32 = 15 * 60;
const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

/// Validated QR login material suitable for terminal and PNG presentation.
#[derive(Clone, Eq, PartialEq)]
pub struct QrArtifact {
    url: String,
    png: Box<[u8]>,
    png_sha256: [u8; 32],
    issued_at_ms: u64,
    expires_at_ms: u64,
}

impl QrArtifact {
    /// Validates and stores one QR response.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid URL, invalid PNG or unsafe lifetime.
    pub fn new(
        url: String,
        png: impl Into<Box<[u8]>>,
        issued_at_ms: u64,
        lifetime_seconds: u32,
    ) -> Result<Self, QrArtifactError> {
        if !valid_url(&url) {
            return Err(QrArtifactError::InvalidUrl);
        }
        let png = png.into();
        if !valid_png(&png) {
            return Err(QrArtifactError::InvalidPng);
        }
        if lifetime_seconds == 0 || lifetime_seconds > MAX_QR_LIFETIME_SECONDS {
            return Err(QrArtifactError::InvalidLifetime);
        }
        let lifetime_ms = u64::from(lifetime_seconds)
            .checked_mul(1_000)
            .ok_or(QrArtifactError::InvalidLifetime)?;
        let expires_at_ms = issued_at_ms
            .checked_add(lifetime_ms)
            .ok_or(QrArtifactError::InvalidLifetime)?;
        let png_sha256 = Sha256::digest(&png).into();
        Ok(Self {
            url,
            png,
            png_sha256,
            issued_at_ms,
            expires_at_ms,
        })
    }

    /// Borrows the QR URL used to render an exact terminal matrix.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Borrows the upstream PNG bytes.
    #[must_use]
    pub fn png(&self) -> &[u8] {
        &self.png
    }

    /// Returns the SHA-256 digest of the PNG artifact.
    #[must_use]
    pub const fn png_sha256(&self) -> [u8; 32] {
        self.png_sha256
    }

    /// Returns the local receipt time.
    #[must_use]
    pub const fn issued_at_ms(&self) -> u64 {
        self.issued_at_ms
    }

    /// Returns the exclusive local expiry time.
    #[must_use]
    pub const fn expires_at_ms(&self) -> u64 {
        self.expires_at_ms
    }

    /// Renders the exact QR URL as a compact Unicode terminal matrix.
    ///
    /// # Errors
    ///
    /// Returns an error when the URL exceeds the QR encoder capacity.
    pub fn terminal_text(&self) -> Result<String, QrArtifactError> {
        let code =
            QrCode::new(self.url.as_bytes()).map_err(|_error| QrArtifactError::MatrixEncoding)?;
        Ok(code
            .render::<char>()
            .quiet_zone(true)
            .module_dimensions(2, 1)
            .build())
    }

    /// Returns whether the artifact has expired at `now_ms`.
    #[must_use]
    pub const fn is_expired_at(&self, now_ms: u64) -> bool {
        now_ms >= self.expires_at_ms
    }
}

impl fmt::Debug for QrArtifact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QrArtifact")
            .field("url", &"<redacted>")
            .field("png_len", &self.png.len())
            .field("png_sha256", &self.png_sha256)
            .field("issued_at_ms", &self.issued_at_ms)
            .field("expires_at_ms", &self.expires_at_ms)
            .finish()
    }
}

fn valid_url(url: &str) -> bool {
    url.starts_with("https://")
        && url.len() <= MAX_QR_URL_LEN
        && url.bytes().all(|byte| byte.is_ascii_graphic())
}

fn valid_png(png: &[u8]) -> bool {
    png.len() > PNG_SIGNATURE.len() && png.len() <= MAX_QR_PNG_LEN && png.starts_with(PNG_SIGNATURE)
}

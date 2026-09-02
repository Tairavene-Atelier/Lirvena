use core::fmt;

use zeroize::{Zeroize, ZeroizeOnDrop};

/// Sixteen-byte QQ TEA key with redacted debug output and zeroizing drop.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct QqTeaKey([u8; Self::LENGTH]);

impl QqTeaKey {
    /// Exact key width.
    pub const LENGTH: usize = 16;

    /// Takes ownership of exact key bytes.
    #[must_use]
    pub const fn new(bytes: [u8; Self::LENGTH]) -> Self {
        Self(bytes)
    }

    /// Borrows the exact key bytes for a protocol field that explicitly carries them.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; Self::LENGTH] {
        &self.0
    }

    pub(crate) const fn words(&self) -> [u32; 4] {
        let bytes = &self.0;
        [
            u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
            u32::from_be_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]),
        ]
    }
}

impl fmt::Debug for QqTeaKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("QqTeaKey(<redacted>)")
    }
}

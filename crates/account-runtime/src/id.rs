/// Installation-local account identifier that never contains a QQ number.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AccountLocalId([u8; Self::LENGTH]);

impl AccountLocalId {
    /// Fixed identifier width.
    pub const LENGTH: usize = 16;

    /// Creates an identifier from opaque installation-local bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; Self::LENGTH]) -> Self {
        Self(bytes)
    }

    /// Returns the opaque identifier bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; Self::LENGTH] {
        &self.0
    }

    pub(crate) fn file_stem(self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut output = String::with_capacity(Self::LENGTH * 2);
        for byte in self.0 {
            output.push(char::from(HEX[usize::from(byte >> 4)]));
            output.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        output
    }

    pub(crate) fn database_path(self, directory: &Path) -> PathBuf {
        directory.join(format!("{}.sqlite3", self.file_stem()))
    }
}

impl core::fmt::Debug for AccountLocalId {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("AccountLocalId")
            .field(&self.file_stem())
            .finish()
    }
}
use std::path::{Path, PathBuf};

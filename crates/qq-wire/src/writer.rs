use crate::{LengthPrefix, WireError};

/// Bounded writer for big-endian QQ values.
#[derive(Clone, Debug)]
pub struct WireWriter {
    output: Vec<u8>,
    maximum: usize,
}

impl WireWriter {
    /// Creates an empty writer with a hard output bound.
    #[must_use]
    pub const fn new(maximum: usize) -> Self {
        Self {
            output: Vec::new(),
            maximum,
        }
    }

    /// Returns the encoded length.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.output.len()
    }

    /// Returns whether no bytes have been written.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.output.is_empty()
    }

    /// Writes one byte.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured output bound would be exceeded.
    pub fn put_u8(&mut self, value: u8) -> Result<(), WireError> {
        self.put_bytes(&[value])
    }

    /// Writes one big-endian `u16`.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured output bound would be exceeded.
    pub fn put_u16(&mut self, value: u16) -> Result<(), WireError> {
        self.put_bytes(&value.to_be_bytes())
    }

    /// Writes one big-endian `u32`.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured output bound would be exceeded.
    pub fn put_u32(&mut self, value: u32) -> Result<(), WireError> {
        self.put_bytes(&value.to_be_bytes())
    }

    /// Writes one big-endian `u64`.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured output bound would be exceeded.
    pub fn put_u64(&mut self, value: u64) -> Result<(), WireError> {
        self.put_bytes(&value.to_be_bytes())
    }

    /// Writes raw bytes after checked growth.
    ///
    /// # Errors
    ///
    /// Returns an error on length overflow or an exceeded output bound.
    pub fn put_bytes(&mut self, bytes: &[u8]) -> Result<(), WireError> {
        self.ensure_growth(bytes.len())?;
        self.output.extend_from_slice(bytes);
        Ok(())
    }

    /// Writes one bounded length-prefixed byte field.
    ///
    /// # Errors
    ///
    /// Returns an error when the field cannot fit its prefix or the output bound.
    pub fn put_prefixed_bytes(
        &mut self,
        prefix: LengthPrefix,
        bytes: &[u8],
    ) -> Result<(), WireError> {
        let declared = if prefix.includes_prefix() {
            bytes
                .len()
                .checked_add(prefix.width())
                .ok_or(WireError::LengthOverflow)?
        } else {
            bytes.len()
        };
        match prefix {
            LengthPrefix::U8Payload | LengthPrefix::U8Inclusive => {
                let length =
                    u8::try_from(declared).map_err(|_error| WireError::LengthLimitExceeded {
                        limit: usize::from(u8::MAX),
                        actual: declared,
                    })?;
                self.put_u8(length)?;
            }
            LengthPrefix::U16Payload | LengthPrefix::U16Inclusive => {
                let length =
                    u16::try_from(declared).map_err(|_error| WireError::LengthLimitExceeded {
                        limit: usize::from(u16::MAX),
                        actual: declared,
                    })?;
                self.put_u16(length)?;
            }
            LengthPrefix::U32Payload | LengthPrefix::U32Inclusive => {
                let length =
                    u32::try_from(declared).map_err(|_error| WireError::LengthLimitExceeded {
                        limit: usize::try_from(u32::MAX).unwrap_or(usize::MAX),
                        actual: declared,
                    })?;
                self.put_u32(length)?;
            }
        }
        self.put_bytes(bytes)
    }

    /// Returns the finished packet.
    #[must_use]
    pub fn finish(self) -> Vec<u8> {
        self.output
    }

    fn ensure_growth(&self, additional: usize) -> Result<(), WireError> {
        let length = self
            .output
            .len()
            .checked_add(additional)
            .ok_or(WireError::LengthOverflow)?;
        if length > self.maximum {
            return Err(WireError::LengthLimitExceeded {
                limit: self.maximum,
                actual: length,
            });
        }
        Ok(())
    }
}

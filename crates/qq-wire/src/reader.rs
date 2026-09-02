use crate::{LengthPrefix, WireError};

/// Forward-only bounded reader for big-endian QQ values.
#[derive(Clone, Debug)]
pub struct WireReader<'a> {
    input: &'a [u8],
    cursor: usize,
}

impl<'a> WireReader<'a> {
    /// Creates a reader over one already-bounded packet.
    #[must_use]
    pub const fn new(input: &'a [u8]) -> Self {
        Self { input, cursor: 0 }
    }

    /// Returns the current cursor offset.
    #[must_use]
    pub const fn position(&self) -> usize {
        self.cursor
    }

    /// Returns the number of bytes left in this packet.
    #[must_use]
    pub const fn remaining(&self) -> usize {
        self.input.len() - self.cursor
    }

    /// Reads one byte.
    ///
    /// # Errors
    ///
    /// Returns an error when the input is truncated.
    pub fn read_u8(&mut self) -> Result<u8, WireError> {
        let bytes = self.read_bytes(1)?;
        Ok(bytes[0])
    }

    /// Reads one big-endian `u16`.
    ///
    /// # Errors
    ///
    /// Returns an error when the input is truncated.
    pub fn read_u16(&mut self) -> Result<u16, WireError> {
        let bytes = self.read_array::<2>()?;
        Ok(u16::from_be_bytes(bytes))
    }

    /// Reads one big-endian `u32`.
    ///
    /// # Errors
    ///
    /// Returns an error when the input is truncated.
    pub fn read_u32(&mut self) -> Result<u32, WireError> {
        let bytes = self.read_array::<4>()?;
        Ok(u32::from_be_bytes(bytes))
    }

    /// Reads one big-endian `i32`.
    ///
    /// # Errors
    ///
    /// Returns an error when the input is truncated.
    pub fn read_i32(&mut self) -> Result<i32, WireError> {
        let bytes = self.read_array::<4>()?;
        Ok(i32::from_be_bytes(bytes))
    }

    /// Reads one big-endian `u64`.
    ///
    /// # Errors
    ///
    /// Returns an error when the input is truncated.
    pub fn read_u64(&mut self) -> Result<u64, WireError> {
        let bytes = self.read_array::<8>()?;
        Ok(u64::from_be_bytes(bytes))
    }

    /// Borrows exactly `length` bytes.
    ///
    /// # Errors
    ///
    /// Returns an error on length overflow or truncated input.
    pub fn read_bytes(&mut self, length: usize) -> Result<&'a [u8], WireError> {
        let end = self
            .cursor
            .checked_add(length)
            .ok_or(WireError::LengthOverflow)?;
        if end > self.input.len() {
            return Err(WireError::Truncated {
                needed: length,
                available: self.remaining(),
            });
        }
        let bytes = &self.input[self.cursor..end];
        self.cursor = end;
        Ok(bytes)
    }

    /// Reads a prefixed byte field and enforces a caller-selected payload bound.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid lengths, exceeded bounds or truncated input.
    pub fn read_prefixed_bytes(
        &mut self,
        prefix: LengthPrefix,
        maximum: usize,
    ) -> Result<&'a [u8], WireError> {
        let declared = match prefix {
            LengthPrefix::U8Payload | LengthPrefix::U8Inclusive => usize::from(self.read_u8()?),
            LengthPrefix::U16Payload | LengthPrefix::U16Inclusive => usize::from(self.read_u16()?),
            LengthPrefix::U32Payload | LengthPrefix::U32Inclusive => {
                usize::try_from(self.read_u32()?).map_err(|_error| WireError::LengthOverflow)?
            }
        };
        let length = if prefix.includes_prefix() {
            declared
                .checked_sub(prefix.width())
                .ok_or(WireError::InvalidInclusiveLength)?
        } else {
            declared
        };
        if length > maximum {
            return Err(WireError::LengthLimitExceeded {
                limit: maximum,
                actual: length,
            });
        }
        self.read_bytes(length)
    }

    /// Rejects any unconsumed bytes.
    ///
    /// # Errors
    ///
    /// Returns an error when bytes remain after the declared packet.
    pub fn finish(self) -> Result<(), WireError> {
        if self.remaining() == 0 {
            Ok(())
        } else {
            Err(WireError::TrailingBytes {
                remaining: self.remaining(),
            })
        }
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], WireError> {
        let bytes = self.read_bytes(N)?;
        let mut value = [0_u8; N];
        value.copy_from_slice(bytes);
        Ok(value)
    }
}

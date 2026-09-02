use crate::OnlinePacketError;

use super::{LIST, MAP};

const BYTE: u8 = 0;
const SHORT: u8 = 1;
const INT: u8 = 2;
const LONG: u8 = 3;
const STRING_ONE: u8 = 6;
const STRING_FOUR: u8 = 7;
const ZERO: u8 = 12;
const SIMPLE_LIST: u8 = 13;

pub(in crate::push) struct Writer(Vec<u8>);

impl Writer {
    pub(in crate::push) const fn new() -> Self {
        Self(Vec::new())
    }

    pub(in crate::push) fn finish(self) -> Vec<u8> {
        self.0
    }

    pub(in crate::push) fn head(&mut self, tag: u8, kind: u8) {
        if tag < 15 {
            self.0.push((tag << 4) | kind);
        } else {
            self.0.push(0xf0 | kind);
            self.0.push(tag);
        }
    }

    pub(in crate::push) fn integer(&mut self, tag: u8, value: i64) {
        if value == 0 {
            self.head(tag, ZERO);
        } else if let Ok(value) = i8::try_from(value) {
            self.head(tag, BYTE);
            self.0.extend_from_slice(&value.to_be_bytes());
        } else if let Ok(value) = i16::try_from(value) {
            self.head(tag, SHORT);
            self.0.extend_from_slice(&value.to_be_bytes());
        } else if let Ok(value) = i32::try_from(value) {
            self.head(tag, INT);
            self.0.extend_from_slice(&value.to_be_bytes());
        } else {
            self.head(tag, LONG);
            self.0.extend_from_slice(&value.to_be_bytes());
        }
    }

    pub(in crate::push) fn string(
        &mut self,
        tag: u8,
        value: &str,
    ) -> Result<(), OnlinePacketError> {
        let bytes = value.as_bytes();
        if let Ok(length) = u8::try_from(bytes.len()) {
            self.head(tag, STRING_ONE);
            self.0.push(length);
        } else {
            self.head(tag, STRING_FOUR);
            self.0.extend_from_slice(
                &u32::try_from(bytes.len())
                    .map_err(|_| OnlinePacketError)?
                    .to_be_bytes(),
            );
        }
        self.0.extend_from_slice(bytes);
        Ok(())
    }

    pub(in crate::push) fn bytes(
        &mut self,
        tag: u8,
        value: &[u8],
    ) -> Result<(), OnlinePacketError> {
        self.head(tag, SIMPLE_LIST);
        self.head(0, BYTE);
        self.integer(
            0,
            i64::try_from(value.len()).map_err(|_| OnlinePacketError)?,
        );
        self.0.extend_from_slice(value);
        Ok(())
    }

    pub(in crate::push) fn empty_map(&mut self, tag: u8) {
        self.head(tag, MAP);
        self.integer(0, 0);
    }

    pub(in crate::push) fn integer_list(
        &mut self,
        tag: u8,
        values: &[i64],
    ) -> Result<(), OnlinePacketError> {
        self.head(tag, LIST);
        self.integer(
            0,
            i64::try_from(values.len()).map_err(|_| OnlinePacketError)?,
        );
        for value in values {
            self.integer(0, *value);
        }
        Ok(())
    }
}

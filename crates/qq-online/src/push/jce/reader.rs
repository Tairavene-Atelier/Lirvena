use crate::OnlinePacketError;

use super::{LIST, MAP, STRUCT_BEGIN, STRUCT_END};

const BYTE: u8 = 0;
const SHORT: u8 = 1;
const INT: u8 = 2;
const LONG: u8 = 3;
const STRING_ONE: u8 = 6;
const STRING_FOUR: u8 = 7;
const ZERO: u8 = 12;
const SIMPLE_LIST: u8 = 13;
const MAX_VALUE_LEN: usize = 1024 * 1024;
const MAX_DEPTH: usize = 24;

#[derive(Clone, Copy)]
pub(in crate::push) struct Head {
    pub(in crate::push) tag: u8,
    pub(in crate::push) kind: u8,
}

pub(in crate::push) struct Reader<'a> {
    input: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    pub(in crate::push) const fn new(input: &'a [u8]) -> Self {
        Self { input, position: 0 }
    }

    pub(in crate::push) const fn remaining(&self) -> usize {
        self.input.len() - self.position
    }

    pub(in crate::push) fn head(&mut self) -> Result<Head, OnlinePacketError> {
        let raw = self.byte()?;
        let mut tag = raw >> 4;
        let kind = raw & 0x0f;
        if tag == 15 {
            tag = self.byte()?;
        }
        Ok(Head { tag, kind })
    }

    pub(in crate::push) fn integer(&mut self, kind: u8) -> Result<i64, OnlinePacketError> {
        match kind {
            ZERO => Ok(0),
            BYTE => Ok(i64::from(i8::from_be_bytes([self.byte()?]))),
            SHORT => Ok(i64::from(i16::from_be_bytes(self.array()?))),
            INT => Ok(i64::from(i32::from_be_bytes(self.array()?))),
            LONG => Ok(i64::from_be_bytes(self.array()?)),
            _ => Err(OnlinePacketError),
        }
    }

    pub(in crate::push) fn string(&mut self, kind: u8) -> Result<String, OnlinePacketError> {
        core::str::from_utf8(self.string_bytes(kind)?)
            .map(str::to_owned)
            .map_err(|_| OnlinePacketError)
    }

    pub(in crate::push) fn bytes(&mut self, kind: u8) -> Result<&'a [u8], OnlinePacketError> {
        if kind != SIMPLE_LIST {
            return self.string_bytes(kind);
        }
        if self.head()?.kind != BYTE {
            return Err(OnlinePacketError);
        }
        let length = self.collection_count(MAX_VALUE_LEN)?;
        self.take(length)
    }

    fn string_bytes(&mut self, kind: u8) -> Result<&'a [u8], OnlinePacketError> {
        let length = match kind {
            STRING_ONE => usize::from(self.byte()?),
            STRING_FOUR => {
                usize::try_from(u32::from_be_bytes(self.array()?)).map_err(|_| OnlinePacketError)?
            }
            _ => return Err(OnlinePacketError),
        };
        if length > MAX_VALUE_LEN {
            return Err(OnlinePacketError);
        }
        self.take(length)
    }

    pub(in crate::push) fn collection_count(
        &mut self,
        maximum: usize,
    ) -> Result<usize, OnlinePacketError> {
        let head = self.head()?;
        let count = usize::try_from(self.integer(head.kind)?).map_err(|_| OnlinePacketError)?;
        if count > maximum {
            return Err(OnlinePacketError);
        }
        Ok(count)
    }

    pub(in crate::push) fn integer_list(
        &mut self,
        kind: u8,
        maximum: usize,
    ) -> Result<Vec<i64>, OnlinePacketError> {
        if kind != LIST {
            return Err(OnlinePacketError);
        }
        let count = self.collection_count(maximum)?;
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            let head = self.head()?;
            values.push(self.integer(head.kind)?);
        }
        Ok(values)
    }

    pub(in crate::push) fn skip(
        &mut self,
        kind: u8,
        depth: usize,
    ) -> Result<(), OnlinePacketError> {
        if depth > MAX_DEPTH {
            return Err(OnlinePacketError);
        }
        match kind {
            ZERO | STRUCT_END => Ok(()),
            BYTE => self.advance(1),
            SHORT => self.advance(2),
            INT | 4 => self.advance(4),
            LONG | 5 => self.advance(8),
            STRING_ONE | STRING_FOUR => {
                let _value = self.string_bytes(kind)?;
                Ok(())
            }
            SIMPLE_LIST => {
                if self.head()?.kind != BYTE {
                    return Err(OnlinePacketError);
                }
                let count = self.collection_count(MAX_VALUE_LEN)?;
                self.advance(count)
            }
            MAP => self.skip_collection(depth, true),
            LIST => self.skip_collection(depth, false),
            STRUCT_BEGIN => self.skip_struct(depth),
            _ => Err(OnlinePacketError),
        }
    }

    fn skip_collection(&mut self, depth: usize, pairs: bool) -> Result<(), OnlinePacketError> {
        let mut count = self.collection_count(1_000_000)?;
        if pairs {
            count = count.checked_mul(2).ok_or(OnlinePacketError)?;
        }
        for _ in 0..count {
            let head = self.head()?;
            self.skip(head.kind, depth + 1)?;
        }
        Ok(())
    }

    fn skip_struct(&mut self, depth: usize) -> Result<(), OnlinePacketError> {
        while self.remaining() != 0 {
            let head = self.head()?;
            if head.kind == STRUCT_END {
                return Ok(());
            }
            self.skip(head.kind, depth + 1)?;
        }
        Err(OnlinePacketError)
    }

    fn byte(&mut self) -> Result<u8, OnlinePacketError> {
        let value = *self.input.get(self.position).ok_or(OnlinePacketError)?;
        self.position += 1;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], OnlinePacketError> {
        self.take(N)?.try_into().map_err(|_| OnlinePacketError)
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], OnlinePacketError> {
        let end = self.position.checked_add(length).ok_or(OnlinePacketError)?;
        let value = self
            .input
            .get(self.position..end)
            .ok_or(OnlinePacketError)?;
        self.position = end;
        Ok(value)
    }

    fn advance(&mut self, length: usize) -> Result<(), OnlinePacketError> {
        let _value = self.take(length)?;
        Ok(())
    }
}

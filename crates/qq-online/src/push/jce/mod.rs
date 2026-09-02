mod reader;
mod writer;

pub(super) use reader::Reader;
pub(super) use writer::Writer;

use crate::OnlinePacketError;

pub(super) const MAP: u8 = 8;
pub(super) const LIST: u8 = 9;
pub(super) const STRUCT_BEGIN: u8 = 10;
pub(super) const STRUCT_END: u8 = 11;

const MAX_NAMED_VALUES: usize = 32;

pub(super) fn named_payload<'a>(
    input: &'a [u8],
    parameter_name: &str,
    type_name: &str,
) -> Result<&'a [u8], OnlinePacketError> {
    let mut reader = Reader::new(input);
    if reader.head()?.kind != MAP {
        return Err(OnlinePacketError);
    }
    for _ in 0..reader.collection_count(MAX_NAMED_VALUES)? {
        let key_head = reader.head()?;
        let key = reader.string(key_head.kind)?;
        let value_head = reader.head()?;
        if value_head.kind != MAP {
            reader.skip(value_head.kind, 0)?;
            continue;
        }
        for _ in 0..reader.collection_count(MAX_NAMED_VALUES)? {
            let type_head = reader.head()?;
            let current_type = reader.string(type_head.kind)?;
            let body_head = reader.head()?;
            let body = reader.bytes(body_head.kind)?;
            if key == parameter_name && current_type == type_name {
                return Ok(body);
            }
        }
    }
    Err(OnlinePacketError)
}

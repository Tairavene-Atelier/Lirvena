use prost::Message;

use super::GroupFileSegment;
use crate::MessageDecodeError;

const GROUP_FILE_TYPE: i32 = 24;
const MAX_TEXT_BYTES: usize = 4096;

pub(super) fn decode(input: &[u8]) -> Result<Option<GroupFileSegment>, MessageDecodeError> {
    let transfer = TransferWire::decode(input).map_err(|_error| MessageDecodeError)?;
    if transfer.kind != GROUP_FILE_TYPE {
        return Ok(None);
    }
    let length = usize::from(u16::from_be_bytes(
        transfer
            .value
            .get(1..3)
            .and_then(|value| value.try_into().ok())
            .ok_or(MessageDecodeError)?,
    ));
    let end = 3_usize.checked_add(length).ok_or(MessageDecodeError)?;
    if end != transfer.value.len() {
        return Err(MessageDecodeError);
    }
    let info = ExtraWire::decode(&transfer.value[3..end])
        .map_err(|_error| MessageDecodeError)?
        .inner
        .and_then(|value| value.info)
        .ok_or(MessageDecodeError)?;
    let size = u64::try_from(info.size).map_err(|_error| MessageDecodeError)?;
    if info.bus_id == 0
        || info.file_id.is_empty()
        || info.name.is_empty()
        || size == 0
        || [&info.file_id, &info.name]
            .into_iter()
            .any(|value| value.len() > MAX_TEXT_BYTES || value.contains('\0'))
    {
        return Err(MessageDecodeError);
    }
    Ok(Some(GroupFileSegment::new(
        info.bus_id,
        info.file_id,
        info.name,
        size,
    )))
}

#[derive(Clone, PartialEq, Message)]
struct TransferWire {
    #[prost(int32, tag = "1")]
    kind: i32,
    #[prost(bytes = "vec", tag = "2")]
    value: Vec<u8>,
}
#[derive(Clone, PartialEq, Message)]
struct ExtraWire {
    #[prost(message, optional, tag = "7")]
    inner: Option<InnerWire>,
}
#[derive(Clone, PartialEq, Message)]
struct InnerWire {
    #[prost(message, optional, tag = "2")]
    info: Option<InfoWire>,
}
#[derive(Clone, PartialEq, Message)]
struct InfoWire {
    #[prost(uint32, tag = "1")]
    bus_id: u32,
    #[prost(string, tag = "2")]
    file_id: String,
    #[prost(int64, tag = "3")]
    size: i64,
    #[prost(string, tag = "4")]
    name: String,
}

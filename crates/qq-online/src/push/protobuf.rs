use prost::Message;

use super::ProtectiveNotice;
use crate::OnlinePacketError;

const MAX_NOTICE_TEXT: usize = 4 * 1024;

#[derive(Clone, Copy, PartialEq, Message)]
struct PairEnvelope {
    #[prost(uint32, tag = "1")]
    first: u32,
    #[prost(uint64, tag = "2")]
    second: u64,
}

#[derive(Clone, PartialEq, Message)]
struct ProtectiveEnvelope {
    #[prost(uint32, tag = "1")]
    account: u32,
    #[prost(string, tag = "3")]
    detail: String,
    #[prost(string, tag = "4")]
    title: String,
    #[prost(int32, tag = "5")]
    reason_code: i32,
    #[prost(int32, tag = "6")]
    control_code: i32,
    #[prost(int32, tag = "8")]
    session_code: i32,
}

pub(super) fn pair_ack(body: &[u8], expected: u32) -> Result<Vec<u8>, OnlinePacketError> {
    let input = PairEnvelope::decode(body).map_err(|_| OnlinePacketError)?;
    if input.first != expected {
        return Err(OnlinePacketError);
    }
    Ok(input.encode_to_vec())
}

pub(super) fn protective_notice(body: &[u8]) -> Result<ProtectiveNotice, OnlinePacketError> {
    let input = ProtectiveEnvelope::decode(body).map_err(|_| OnlinePacketError)?;
    if input.title.len() > MAX_NOTICE_TEXT || input.detail.len() > MAX_NOTICE_TEXT {
        return Err(OnlinePacketError);
    }
    Ok(ProtectiveNotice {
        title: input.title,
        detail: input.detail,
        account: input.account,
        reason_code: input.reason_code,
        control_code: input.control_code,
        session_code: input.session_code,
    })
}

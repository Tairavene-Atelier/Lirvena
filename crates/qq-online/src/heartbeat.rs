use prost::Message;

use crate::proto::{HeartbeatRequest, HeartbeatResponse, HeartbeatSilence};
use crate::register::encode;
use crate::{HeartbeatInput, OnlinePacketError};

const MAX_PACKET_LEN: usize = 1024 * 1024;

/// Parsed modern heartbeat response.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HeartbeatOutcome {
    /// Server-selected next interval in milliseconds, or none when omitted.
    pub requested_interval_ms: Option<u64>,
}

/// Encodes one modern online heartbeat.
///
/// # Errors
///
/// Returns an error for invalid signed-Profile tuning, time or battery values.
pub fn encode_heartbeat(input: HeartbeatInput) -> Result<Vec<u8>, OnlinePacketError> {
    let tuning = input.tuning.spec();
    if input.unix_seconds == 0 || input.battery_state > 100 {
        return Err(OnlinePacketError);
    }
    encode(&HeartbeatRequest {
        heartbeat_type: tuning.heartbeat_type,
        silence: Some(HeartbeatSilence {
            local_silence: input.state.local_silence,
            silence_version: input.state.silence_version,
        }),
        battery_state: input.battery_state,
        unix_seconds: input.unix_seconds,
    })
}

/// Parses an empty or bounded modern heartbeat response.
///
/// # Errors
///
/// Returns an error for malformed protobuf, a negative interval or arithmetic overflow.
pub fn parse_heartbeat_response(bytes: &[u8]) -> Result<HeartbeatOutcome, OnlinePacketError> {
    if bytes.len() > MAX_PACKET_LEN {
        return Err(OnlinePacketError);
    }
    if bytes.is_empty() {
        return Ok(HeartbeatOutcome {
            requested_interval_ms: None,
        });
    }
    let response = HeartbeatResponse::decode(bytes).map_err(|_error| OnlinePacketError)?;
    let requested_interval_ms = match response.next_interval_seconds {
        0 => None,
        seconds if seconds > 0 => Some(
            u64::try_from(seconds)
                .map_err(|_error| OnlinePacketError)?
                .checked_mul(1_000)
                .ok_or(OnlinePacketError)?,
        ),
        _ => return Err(OnlinePacketError),
    };
    Ok(HeartbeatOutcome {
        requested_interval_ms,
    })
}

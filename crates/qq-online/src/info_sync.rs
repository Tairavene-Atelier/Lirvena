use std::collections::HashMap;

use prost::Message;

use crate::proto::{
    ApplicationState, AuxiliaryState, DirectCookie, DirectSync, InfoSyncRequest, InfoSyncResponse,
    NormalConfig,
};
use crate::register::{encode, parse_silence, register_info, valid_diagnostic};
use crate::{InfoSyncInput, OnlinePacketError};

const MAX_PACKET_LEN: usize = 1024 * 1024;

/// Parsed synchronization result required by the online state machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InfoSyncOutcome {
    /// Whether the top-level and required embedded registration succeeded.
    pub success: bool,
    /// QQ business result code.
    pub result: i32,
    /// Response correlation value.
    pub response_random: u32,
    /// Requested delayed continuation in milliseconds.
    pub delayed_after_ms: Option<u64>,
    /// Updated local silence value, when supplied.
    pub local_silence: Option<u32>,
    /// Updated silence version, when supplied.
    pub silence_version: Option<u32>,
}

/// Encodes an initial or QQ-requested delayed synchronization packet.
///
/// # Errors
///
/// Returns an error for zero correlation, invalid signed-Profile tuning or oversized data.
pub fn encode_info_sync(input: InfoSyncInput<'_>) -> Result<Vec<u8>, OnlinePacketError> {
    if input.request_random == 0 {
        return Err(OnlinePacketError);
    }
    let tuning = input.tuning;
    let tuning_spec = tuning.spec();
    let state = input.state;
    encode(&InfoSyncRequest {
        sync_flag: tuning_spec.sync_flag,
        request_random: Some(input.request_random),
        active_status: Some(state.active_status),
        group_last_message_time: Some(state.group_last_message_time),
        direct_sync: Some(DirectSync {
            current_cookie: Some(DirectCookie {
                last_message_time: state.direct_last_message_time,
            }),
            last_message_time: state.direct_last_message_time,
            previous_cookie: Some(DirectCookie {
                last_message_time: state.previous_direct_message_time,
            }),
        }),
        normal_config: Some(NormalConfig {
            integer_values: HashMap::new(),
        }),
        register: Some(register_info(input.device, state, tuning, true)?),
        auxiliary: Some(AuxiliaryState {
            group_code: Some(state.last_group_code),
            flag: tuning_spec.auxiliary_flag,
        }),
        application: Some(ApplicationState {
            delayed: Some(u32::from(input.delayed)),
            application_status: Some(state.application_status),
            silence_status: Some(state.local_silence),
        }),
    })
}

/// Parses the bounded synchronization fields required for online transitions.
///
/// # Errors
///
/// Returns an error for malformed protobuf, oversized diagnostics, negative silence or overflow.
pub fn parse_info_sync_response(bytes: &[u8]) -> Result<InfoSyncOutcome, OnlinePacketError> {
    if bytes.len() > MAX_PACKET_LEN {
        return Err(OnlinePacketError);
    }
    let response = InfoSyncResponse::decode(bytes).map_err(|_error| OnlinePacketError)?;
    if !valid_diagnostic(&response.message) {
        return Err(OnlinePacketError);
    }
    let required_online = response
        .register
        .as_ref()
        .is_some_and(|register| register.result == 0);
    let success = response.result == 0 && required_online;
    let delayed_after_ms = response
        .delayed_sync
        .filter(|delayed| delayed.enabled && delayed.delay_seconds > 0)
        .map(|delayed| u64::from(delayed.delay_seconds) * 1_000);
    let silence = response
        .register
        .and_then(|register| register.silence)
        .map(parse_silence)
        .transpose()?;
    Ok(InfoSyncOutcome {
        success,
        result: response.result,
        response_random: response.response_random,
        delayed_after_ms,
        local_silence: silence.map(|value| value.0),
        silence_version: silence.map(|value| value.1),
    })
}

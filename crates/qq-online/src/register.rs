use prost::Message;
use qq_profile::OnlinePacketTuning;

use crate::proto::{BusinessInfo, DeviceInfo, RegisterInfo, RegisterResponse, SilentControl};
use crate::{OnlineDevice, OnlinePacketError, OnlineSyncState, RegisterInput};

const MAX_PACKET_LEN: usize = 1024 * 1024;
const MAX_MESSAGE_LEN: usize = 4 * 1024;

/// Parsed result of an online registration response.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisterOutcome {
    /// QQ business result code.
    pub result: i32,
    /// Updated local silence value, when supplied.
    pub local_silence: Option<u32>,
    /// Updated silence version, when supplied.
    pub silence_version: Option<u32>,
}

/// Encodes the optional standalone online registration.
///
/// # Errors
///
/// Returns an error for invalid signed-Profile tuning or integer overflow.
pub fn encode_register(input: RegisterInput<'_>) -> Result<Vec<u8>, OnlinePacketError> {
    encode(&register_info(
        input.device,
        input.state,
        input.tuning,
        false,
    )?)
}

/// Parses a bounded standalone registration response.
///
/// # Errors
///
/// Returns an error for malformed protobuf, oversized diagnostics or negative silence values.
pub fn parse_register_response(bytes: &[u8]) -> Result<RegisterOutcome, OnlinePacketError> {
    if bytes.len() > MAX_PACKET_LEN {
        return Err(OnlinePacketError);
    }
    let response = RegisterResponse::decode(bytes).map_err(|_error| OnlinePacketError)?;
    if response
        .message
        .as_deref()
        .is_some_and(|message| !valid_diagnostic(message))
    {
        return Err(OnlinePacketError);
    }
    let silence = response.silence.map(parse_silence).transpose()?;
    Ok(RegisterOutcome {
        result: response.result,
        local_silence: silence.map(|value| value.0),
        silence_version: silence.map(|value| value.1),
    })
}

pub(super) fn register_info(
    device: &OnlineDevice,
    state: OnlineSyncState,
    tuning: OnlinePacketTuning,
    initial: bool,
) -> Result<RegisterInfo, OnlinePacketError> {
    let battery_state =
        i32::try_from(device.battery_state()).map_err(|_error| OnlinePacketError)?;
    let local_silence = i32::try_from(state.local_silence).map_err(|_error| OnlinePacketError)?;
    let silence_version =
        i32::try_from(state.silence_version).map_err(|_error| OnlinePacketError)?;
    let tuning = tuning.spec();
    let (vendor_type, register_type, first_register) = if initial {
        (
            tuning.initial_vendor_type,
            tuning.initial_register_type,
            i32::from(state.first_register),
        )
    } else {
        (tuning.status_vendor_type, tuning.status_register_type, 0)
    };
    Ok(RegisterInfo {
        guid: Some(device.guid_hex().to_owned()),
        kick_pc: Some(0),
        current_version: Some(device.client_version().to_owned()),
        first_register: Some(first_register),
        locale_id: Some(tuning.locale_id),
        device: Some(DeviceInfo {
            user: device.name().to_owned(),
            os: device.operating_system().to_owned(),
            os_version: device.operating_system_version().to_owned(),
            vendor_name: Some(String::new()),
            os_lower: device.vendor_operating_system().to_owned(),
        }),
        set_mute: Some(0),
        vendor_type: Some(vendor_type),
        register_type: Some(register_type),
        business: Some(BusinessInfo {
            notify_switch: 1,
            bind_uin_notify_switch: 1,
        }),
        battery_state: Some(battery_state),
        field_12: Some(1),
        silence: Some(SilentControl {
            local_silence,
            silence_version,
        }),
        scene: Some(state.scene),
        background_seconds: Some(state.background_seconds),
        chat_on_focus: Some(state.chat_on_focus),
    })
}

pub(super) fn parse_silence(value: SilentControl) -> Result<(u32, u32), OnlinePacketError> {
    Ok((
        u32::try_from(value.local_silence).map_err(|_error| OnlinePacketError)?,
        u32::try_from(value.silence_version).map_err(|_error| OnlinePacketError)?,
    ))
}

pub(super) fn encode(message: &impl Message) -> Result<Vec<u8>, OnlinePacketError> {
    if message.encoded_len() > MAX_PACKET_LEN {
        return Err(OnlinePacketError);
    }
    Ok(message.encode_to_vec())
}

pub(super) fn valid_diagnostic(value: &str) -> bool {
    value.len() <= MAX_MESSAGE_LEN && !value.chars().any(char::is_control)
}

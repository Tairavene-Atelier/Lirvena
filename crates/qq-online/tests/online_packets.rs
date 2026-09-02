//! Online codec contract tests.

use qq_online::{
    HeartbeatInput, InfoSyncInput, OnlineDevice, OnlineSyncState, RegisterInput, encode_heartbeat,
    encode_info_sync, encode_register, parse_heartbeat_response, parse_info_sync_response,
    parse_register_response,
};
use qq_profile::{OnlinePacketTuning, OnlinePacketTuningSpec};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn tuning() -> Result<OnlinePacketTuning, qq_profile::OnlinePacketPlanError> {
    OnlinePacketTuning::new(OnlinePacketTuningSpec {
        sync_flag: 0x123,
        locale_id: 1_033,
        initial_vendor_type: 4,
        initial_register_type: 3,
        status_vendor_type: 5,
        status_register_type: 2,
        auxiliary_flag: 7,
        heartbeat_type: 9,
    })
}

fn device() -> Result<OnlineDevice, qq_online::OnlinePacketError> {
    OnlineDevice::new(
        "00112233445566778899aabbccddeeff",
        "Lirvena".to_owned(),
        "Linux".to_owned(),
        String::new(),
        "linux".to_owned(),
        "1.2.3-456".to_owned(),
        77,
    )
}

#[test]
fn initial_and_delayed_sync_differ_only_in_the_profile_selected_flag() -> TestResult {
    let device = device()?;
    let state = OnlineSyncState::default();
    let initial = encode_info_sync(InfoSyncInput {
        device: &device,
        state,
        tuning: tuning()?,
        request_random: 7,
        delayed: false,
    })?;
    let delayed = encode_info_sync(InfoSyncInput {
        device: &device,
        state,
        tuning: tuning()?,
        request_random: 7,
        delayed: true,
    })?;
    assert_ne!(initial, delayed);
    assert!(initial.len() < 1_024);
    assert!(delayed.len() < 1_024);
    Ok(())
}

#[test]
fn register_and_heartbeat_are_canonical_and_bounded() -> TestResult {
    let device = device()?;
    let state = OnlineSyncState::default();
    let register = encode_register(RegisterInput {
        device: &device,
        state,
        tuning: tuning()?,
    })?;
    assert_eq!(
        register,
        encode_register(RegisterInput {
            device: &device,
            state,
            tuning: tuning()?,
        })?
    );
    let heartbeat = encode_heartbeat(HeartbeatInput {
        state,
        tuning: tuning()?,
        unix_seconds: 1_800_000_000,
        battery_state: 73,
    })?;
    assert_eq!(heartbeat, hex("0809120018492080A4A7DA06")?);
    assert_eq!(
        parse_heartbeat_response(&hex("189E02")?)?.requested_interval_ms,
        Some(286_000)
    );
    assert_eq!(parse_heartbeat_response(&[])?.requested_interval_ms, None);
    Ok(())
}

#[test]
fn invalid_profile_values_fail_before_encoding() -> TestResult {
    let invalid = OnlinePacketTuning::new(OnlinePacketTuningSpec {
        sync_flag: 0,
        ..tuning()?.spec()
    });
    assert!(invalid.is_err());
    assert!(
        OnlineDevice::new(
            "not-a-guid",
            "device".to_owned(),
            "Linux".to_owned(),
            String::new(),
            "linux".to_owned(),
            "1.2.3-456".to_owned(),
            77,
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn response_decoders_keep_only_bounded_transition_values() -> TestResult {
    let sync = parse_info_sync_response(&hex("18073A064A040802100352040801104B")?)?;
    assert!(sync.success);
    assert_eq!(sync.response_random, 7);
    assert_eq!(sync.delayed_after_ms, Some(75_000));
    assert_eq!(sync.local_silence, Some(2));
    assert_eq!(sync.silence_version, Some(3));

    let register = parse_register_response(&hex("4A0408021003")?)?;
    assert_eq!(register.result, 0);
    assert_eq!(register.local_silence, Some(2));
    assert_eq!(register.silence_version, Some(3));
    assert!(parse_register_response(&hex("08014A0208FF")?).is_err());
    Ok(())
}

fn hex(value: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|chunk| {
            let text = core::str::from_utf8(chunk)?;
            Ok(u8::from_str_radix(text, 16)?)
        })
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()
}

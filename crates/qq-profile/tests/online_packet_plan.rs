//! Canonical signed-Profile online packet plan tests.

use ceylith_protocol::{OpaqueSlot, OpaqueSlotId, OpaqueSlots, ProfileId};
use qq_profile::{
    LinuxNtProfile, LinuxNtProfileSpec, ONLINE_PACKET_PLAN_SLOT_ID, OnlinePacketPlan,
    OnlinePacketPlanSpec, OnlinePacketTuning, OnlinePacketTuningSpec, decode_online_packet_plan,
    encode_online_packet_plan,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn plan() -> TestResult<OnlinePacketPlan> {
    Ok(OnlinePacketPlan::new(OnlinePacketPlanSpec {
        initial_sync_route: "route.initial".to_owned(),
        delayed_sync_route: "route.delayed".to_owned(),
        status_register_route: Some("route.status".to_owned()),
        heartbeat_route: "route.heartbeat".to_owned(),
        tuning: OnlinePacketTuning::new(OnlinePacketTuningSpec {
            sync_flag: 0x6df,
            locale_id: 2_052,
            initial_vendor_type: 6,
            initial_register_type: 0,
            status_vendor_type: 0,
            status_register_type: 1,
            auxiliary_flag: 2,
            heartbeat_type: 1,
        })?,
    })?)
}

fn profile(slot: Vec<u8>) -> TestResult<LinuxNtProfile> {
    let slots = OpaqueSlots::new(vec![OpaqueSlot::new(
        OpaqueSlotId::new(ONLINE_PACKET_PLAN_SLOT_ID)?,
        slot,
    )?])?;
    Ok(LinuxNtProfile::new(profile_spec(), slots)?)
}

fn profile_spec() -> LinuxNtProfileSpec {
    LinuxNtProfileSpec {
        profile_id: ProfileId::from_bytes([1; 16]),
        client_version: "1.2.3-456".to_owned(),
        app_id: 1,
        sub_app_id: 2,
        qr_app_id: 2,
        app_client_version: 456,
        package_name: "example.client".to_owned(),
        operating_system: "Linux".to_owned(),
        pt_version: "1.2.3".to_owned(),
        sso_version: 19,
        misc_bitmap: 1,
        login_sdk: "example.login.1".to_owned(),
        main_sig_map: 1,
        sub_sig_map: 1,
        login_misc_bitmap: 1,
        runtime_abi: 3,
    }
}

#[test]
fn packet_plan_round_trips_through_its_opaque_profile_slot() -> TestResult {
    let expected = plan()?;
    let encoded = encode_online_packet_plan(&expected)?;
    assert_eq!(decode_online_packet_plan(&profile(encoded)?)?, expected);
    Ok(())
}

#[test]
fn malformed_routes_and_trailing_data_fail_closed() -> TestResult {
    let tuning = plan()?.tuning();
    assert!(
        OnlinePacketPlan::new(OnlinePacketPlanSpec {
            initial_sync_route: "bad route".to_owned(),
            delayed_sync_route: "route.delayed".to_owned(),
            status_register_route: None,
            heartbeat_route: "route.heartbeat".to_owned(),
            tuning,
        })
        .is_err()
    );
    let mut encoded = encode_online_packet_plan(&plan()?)?;
    encoded.push(0);
    assert!(decode_online_packet_plan(&profile(encoded)?).is_err());
    Ok(())
}

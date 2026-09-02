//! Canonical signed-Profile online plan tests.

use ceylith_protocol::{OpaqueSlot, OpaqueSlotId, OpaqueSlots, ProfileId};
use qq_domain::{OnlinePlan, OnlinePlanSpec, PlanActionId};
use qq_profile::{
    LinuxNtProfile, LinuxNtProfileSpec, ONLINE_PLAN_SLOT_ID, decode_online_plan, encode_online_plan,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn id(value: u8) -> TestResult<PlanActionId> {
    Ok(PlanActionId::new([value; 16])?)
}

fn plan() -> TestResult<OnlinePlan> {
    Ok(OnlinePlan::new(OnlinePlanSpec {
        initial_sync: id(1)?,
        delayed_sync: id(2)?,
        security_bootstrap: id(3)?,
        status_confirmation: Some(id(4)?),
        business_heartbeat: id(5)?,
        initial_heartbeat_ms: 270_000,
        minimum_heartbeat_ms: 120_000,
        maximum_heartbeat_ms: 500_000,
        minimum_delayed_sync_ms: 60_000,
        maximum_delayed_sync_ms: 3_600_000,
    })?)
}

fn profile(slot: Vec<u8>) -> TestResult<LinuxNtProfile> {
    let slots = OpaqueSlots::new(vec![OpaqueSlot::new(
        OpaqueSlotId::new(ONLINE_PLAN_SLOT_ID)?,
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
        runtime_abi: 2,
    }
}

#[test]
fn online_plan_round_trips_through_opaque_profile_slot() -> TestResult {
    let expected = plan()?;
    let encoded = encode_online_plan(expected)?;
    assert_eq!(decode_online_plan(&profile(encoded)?)?, expected);
    Ok(())
}

#[test]
fn malformed_or_missing_plan_fails_closed() -> TestResult {
    let mut encoded = encode_online_plan(plan()?)?;
    encoded.push(0);
    assert!(decode_online_plan(&profile(encoded)?).is_err());
    let empty = LinuxNtProfile::new(profile_spec(), OpaqueSlots::new(Vec::new())?)?;
    assert!(decode_online_plan(&empty).is_err());
    Ok(())
}

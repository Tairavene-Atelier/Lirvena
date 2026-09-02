//! Canonical signed-Profile Push plan tests.

use ceylith_protocol::{OpaqueSlot, OpaqueSlotId, OpaqueSlots, ProfileId};
use qq_profile::{
    LinuxNtProfile, LinuxNtProfileSpec, PUSH_PLAN_SLOT_ID, PushBehavior, PushPlan, PushPlanEntry,
    PushPlanSpec, decode_push_plan, encode_push_plan,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn plan() -> TestResult<PushPlan> {
    Ok(PushPlan::new(PushPlanSpec {
        entries: vec![
            PushPlanEntry::new(
                "push.alpha".to_owned(),
                PushBehavior::EchoBody,
                Some("push.alpha.ack".to_owned()),
                0,
                1_024,
            )?,
            PushPlanEntry::new(
                "push.stop".to_owned(),
                PushBehavior::ProtectiveOffline,
                None,
                0,
                4_096,
            )?,
            PushPlanEntry::new(
                "push.video".to_owned(),
                PushBehavior::LegacyVideoAck,
                Some("push.video.ack".to_owned()),
                0,
                1024 * 1024,
            )?,
            PushPlanEntry::new(
                "push.sync".to_owned(),
                PushBehavior::InfoSyncState,
                None,
                0,
                2 * 1024 * 1024,
            )?,
        ],
    })?)
}

#[test]
fn plan_round_trips_through_opaque_slot() -> TestResult {
    let expected = plan()?;
    let encoded = encode_push_plan(&expected)?;
    let slots = OpaqueSlots::new(vec![OpaqueSlot::new(
        OpaqueSlotId::new(PUSH_PLAN_SLOT_ID)?,
        encoded,
    )?])?;
    let profile = LinuxNtProfile::new(profile_spec(), slots)?;
    assert_eq!(decode_push_plan(&profile)?, expected);
    Ok(())
}

#[test]
fn duplicate_routes_and_inconsistent_ack_shape_fail_closed() -> TestResult {
    let entry = PushPlanEntry::new("push.alpha".to_owned(), PushBehavior::Observe, None, 0, 1)?;
    assert!(
        PushPlan::new(PushPlanSpec {
            entries: vec![entry.clone(), entry]
        })
        .is_err()
    );
    assert!(PushPlanEntry::new("push.bad".to_owned(), PushBehavior::EchoBody, None, 0, 1).is_err());
    assert!(
        PushPlanEntry::new(
            "push.video".to_owned(),
            PushBehavior::LegacyVideoAck,
            None,
            0,
            1,
        )
        .is_err()
    );
    Ok(())
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

//! Signed-plan online-generation state machine tests.

use qq_domain::{
    OnlineAction, OnlineDirective, OnlineGeneration, OnlineMachine, OnlinePlan, OnlinePlanSpec,
    OnlineState, PlanActionId,
};

const NOW: u64 = 10_000;
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn id(value: u8) -> TestResult<PlanActionId> {
    Ok(PlanActionId::new([value; 16])?)
}

fn plan(with_confirmation: bool) -> TestResult<OnlinePlan> {
    Ok(OnlinePlan::new(OnlinePlanSpec {
        initial_sync: id(1)?,
        delayed_sync: id(2)?,
        security_bootstrap: id(3)?,
        status_confirmation: if with_confirmation {
            Some(id(4)?)
        } else {
            None
        },
        business_heartbeat: id(5)?,
        initial_heartbeat_ms: 270_000,
        minimum_heartbeat_ms: 120_000,
        maximum_heartbeat_ms: 500_000,
        minimum_delayed_sync_ms: 60_000,
        maximum_delayed_sync_ms: 3_600_000,
    })?)
}

#[test]
fn required_startup_gates_precede_online_schedules() -> TestResult {
    let generation = OnlineGeneration::new(7)?;
    let mut machine = OnlineMachine::new(plan(true)?);
    assert_eq!(
        machine.start(generation)?,
        OnlineDirective::Dispatch {
            generation,
            action: OnlineAction::InitialSync(id(1)?),
        }
    );
    assert_eq!(machine.poll_due(u64::MAX), [None, None]);
    assert_eq!(
        machine.initial_sync_succeeded(generation, NOW, Some(30_000))?,
        OnlineDirective::Dispatch {
            generation,
            action: OnlineAction::SecurityBootstrap(id(3)?),
        }
    );
    assert_eq!(
        machine.security_bootstrap_succeeded(generation, NOW)?,
        OnlineDirective::Dispatch {
            generation,
            action: OnlineAction::StatusConfirmation(id(4)?),
        }
    );
    assert_eq!(
        machine.status_confirmation_completed(generation, NOW)?,
        OnlineDirective::EnteredOnline(generation)
    );
    assert_eq!(machine.state(), OnlineState::Online(generation));
    assert_eq!(machine.next_due_ms(), Some(NOW + 60_000));

    let due = machine.poll_due(NOW + 60_000);
    assert_eq!(due[0], None);
    assert_eq!(
        due[1],
        Some(OnlineDirective::Dispatch {
            generation,
            action: OnlineAction::DelayedSync(id(2)?),
        })
    );
    assert_eq!(machine.next_due_ms(), Some(NOW + 270_000));
    assert_eq!(machine.poll_due(u64::MAX)[1], None);
    Ok(())
}

#[test]
fn heartbeat_interval_is_server_directed_bounded_and_non_overlapping() -> TestResult {
    let generation = OnlineGeneration::new(8)?;
    let mut machine = OnlineMachine::new(plan(false)?);
    machine.start(generation)?;
    machine.initial_sync_succeeded(generation, NOW, None)?;
    machine.security_bootstrap_succeeded(generation, NOW)?;

    assert_eq!(machine.poll_due(NOW + 269_999), [None, None]);
    assert!(matches!(
        machine.poll_due(NOW + 270_000)[0],
        Some(OnlineDirective::Dispatch {
            action: OnlineAction::BusinessHeartbeat(_),
            ..
        })
    ));
    assert_eq!(machine.poll_due(u64::MAX)[0], None);
    machine.heartbeat_completed(generation, NOW + 270_000, Some(20_000))?;
    assert_eq!(machine.poll_due(NOW + 389_999)[0], None);
    assert!(machine.poll_due(NOW + 390_000)[0].is_some());
    machine.heartbeat_completed(generation, NOW + 390_000, None)?;
    assert_eq!(machine.poll_due(NOW + 509_999)[0], None);
    assert!(machine.poll_due(NOW + 510_000)[0].is_some());
    Ok(())
}

#[test]
fn stale_generation_and_required_failure_fail_closed() -> TestResult {
    let first = OnlineGeneration::new(9)?;
    let stale = OnlineGeneration::new(8)?;
    let mut machine = OnlineMachine::new(plan(false)?);
    machine.start(first)?;
    assert!(machine.initial_sync_succeeded(stale, NOW, None).is_err());
    assert_eq!(
        machine.required_action_failed(first)?,
        OnlineDirective::ProtectiveOffline(first)
    );
    assert_eq!(machine.state(), OnlineState::ProtectiveOffline(first));
    assert_eq!(machine.poll_due(u64::MAX), [None, None]);
    assert_eq!(machine.next_due_ms(), None);
    Ok(())
}

#[test]
fn malformed_plan_is_rejected_before_execution() -> TestResult {
    assert!(PlanActionId::new([0; 16]).is_err());
    assert!(
        OnlinePlan::new(OnlinePlanSpec {
            initial_sync: id(1)?,
            delayed_sync: id(1)?,
            security_bootstrap: id(3)?,
            status_confirmation: None,
            business_heartbeat: id(5)?,
            initial_heartbeat_ms: 270_000,
            minimum_heartbeat_ms: 120_000,
            maximum_heartbeat_ms: 500_000,
            minimum_delayed_sync_ms: 60_000,
            maximum_delayed_sync_ms: 3_600_000,
        })
        .is_err()
    );
    assert!(
        OnlinePlan::new(OnlinePlanSpec {
            initial_sync: id(1)?,
            delayed_sync: id(2)?,
            security_bootstrap: id(3)?,
            status_confirmation: None,
            business_heartbeat: id(5)?,
            initial_heartbeat_ms: 100,
            minimum_heartbeat_ms: 100,
            maximum_heartbeat_ms: 500_000,
            minimum_delayed_sync_ms: 60_000,
            maximum_delayed_sync_ms: 3_600_000,
        })
        .is_err()
    );
    Ok(())
}

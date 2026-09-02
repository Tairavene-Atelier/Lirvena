use std::io;

#[cfg(target_os = "linux")]
use account_runtime::{AccountLocalId, ProtectiveReason};
#[cfg(target_os = "linux")]
use ceylith_protocol::{WatchEvent, WatchEventKind};
use notify_runtime::{
    DedupeKey, EventCategory, EventId, EventSource, EventState, NotificationEvent,
    NotificationText, Severity, StateTransition,
};
use sha2::{Digest, Sha256};

pub(super) fn test_event(occurred_at_ms: u64) -> Result<NotificationEvent, io::Error> {
    build(EventTemplate {
        occurred_at_ms,
        source: EventSource::Lirvena,
        category: EventCategory::Worker,
        severity: Severity::Info,
        account: None,
        reason_code: 1,
        previous: EventState::Recovering,
        current: EventState::Active,
        summary: "Lirvena notification test succeeded",
        next_action: "No action is required",
    })
}

#[cfg(target_os = "linux")]
pub(super) fn from_watch(
    event: &WatchEvent,
    account: AccountLocalId,
) -> Result<NotificationEvent, io::Error> {
    let (category, severity, previous, current, summary, next_action) = match event.kind() {
        WatchEventKind::GrantExpiring => (
            EventCategory::Authorization,
            Severity::Warning,
            EventState::Current,
            EventState::Expiring,
            "Ceylith authorization is approaching expiry",
            "Review Token renewal status",
        ),
        WatchEventKind::RenewalPaused => (
            EventCategory::Authorization,
            Severity::Critical,
            EventState::Current,
            EventState::Paused,
            "Ceylith automatic renewal is paused",
            "Review Token status before authorization expires",
        ),
        WatchEventKind::GrantRevoked => (
            EventCategory::Authorization,
            Severity::Critical,
            EventState::Current,
            EventState::Revoked,
            "Ceylith authorization was revoked",
            "Review account mode before restarting Lirvena",
        ),
        WatchEventKind::QuotaChanged => (
            EventCategory::Authorization,
            Severity::Warning,
            EventState::Current,
            EventState::Unavailable,
            "Ceylith authorization quota changed",
            "Review Full account assignments",
        ),
        WatchEventKind::PolicyChanged => (
            EventCategory::Authorization,
            Severity::Info,
            EventState::Active,
            EventState::Recovering,
            "Ceylith authorization policy changed",
            "Lirvena will apply the signed policy state",
        ),
        WatchEventKind::ProfileChanged => (
            EventCategory::Continuity,
            Severity::Critical,
            EventState::Current,
            EventState::Unavailable,
            "Ceylith Profile continuity changed",
            "Restart after a compatible Profile is available",
        ),
        WatchEventKind::Maintenance => (
            EventCategory::Continuity,
            Severity::Critical,
            EventState::Active,
            EventState::Stopped,
            "Ceylith requested protective maintenance shutdown",
            "Wait for maintenance completion before restarting",
        ),
        WatchEventKind::GrantRestored => (
            EventCategory::Recovery,
            Severity::Info,
            EventState::Paused,
            EventState::Active,
            "Ceylith authorization was restored",
            "Restart protected accounts when ready",
        ),
    };
    build(EventTemplate {
        occurred_at_ms: event.occurred_at_ms(),
        source: EventSource::Ceylith,
        category,
        severity,
        account: Some(*account.as_bytes()),
        reason_code: event.reason_code(),
        previous,
        current,
        summary,
        next_action,
    })
}

#[cfg(target_os = "linux")]
pub(super) fn protective_offline(
    occurred_at_ms: u64,
    account: AccountLocalId,
    reason: ProtectiveReason,
) -> Result<NotificationEvent, io::Error> {
    let (source, category, reason_code, summary, next_action) = match reason {
        ProtectiveReason::GrantUnavailable => (
            EventSource::Ceylith,
            EventCategory::Authorization,
            100,
            "Lirvena closed the QQ transport after authorization loss",
            "Review Token and account mode before restarting",
        ),
        ProtectiveReason::ProfileUnavailable => (
            EventSource::Ceylith,
            EventCategory::Continuity,
            101,
            "Lirvena closed the QQ transport after Profile loss",
            "Wait for a compatible signed Profile before restarting",
        ),
        ProtectiveReason::CeylithContinuity => (
            EventSource::Lirvena,
            EventCategory::Continuity,
            102,
            "Lirvena lost Ceylith continuity and closed the QQ transport",
            "Check Ceylith connectivity before restarting",
        ),
        ProtectiveReason::WorkerFailure => (
            EventSource::Account,
            EventCategory::Worker,
            103,
            "Lirvena account worker stopped unexpectedly",
            "Review local logs before restarting the account",
        ),
        ProtectiveReason::ProcessRestart => (
            EventSource::Account,
            EventCategory::Worker,
            104,
            "Lirvena recovered an interrupted account generation",
            "Review the prior shutdown before restarting the account",
        ),
        ProtectiveReason::RemoteSessionEnded => (
            EventSource::Qq,
            EventCategory::RiskControl,
            105,
            "QQ ended the active account session",
            "Review QQ risk-control state before restarting",
        ),
    };
    build(EventTemplate {
        occurred_at_ms,
        source,
        category,
        severity: Severity::Critical,
        account: Some(*account.as_bytes()),
        reason_code,
        previous: EventState::Active,
        current: EventState::ProtectiveOffline,
        summary,
        next_action,
    })
}

#[derive(Clone, Copy)]
struct EventTemplate {
    occurred_at_ms: u64,
    source: EventSource,
    category: EventCategory,
    severity: Severity,
    account: Option<[u8; 16]>,
    reason_code: u32,
    previous: EventState,
    current: EventState,
    summary: &'static str,
    next_action: &'static str,
}

fn build(template: EventTemplate) -> Result<NotificationEvent, io::Error> {
    let mut event_id = [0_u8; 16];
    getrandom::fill(&mut event_id).map_err(|_| io::Error::other("event identity unavailable"))?;
    let dedupe = dedupe_key(&template, template.account.as_ref());
    NotificationEvent::new(
        EventId::from_bytes(event_id),
        template.occurred_at_ms,
        template.source,
        template.category,
        template.severity,
        template.account,
        template.reason_code,
        StateTransition::new(template.previous, template.current)
            .map_err(|_| io::Error::other("compiled notification transition is invalid"))?,
        NotificationText::new(template.summary)
            .map_err(|_| io::Error::other("compiled notification summary is invalid"))?,
        NotificationText::new(template.next_action)
            .map_err(|_| io::Error::other("compiled notification action is invalid"))?,
        DedupeKey::from_bytes(dedupe),
    )
    .map_err(|_| io::Error::other("notification event is invalid"))
}

fn dedupe_key(template: &EventTemplate, account: Option<&[u8; 16]>) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update([template.source as u8]);
    digest.update([template.category as u8]);
    digest.update(template.reason_code.to_be_bytes());
    if let Some(account) = account {
        digest.update(account);
    }
    digest.finalize().into()
}

#[cfg(test)]
mod tests {
    use notify_runtime::{EventCategory, EventSource, EventState, Severity};

    use super::test_event;

    #[test]
    fn test_event_is_a_recovery_without_identifiers() -> Result<(), Box<dyn std::error::Error>> {
        let event = test_event(1)?;
        assert_eq!(event.source(), EventSource::Lirvena);
        assert_eq!(event.category(), EventCategory::Worker);
        assert_eq!(event.severity(), Severity::Info);
        assert_eq!(event.transition().current(), EventState::Active);
        assert!(event.account_local_id().is_none());
        Ok(())
    }
}

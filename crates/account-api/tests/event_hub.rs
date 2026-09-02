//! Account event hub contract tests.

use account_api::{AccountEvent, AccountEventHub};
use account_runtime::{AccountLocalId, AccountPhase};

#[tokio::test]
async fn adapters_share_one_bounded_event_stream() -> Result<(), Box<dyn std::error::Error>> {
    assert!(AccountEventHub::new(0).is_err());
    assert!(AccountEventHub::new(4_097).is_err());
    let hub = AccountEventHub::new(2)?;
    let publisher = hub.publisher();
    let mut subscriber = hub.subscribe();
    let local_id = AccountLocalId::from_bytes([7; 16]);
    assert!(publisher.publish(AccountEvent::Lifecycle {
        local_id,
        phase: AccountPhase::Starting,
        protective_reason: None,
        occurred_at_ms: 10,
    }));
    assert_eq!(
        subscriber.receive().await?,
        AccountEvent::Lifecycle {
            local_id,
            phase: AccountPhase::Starting,
            protective_reason: None,
            occurred_at_ms: 10,
        }
    );
    Ok(())
}

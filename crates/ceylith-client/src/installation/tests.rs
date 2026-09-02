use std::sync::{Arc, Mutex};

use ceylith_protocol::{
    RequestId, SessionAdmission, WatchEventKind, decode_session_welcome, proto,
};

use super::{ExchangeTransport, spawn_installation_watch, spawn_transport};
use crate::ClientError;

#[tokio::test]
async fn installation_client_serializes_unique_requests() -> Result<(), Box<dyn std::error::Error>>
{
    let request_ids = Arc::new(Mutex::new(Vec::new()));
    let transport = fake_transport(Arc::clone(&request_ids))?;
    let runtime = spawn_transport(transport, 2)?;
    let client = runtime.client();
    assert_eq!(client.remaining_capacity(), 2);

    let first = client.exchange(frame(10)).await?;
    let second = client.exchange(frame(20)).await?;
    assert_eq!(first.contract, 10);
    assert_eq!(second.contract, 20);
    assert_eq!(
        request_ids.lock().map_err(|_| ClientError::Worker)?.len(),
        2
    );
    let unique = {
        let ids = request_ids.lock().map_err(|_| ClientError::Worker)?;
        ids[0] != ids[1]
    };
    assert!(unique);
    runtime.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn installation_watch_preserves_idle_cursor() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = spawn_transport(fake_transport(Arc::new(Mutex::new(Vec::new())))?, 1)?;
    let client = runtime.client();
    assert_eq!(
        client.watch_once(12, 1_000).await?,
        ceylith_protocol::WatchOutcome::Idle { cursor: 12 }
    );
    runtime.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn dedicated_watch_delivers_typed_events_and_stops() -> Result<(), Box<dyn std::error::Error>>
{
    let runtime = spawn_transport(
        FakeTransport {
            admission: admission()?,
            request_ids: Arc::new(Mutex::new(Vec::new())),
            watch_event: Mutex::new(Some(watch_event())),
        },
        1,
    )?;
    let mut runtime = spawn_installation_watch(runtime, 1)?;
    let event = runtime.watch().next().await.ok_or(ClientError::Closed)??;
    assert_eq!(event.cursor(), 1);
    assert_eq!(event.kind(), WatchEventKind::RenewalPaused);
    runtime.shutdown().await?;
    Ok(())
}

#[test]
fn installation_client_rejects_unbounded_queues() -> Result<(), Box<dyn std::error::Error>> {
    let first = fake_transport(Arc::new(Mutex::new(Vec::new())))?;
    let second = fake_transport(Arc::new(Mutex::new(Vec::new())))?;
    assert_eq!(
        spawn_transport(first, 0).err(),
        Some(ClientError::Configuration)
    );
    assert_eq!(
        spawn_transport(second, 1_025).err(),
        Some(ClientError::Configuration)
    );
    Ok(())
}

#[test]
fn installation_client_requires_a_tokio_runtime() -> Result<(), Box<dyn std::error::Error>> {
    let transport = fake_transport(Arc::new(Mutex::new(Vec::new())))?;
    assert_eq!(
        spawn_transport(transport, 1).err(),
        Some(ClientError::Configuration)
    );
    Ok(())
}

struct FakeTransport {
    admission: SessionAdmission,
    request_ids: Arc<Mutex<Vec<RequestId>>>,
    watch_event: Mutex<Option<proto::WatchEvent>>,
}

impl ExchangeTransport for FakeTransport {
    fn admission(&self) -> &SessionAdmission {
        &self.admission
    }

    fn exchange(
        &mut self,
        request_id: RequestId,
        request: &proto::InnerFrame,
    ) -> impl Future<Output = Result<proto::InnerFrame, ClientError>> + Send {
        let result = self
            .request_ids
            .lock()
            .map_err(|_| ClientError::Worker)
            .map(|mut request_ids| {
                request_ids.push(request_id);
                match &request.body {
                    Some(proto::inner_frame::Body::WatchRequest(_)) => self.watch_response(request),
                    _ => request.clone(),
                }
            });
        std::future::ready(result)
    }
}

impl FakeTransport {
    fn watch_response(&self, request: &proto::InnerFrame) -> proto::InnerFrame {
        let event = self
            .watch_event
            .lock()
            .ok()
            .and_then(|mut event| event.take());
        let body = event.map_or_else(
            || {
                proto::inner_frame::Body::GenericResult(proto::GenericResult {
                    accepted: true,
                    code: 1,
                    payload: Vec::new(),
                })
            },
            proto::inner_frame::Body::WatchEvent,
        );
        proto::InnerFrame {
            contract: request.contract,
            body: Some(body),
        }
    }
}

fn fake_transport(
    request_ids: Arc<Mutex<Vec<RequestId>>>,
) -> Result<FakeTransport, Box<dyn std::error::Error>> {
    Ok(FakeTransport {
        admission: admission()?,
        request_ids,
        watch_event: Mutex::new(None),
    })
}

fn watch_event() -> proto::WatchEvent {
    proto::WatchEvent {
        cursor: 1,
        kind: proto::WatchEventKind::RenewalPaused as i32,
        occurred_at_ms: 1_001,
        account_slot_id: Vec::new(),
        reason_code: 2,
        payload: Vec::new(),
        grant: Some(proto::WatchGrantSnapshot {
            grant_class: proto::GrantClass::Community as i32,
            max_full_accounts: 3,
            max_active_installations: 2,
            expires_at_ms: 2_000,
            renewal_state: proto::RenewalState::Paused as i32,
            policy_epoch: 1,
        }),
    }
}

fn admission() -> Result<SessionAdmission, Box<dyn std::error::Error>> {
    Ok(decode_session_welcome(&proto::SessionWelcome {
        session_id: vec![1; 16],
        runtime_lease: vec![2; 32],
        lease_expires_at_ms: 2_000,
        grant_class: proto::GrantClass::Community as i32,
        max_full_accounts: 3,
        max_active_installations: 2,
        max_registered_installations: 4,
        server_time_ms: 1_000,
        policy_epoch: 1,
        accepted_contracts: vec![1],
    })?)
}

fn frame(contract: u32) -> proto::InnerFrame {
    proto::InnerFrame {
        contract,
        body: None,
    }
}

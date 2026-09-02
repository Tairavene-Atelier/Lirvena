use std::sync::Arc;

use ceylith_protocol::{
    ActionFlowContext, ActionObservation, Digest32, OpaqueSlots, ProfileId, RequestId,
    SessionAdmission, WatchOutcome, decode_watch_response, proto,
};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::{
    CeylithTcpClient, ClientError, CommunityTelemetrySigner, CommunityTelemetrySpec,
    OpaqueExchangeContext, RequestedAccess, RuntimeDescriptor,
    connection::{
        action_flow_begin_request_for, action_observation_request_for, opaque_exchange_request_for,
        profile_request_for, watch_request_for,
    },
    decode_telemetry_receipt,
};

mod watch;

pub use watch::{InstallationWatch, InstallationWatchRuntime, spawn_installation_watch};

const MAX_QUEUE_CAPACITY: usize = 1_024;

/// Cloneable, bounded handle to the installation-wide Ceylith connection owner.
#[derive(Clone)]
pub struct InstallationClient {
    sender: mpsc::Sender<Command>,
    admission: Arc<SessionAdmission>,
}

impl InstallationClient {
    /// Returns the immutable admission snapshot bound to this connection generation.
    #[must_use]
    pub fn admission(&self) -> &SessionAdmission {
        &self.admission
    }

    /// Returns currently unused queue slots.
    #[must_use]
    pub fn remaining_capacity(&self) -> usize {
        self.sender.capacity()
    }

    /// Builds a profile request bound to this connection generation.
    #[must_use]
    pub fn profile_request(
        &self,
        profile_id: ProfileId,
        cached_manifest_digest: Option<Digest32>,
        requested_access: RequestedAccess,
        runtime: &RuntimeDescriptor,
    ) -> proto::InnerFrame {
        profile_request_for(
            &self.admission,
            profile_id,
            cached_manifest_digest,
            requested_access,
            runtime,
        )
    }

    /// Builds a short-lived opaque request bound to this connection generation.
    ///
    /// # Errors
    ///
    /// Returns an error for empty slots, zero generation or an invalid deadline or binding.
    pub fn opaque_exchange_request(
        &self,
        context: OpaqueExchangeContext,
        slots: &OpaqueSlots,
        now_ms: u64,
    ) -> Result<proto::InnerFrame, ClientError> {
        opaque_exchange_request_for(&self.admission, context, slots, now_ms)
    }

    /// Builds a generation-bound action-flow request.
    ///
    /// # Errors
    ///
    /// Returns an error for empty inputs, zero generations or an expired deadline.
    pub fn action_flow_begin_request(
        &self,
        context: ActionFlowContext,
        inputs: &OpaqueSlots,
        now_ms: u64,
    ) -> Result<proto::InnerFrame, ClientError> {
        action_flow_begin_request_for(&self.admission, context, inputs, now_ms)
    }

    /// Builds an observation for one action returned by Ceylith.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid outcome/payload pairing or exceeded payload bound.
    pub fn action_observation_request(
        &self,
        observation: ActionObservation<'_>,
    ) -> Result<proto::InnerFrame, ClientError> {
        action_observation_request_for(&self.admission, observation)
    }

    /// Builds a bounded Watch long-poll request for this connection generation.
    ///
    /// # Errors
    ///
    /// Returns an error when the wait is zero or exceeds the public maximum.
    pub fn watch_request(
        &self,
        cursor: u64,
        max_wait_ms: u32,
    ) -> Result<proto::InnerFrame, ClientError> {
        watch_request_for(&self.admission, cursor, max_wait_ms)
    }

    /// Serializes one exchange through the installation-wide secure connection.
    ///
    /// Awaiting a full bounded queue applies backpressure to the calling account. A terminal
    /// carrier or secure-session failure closes this connection generation for every account.
    ///
    /// # Errors
    ///
    /// Returns the exchange failure, or `Closed` when the installation worker no longer accepts
    /// requests.
    pub async fn exchange(
        &self,
        request: proto::InnerFrame,
    ) -> Result<proto::InnerFrame, ClientError> {
        let (reply, response) = oneshot::channel();
        self.sender
            .send(Command::Exchange {
                request: Box::new(request),
                reply,
            })
            .await
            .map_err(|_| ClientError::Closed)?;
        response.await.map_err(|_| ClientError::Closed)?
    }

    /// Performs one cursor-bound Watch long poll.
    ///
    /// A dedicated installation connection should own repeated Watch calls so a long poll cannot
    /// delay ordinary account exchanges on another connection.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid wait, carrier failure, closed worker or malformed response.
    pub async fn watch_once(
        &self,
        cursor: u64,
        max_wait_ms: u32,
    ) -> Result<WatchOutcome, ClientError> {
        let request = self.watch_request(cursor, max_wait_ms)?;
        let response = self.exchange(request).await?;
        decode_watch_response(&response, cursor).map_err(ClientError::from)
    }

    /// Signs, submits, and verifies one completed Community daily report.
    ///
    /// # Errors
    ///
    /// Returns an error unless this is a Community admission, or when signing, transport, server
    /// acceptance, or response correlation fails.
    pub async fn submit_community_telemetry(
        &self,
        signer: &CommunityTelemetrySigner,
        spec: CommunityTelemetrySpec,
    ) -> Result<ceylith_protocol::TelemetryReportId, ClientError> {
        let report_id = spec.report_id;
        let request = signer.report(&self.admission, spec)?;
        let response = self.exchange(request).await?;
        decode_telemetry_receipt(&response, report_id)
    }
}

impl core::fmt::Debug for InstallationClient {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("InstallationClient")
            .field("admission", &self.admission)
            .field("remaining_capacity", &self.remaining_capacity())
            .finish_non_exhaustive()
    }
}

/// Owned lifetime of the installation-wide Ceylith connection task.
pub struct InstallationClientRuntime {
    client: InstallationClient,
    task: Option<JoinHandle<()>>,
}

impl InstallationClientRuntime {
    /// Returns a cloneable bounded client handle.
    #[must_use]
    pub fn client(&self) -> InstallationClient {
        self.client.clone()
    }

    /// Stops the connection owner after all earlier queued exchanges.
    ///
    /// # Errors
    ///
    /// Returns `Closed` if the owner already ended or `Worker` if its task failed.
    pub async fn shutdown(mut self) -> Result<(), ClientError> {
        let (reply, response) = oneshot::channel();
        self.client
            .sender
            .send(Command::Shutdown { reply })
            .await
            .map_err(|_| ClientError::Closed)?;
        response.await.map_err(|_| ClientError::Closed)?;
        let task = self.task.take().ok_or(ClientError::Worker)?;
        task.await.map_err(|_| ClientError::Worker)
    }

    fn abort(mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

impl Drop for InstallationClientRuntime {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

impl core::fmt::Debug for InstallationClientRuntime {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("InstallationClientRuntime")
            .field("client", &self.client)
            .finish_non_exhaustive()
    }
}

/// Starts one bounded installation-wide owner for an authenticated Ceylith connection.
///
/// # Errors
///
/// Returns an error for a queue capacity outside `1..=1024` or unavailable operating-system
/// randomness.
pub fn spawn_installation_client(
    connection: CeylithTcpClient,
    queue_capacity: usize,
) -> Result<InstallationClientRuntime, ClientError> {
    spawn_transport(connection, queue_capacity)
}

enum Command {
    Exchange {
        request: Box<proto::InnerFrame>,
        reply: oneshot::Sender<Result<proto::InnerFrame, ClientError>>,
    },
    Shutdown {
        reply: oneshot::Sender<()>,
    },
}

trait ExchangeTransport: Send + 'static {
    fn admission(&self) -> &SessionAdmission;

    fn exchange(
        &mut self,
        request_id: RequestId,
        request: &proto::InnerFrame,
    ) -> impl Future<Output = Result<proto::InnerFrame, ClientError>> + Send;
}

impl ExchangeTransport for CeylithTcpClient {
    fn admission(&self) -> &SessionAdmission {
        self.connection().admission()
    }

    fn exchange(
        &mut self,
        request_id: RequestId,
        request: &proto::InnerFrame,
    ) -> impl Future<Output = Result<proto::InnerFrame, ClientError>> + Send {
        Self::exchange(self, request_id, request)
    }
}

fn spawn_transport<T: ExchangeTransport>(
    transport: T,
    queue_capacity: usize,
) -> Result<InstallationClientRuntime, ClientError> {
    if !(1..=MAX_QUEUE_CAPACITY).contains(&queue_capacity) {
        return Err(ClientError::Configuration);
    }
    let request_ids = RequestIdSequence::new()?;
    let admission = Arc::new(transport.admission().clone());
    let (sender, receiver) = mpsc::channel(queue_capacity);
    let client = InstallationClient { sender, admission };
    let runtime = tokio::runtime::Handle::try_current().map_err(|_| ClientError::Configuration)?;
    let task = runtime.spawn(run(transport, receiver, request_ids));
    Ok(InstallationClientRuntime {
        client,
        task: Some(task),
    })
}

async fn run<T: ExchangeTransport>(
    mut transport: T,
    mut receiver: mpsc::Receiver<Command>,
    mut request_ids: RequestIdSequence,
) {
    while let Some(command) = receiver.recv().await {
        match command {
            Command::Exchange { request, reply } => {
                let result = match request_ids.next() {
                    Ok(request_id) => transport.exchange(request_id, &request).await,
                    Err(error) => Err(error),
                };
                let terminal = result.is_err();
                let _ignored = reply.send(result);
                if terminal {
                    break;
                }
            }
            Command::Shutdown { reply } => {
                let _ignored = reply.send(());
                break;
            }
        }
    }
}

struct RequestIdSequence {
    prefix: [u8; 8],
    next: u64,
}

impl RequestIdSequence {
    fn new() -> Result<Self, ClientError> {
        let mut prefix = [0_u8; 8];
        getrandom::fill(&mut prefix).map_err(|_| ClientError::Identity)?;
        Ok(Self { prefix, next: 1 })
    }

    fn next(&mut self) -> Result<RequestId, ClientError> {
        let sequence = self.next;
        self.next = self.next.checked_add(1).ok_or(ClientError::Closed)?;
        let mut bytes = [0_u8; RequestId::LENGTH];
        bytes[..8].copy_from_slice(&self.prefix);
        bytes[8..].copy_from_slice(&sequence.to_be_bytes());
        Ok(RequestId::from_bytes(bytes))
    }
}

#[cfg(test)]
mod tests;

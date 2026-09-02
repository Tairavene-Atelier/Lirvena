use ceylith_crypto::{SecureSession, TRANSPORT_TAG_LEN};
use ceylith_protocol::{
    ActionFlowContext, ActionObservation, CURRENT_INNER_CONTRACT, Digest32, OpaqueSlots, ProfileId,
    RequestId, SecureFrame, SessionAdmission, WireLimits, action_flow_binding_digest,
    action_observation_binding_digest, decode_inner_frame, decode_secure_frame, encode_inner_frame,
    encode_secure_frame, encode_secure_frame_header, opaque_binding_digest, proto,
};

use crate::{ClientError, OpaqueExchangeContext, RuntimeDescriptor};

/// Requested account capability for profile negotiation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum RequestedAccess {
    /// Public Basic capability.
    Public = 1,
    /// Token-authorized Full capability.
    Full = 2,
}

/// Authenticated, counter-bound Ceylith connection state.
pub struct ClientConnection {
    admission: SessionAdmission,
    secure_session: SecureSession,
    limits: WireLimits,
    closed: bool,
}

impl ClientConnection {
    pub(crate) const fn new(
        admission: SessionAdmission,
        secure_session: SecureSession,
        limits: WireLimits,
    ) -> Self {
        Self {
            admission,
            secure_session,
            limits,
            closed: false,
        }
    }

    /// Authenticated admission and grant policy.
    #[must_use]
    pub const fn admission(&self) -> &SessionAdmission {
        &self.admission
    }

    /// Builds a profile request bound to this runtime lease.
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

    /// Builds a short-lived request carrying only numeric opaque slots.
    ///
    /// # Errors
    ///
    /// Returns an error for empty slots, zero generation or an invalid deadline.
    pub fn opaque_exchange_request(
        &self,
        context: OpaqueExchangeContext,
        slots: &OpaqueSlots,
        now_ms: u64,
    ) -> Result<proto::InnerFrame, ClientError> {
        opaque_exchange_request_for(&self.admission, context, slots, now_ms)
    }

    /// Builds a generation-bound request that starts one closed action flow.
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

    /// Builds an authenticated observation for one previously issued action.
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

    /// Authenticates and encodes one logical request.
    ///
    /// # Errors
    ///
    /// Returns an error when the connection is closed or encoding/encryption fails.
    pub fn seal(
        &mut self,
        request_id: RequestId,
        inner: &proto::InnerFrame,
    ) -> Result<Vec<u8>, ClientError> {
        self.ensure_open()?;
        let plaintext = encode_inner_frame(inner, self.limits)?;
        let ciphertext_len = plaintext
            .len()
            .checked_add(TRANSPORT_TAG_LEN)
            .ok_or(ClientError::Protocol)?;
        let counter = self.secure_session.next_send_counter();
        let header = encode_secure_frame_header(
            self.admission.session_id(),
            counter,
            request_id,
            ciphertext_len,
            self.limits,
        )?;
        let ciphertext = self
            .secure_session
            .seal(counter, &header, &plaintext)
            .map_err(ClientError::from)?;
        let frame = SecureFrame::new(
            self.admission.session_id(),
            counter,
            request_id,
            ciphertext,
            self.limits,
        )?;
        encode_secure_frame(&frame, self.limits).map_err(ClientError::from)
    }

    /// Authenticates and decodes one in-order response.
    ///
    /// # Errors
    ///
    /// Returns an error for closed, malformed, unbound, out-of-order, or unauthenticated input.
    pub fn open(&mut self, encoded: &[u8]) -> Result<(RequestId, proto::InnerFrame), ClientError> {
        self.ensure_open()?;
        let frame = match decode_secure_frame(encoded, self.limits) {
            Ok(frame) => frame,
            Err(error) => {
                self.closed = true;
                return Err(ClientError::from(error));
            }
        };
        if frame.session_id() != self.admission.session_id() {
            self.closed = true;
            return Err(ClientError::SessionBinding);
        }
        let header = encode_secure_frame_header(
            frame.session_id(),
            frame.counter(),
            frame.request_id(),
            frame.ciphertext().len(),
            self.limits,
        )?;
        let plaintext = match self
            .secure_session
            .open(frame.counter(), &header, frame.ciphertext())
        {
            Ok(plaintext) => plaintext,
            Err(error) => {
                self.closed = true;
                return Err(ClientError::from(error));
            }
        };
        let inner = match decode_inner_frame(&plaintext, self.limits) {
            Ok(inner) => inner,
            Err(error) => {
                self.closed = true;
                return Err(ClientError::from(error));
            }
        };
        Ok((frame.request_id(), inner))
    }

    /// Whether this connection is terminally closed.
    #[must_use]
    pub const fn is_closed(&self) -> bool {
        self.closed || self.secure_session.is_closed()
    }

    fn ensure_open(&self) -> Result<(), ClientError> {
        if self.is_closed() {
            Err(ClientError::Closed)
        } else {
            Ok(())
        }
    }
}

pub(crate) fn action_flow_begin_request_for(
    admission: &SessionAdmission,
    context: ActionFlowContext,
    inputs: &OpaqueSlots,
    now_ms: u64,
) -> Result<proto::InnerFrame, ClientError> {
    if inputs.is_empty()
        || context.login_epoch == 0
        || context.transport_epoch == 0
        || context.expires_at_ms == 0
        || now_ms >= context.expires_at_ms
    {
        return Err(ClientError::Protocol);
    }
    let request = proto::ActionFlowBegin {
        runtime_lease: admission.runtime_lease().to_vec(),
        flow_id: context.flow_id.as_bytes().to_vec(),
        account_slot_id: context.account_slot_id.as_bytes().to_vec(),
        login_epoch: context.login_epoch,
        online_epoch: context.online_epoch,
        transport_epoch: context.transport_epoch,
        inputs: inputs.to_wire(),
        expires_at_ms: context.expires_at_ms,
        binding_digest: action_flow_binding_digest(context, inputs)
            .as_bytes()
            .to_vec(),
    };
    Ok(proto::InnerFrame {
        contract: CURRENT_INNER_CONTRACT,
        body: Some(proto::inner_frame::Body::ActionFlowBegin(request)),
    })
}

pub(crate) fn action_observation_request_for(
    admission: &SessionAdmission,
    observation: ActionObservation<'_>,
) -> Result<proto::InnerFrame, ClientError> {
    if observation.observed_at_ms == 0
        || (observation.outcome == ceylith_protocol::ActionObservationKind::Response)
            == observation.payload.is_empty()
        || observation.payload.len() > ceylith_protocol::MAX_ACTION_PAYLOAD_LEN
    {
        return Err(ClientError::Protocol);
    }
    let request = proto::ActionObservation {
        runtime_lease: admission.runtime_lease().to_vec(),
        flow_id: observation.flow_id.as_bytes().to_vec(),
        action_id: observation.action_id.as_bytes().to_vec(),
        action_digest: observation.action_digest.as_bytes().to_vec(),
        outcome: observation.outcome as i32,
        payload: observation.payload.to_vec(),
        observed_at_ms: observation.observed_at_ms,
        binding_digest: action_observation_binding_digest(observation)
            .as_bytes()
            .to_vec(),
    };
    Ok(proto::InnerFrame {
        contract: CURRENT_INNER_CONTRACT,
        body: Some(proto::inner_frame::Body::ActionObservation(request)),
    })
}

pub(crate) fn profile_request_for(
    admission: &SessionAdmission,
    profile_id: ProfileId,
    cached_manifest_digest: Option<Digest32>,
    requested_access: RequestedAccess,
    runtime: &RuntimeDescriptor,
) -> proto::InnerFrame {
    let request = proto::ProfileRequest {
        runtime_lease: admission.runtime_lease().to_vec(),
        profile_id: profile_id.as_bytes().to_vec(),
        cached_manifest_digest: cached_manifest_digest
            .map_or_else(Vec::new, |digest| digest.as_bytes().to_vec()),
        requested_access: requested_access as u32,
        runtime: Some(runtime.as_wire().clone()),
    };
    proto::InnerFrame {
        contract: CURRENT_INNER_CONTRACT,
        body: Some(proto::inner_frame::Body::ProfileRequest(request)),
    }
}

pub(crate) fn opaque_exchange_request_for(
    admission: &SessionAdmission,
    context: OpaqueExchangeContext,
    slots: &OpaqueSlots,
    now_ms: u64,
) -> Result<proto::InnerFrame, ClientError> {
    if slots.is_empty()
        || context.generation == 0
        || context.expires_at_ms == 0
        || now_ms >= context.expires_at_ms
        || context.binding_digest
            != opaque_binding_digest(
                context.exchange_id,
                context.account_slot_id,
                context.generation,
                context.expires_at_ms,
                slots,
            )
    {
        return Err(ClientError::Protocol);
    }
    let request = proto::OpaqueExchangeRequest {
        runtime_lease: admission.runtime_lease().to_vec(),
        exchange_id: context.exchange_id.as_bytes().to_vec(),
        account_slot_id: context.account_slot_id.as_bytes().to_vec(),
        generation: context.generation,
        slots: slots.to_wire(),
        expires_at_ms: context.expires_at_ms,
        binding_digest: context.binding_digest.as_bytes().to_vec(),
    };
    Ok(proto::InnerFrame {
        contract: CURRENT_INNER_CONTRACT,
        body: Some(proto::inner_frame::Body::OpaqueExchangeRequest(request)),
    })
}

pub(crate) fn watch_request_for(
    admission: &SessionAdmission,
    cursor: u64,
    max_wait_ms: u32,
) -> Result<proto::InnerFrame, ClientError> {
    if max_wait_ms == 0 || max_wait_ms > ceylith_protocol::MAX_WATCH_WAIT_MS {
        return Err(ClientError::Protocol);
    }
    Ok(proto::InnerFrame {
        contract: CURRENT_INNER_CONTRACT,
        body: Some(proto::inner_frame::Body::WatchRequest(
            proto::WatchRequest {
                runtime_lease: admission.runtime_lease().to_vec(),
                cursor,
                max_wait_ms,
            },
        )),
    })
}

impl core::fmt::Debug for ClientConnection {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ClientConnection")
            .field("admission", &self.admission)
            .field("secure_session", &self.secure_session)
            .field("limits", &self.limits)
            .field("closed", &self.is_closed())
            .finish()
    }
}

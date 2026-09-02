use crate::{AccountSlotId, CodecError, GrantClass, proto};

const WATCH_IDLE_CODE: u32 = 1;

/// Closed public Watch event kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WatchEventKind {
    /// Current grant is approaching expiry.
    GrantExpiring,
    /// Automatic renewal is paused without revoking the current lease.
    RenewalPaused,
    /// Current grant was explicitly revoked.
    GrantRevoked,
    /// Authenticated account or installation quotas changed.
    QuotaChanged,
    /// Server policy generation changed.
    PolicyChanged,
    /// A compatible Profile changed or became unavailable.
    ProfileChanged,
    /// Server-requested maintenance affects the installation.
    Maintenance,
    /// A previously degraded grant returned to current state.
    GrantRestored,
}

/// Renewal state included in a Watch grant snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenewalState {
    /// Renewal remains current.
    Current,
    /// Renewal is paused while the current grant remains valid.
    Paused,
    /// The grant was explicitly revoked.
    Revoked,
}

/// Validated authorization snapshot delivered by Ceylith Watch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WatchGrantSnapshot {
    grant_class: GrantClass,
    max_full_accounts: u32,
    max_active_installations: u32,
    expires_at_ms: u64,
    renewal_state: RenewalState,
    policy_epoch: u64,
}

impl WatchGrantSnapshot {
    /// Effective authorization class.
    #[must_use]
    pub const fn grant_class(self) -> GrantClass {
        self.grant_class
    }

    /// Maximum simultaneous Full accounts; zero means unlimited for Full only.
    #[must_use]
    pub const fn max_full_accounts(self) -> u32 {
        self.max_full_accounts
    }

    /// Maximum active installations; zero means unlimited for Full only.
    #[must_use]
    pub const fn max_active_installations(self) -> u32 {
        self.max_active_installations
    }

    /// Exclusive grant expiry in Unix epoch milliseconds.
    #[must_use]
    pub const fn expires_at_ms(self) -> u64 {
        self.expires_at_ms
    }

    /// Renewal policy state.
    #[must_use]
    pub const fn renewal_state(self) -> RenewalState {
        self.renewal_state
    }

    /// Monotonic server policy generation.
    #[must_use]
    pub const fn policy_epoch(self) -> u64 {
        self.policy_epoch
    }
}

/// Structurally validated, cursor-bound Watch event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WatchEvent {
    cursor: u64,
    kind: WatchEventKind,
    occurred_at_ms: u64,
    account_slot_id: Option<AccountSlotId>,
    reason_code: u32,
    payload: Box<[u8]>,
    grant: Option<WatchGrantSnapshot>,
}

impl WatchEvent {
    /// Monotonic event cursor.
    #[must_use]
    pub const fn cursor(&self) -> u64 {
        self.cursor
    }

    /// Closed event kind.
    #[must_use]
    pub const fn kind(&self) -> WatchEventKind {
        self.kind
    }

    /// Server event time in Unix epoch milliseconds.
    #[must_use]
    pub const fn occurred_at_ms(&self) -> u64 {
        self.occurred_at_ms
    }

    /// Optional installation-local account scope.
    #[must_use]
    pub const fn account_slot_id(&self) -> Option<AccountSlotId> {
        self.account_slot_id
    }

    /// Stable public reason code within the event kind.
    #[must_use]
    pub const fn reason_code(&self) -> u32 {
        self.reason_code
    }

    /// Bounded public payload with kind-specific future extensions.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Validated grant snapshot when required by the event kind.
    #[must_use]
    pub const fn grant(&self) -> Option<WatchGrantSnapshot> {
        self.grant
    }
}

/// One Watch long-poll outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WatchOutcome {
    /// A strictly newer event was delivered.
    Event(WatchEvent),
    /// The long poll completed without advancing the cursor.
    Idle {
        /// Unchanged caller cursor.
        cursor: u64,
    },
}

/// Decodes a cursor-bound Watch response.
///
/// # Errors
///
/// Returns an error for an unsupported body, a non-advancing event cursor, malformed enums or a
/// contradictory grant snapshot.
pub fn decode_watch_response(
    frame: &proto::InnerFrame,
    requested_cursor: u64,
) -> Result<WatchOutcome, CodecError> {
    crate::inner::validate::validate_inner(frame)?;
    match frame.body.as_ref().ok_or(CodecError::UnsupportedBody)? {
        proto::inner_frame::Body::WatchEvent(value) if value.cursor > requested_cursor => {
            decode_event(value).map(WatchOutcome::Event)
        }
        proto::inner_frame::Body::GenericResult(value)
            if value.accepted && value.code == WATCH_IDLE_CODE && value.payload.is_empty() =>
        {
            Ok(WatchOutcome::Idle {
                cursor: requested_cursor,
            })
        }
        _ => Err(CodecError::InvalidField),
    }
}

fn decode_event(value: &proto::WatchEvent) -> Result<WatchEvent, CodecError> {
    let kind = decode_kind(value.kind)?;
    let account_slot_id = (!value.account_slot_id.is_empty())
        .then(|| AccountSlotId::try_from(value.account_slot_id.as_slice()))
        .transpose()
        .map_err(|_| CodecError::InvalidField)?;
    let grant = value.grant.as_ref().map(decode_grant).transpose()?;
    Ok(WatchEvent {
        cursor: value.cursor,
        kind,
        occurred_at_ms: value.occurred_at_ms,
        account_slot_id,
        reason_code: value.reason_code,
        payload: value.payload.clone().into_boxed_slice(),
        grant,
    })
}

fn decode_kind(value: i32) -> Result<WatchEventKind, CodecError> {
    match proto::WatchEventKind::try_from(value).map_err(|_| CodecError::InvalidField)? {
        proto::WatchEventKind::GrantExpiring => Ok(WatchEventKind::GrantExpiring),
        proto::WatchEventKind::RenewalPaused => Ok(WatchEventKind::RenewalPaused),
        proto::WatchEventKind::GrantRevoked => Ok(WatchEventKind::GrantRevoked),
        proto::WatchEventKind::QuotaChanged => Ok(WatchEventKind::QuotaChanged),
        proto::WatchEventKind::PolicyChanged => Ok(WatchEventKind::PolicyChanged),
        proto::WatchEventKind::ProfileChanged => Ok(WatchEventKind::ProfileChanged),
        proto::WatchEventKind::Maintenance => Ok(WatchEventKind::Maintenance),
        proto::WatchEventKind::GrantRestored => Ok(WatchEventKind::GrantRestored),
        proto::WatchEventKind::Unspecified => Err(CodecError::InvalidField),
    }
}

fn decode_grant(value: &proto::WatchGrantSnapshot) -> Result<WatchGrantSnapshot, CodecError> {
    let grant_class = match proto::GrantClass::try_from(value.grant_class)
        .map_err(|_| CodecError::InvalidField)?
    {
        proto::GrantClass::Public => GrantClass::Public,
        proto::GrantClass::Community => GrantClass::Community,
        proto::GrantClass::Full => GrantClass::Full,
        proto::GrantClass::Unspecified => return Err(CodecError::InvalidField),
    };
    let renewal_state = match proto::RenewalState::try_from(value.renewal_state)
        .map_err(|_| CodecError::InvalidField)?
    {
        proto::RenewalState::Current => RenewalState::Current,
        proto::RenewalState::Paused => RenewalState::Paused,
        proto::RenewalState::Revoked => RenewalState::Revoked,
        proto::RenewalState::Unspecified => return Err(CodecError::InvalidField),
    };
    Ok(WatchGrantSnapshot {
        grant_class,
        max_full_accounts: value.max_full_accounts,
        max_active_installations: value.max_active_installations,
        expires_at_ms: value.expires_at_ms,
        renewal_state,
        policy_epoch: value.policy_epoch,
    })
}

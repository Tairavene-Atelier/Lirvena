use super::{
    OnlineAction, OnlineDirective, OnlineGeneration, OnlinePlan, OnlineState, OnlineTransitionError,
};

/// Single-owner, generation-fenced online controller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OnlineMachine {
    plan: OnlinePlan,
    state: OnlineState,
    heartbeat_interval_ms: u64,
    next_heartbeat_ms: Option<u64>,
    next_delayed_sync_ms: Option<u64>,
    heartbeat_in_flight: bool,
    delayed_sync_in_flight: bool,
}

impl OnlineMachine {
    /// Creates a stopped controller from one already authenticated plan.
    #[must_use]
    pub const fn new(plan: OnlinePlan) -> Self {
        Self {
            plan,
            state: OnlineState::Stopped,
            heartbeat_interval_ms: plan.initial_heartbeat_ms,
            next_heartbeat_ms: None,
            next_delayed_sync_ms: None,
            heartbeat_in_flight: false,
            delayed_sync_in_flight: false,
        }
    }

    /// Returns the current state.
    #[must_use]
    pub const fn state(self) -> OnlineState {
        self.state
    }

    /// Returns the earliest scheduled continuity deadline for an online generation.
    #[must_use]
    pub fn next_due_ms(self) -> Option<u64> {
        if !matches!(self.state, OnlineState::Online(_)) {
            return None;
        }
        let heartbeat = (!self.heartbeat_in_flight)
            .then_some(self.next_heartbeat_ms)
            .flatten();
        let delayed = (!self.delayed_sync_in_flight)
            .then_some(self.next_delayed_sync_ms)
            .flatten();
        [heartbeat, delayed].into_iter().flatten().min()
    }

    /// Starts a fresh generation and requests its initial synchronization.
    ///
    /// # Errors
    ///
    /// Returns an error if another generation still owns the machine.
    pub fn start(
        &mut self,
        generation: OnlineGeneration,
    ) -> Result<OnlineDirective, OnlineTransitionError> {
        if !matches!(
            self.state,
            OnlineState::Stopped | OnlineState::ProtectiveOffline(_)
        ) {
            return Err(OnlineTransitionError);
        }
        self.clear_schedule();
        self.state = OnlineState::Synchronizing(generation);
        Ok(dispatch(
            generation,
            OnlineAction::InitialSync(self.plan.initial_sync),
        ))
    }

    /// Accepts initial synchronization and starts required security bootstrap.
    ///
    /// A server-requested delayed synchronization is recorded but cannot run
    /// until all required startup gates have entered the online state.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale generation or unexpected state.
    pub fn initial_sync_succeeded(
        &mut self,
        generation: OnlineGeneration,
        now_ms: u64,
        delayed_after_ms: Option<u64>,
    ) -> Result<OnlineDirective, OnlineTransitionError> {
        self.require_state(OnlineState::Synchronizing(generation))?;
        self.next_delayed_sync_ms = delayed_after_ms
            .map(|delay| checked_due(now_ms, self.plan.clamp_delayed_sync(delay)))
            .transpose()?;
        self.state = OnlineState::Bootstrapping(generation);
        Ok(dispatch(
            generation,
            OnlineAction::SecurityBootstrap(self.plan.security_bootstrap),
        ))
    }

    /// Accepts required security bootstrap and advances to the optional gate.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale generation or unexpected state.
    pub fn security_bootstrap_succeeded(
        &mut self,
        generation: OnlineGeneration,
        now_ms: u64,
    ) -> Result<OnlineDirective, OnlineTransitionError> {
        self.require_state(OnlineState::Bootstrapping(generation))?;
        if let Some(action) = self.plan.status_confirmation {
            self.state = OnlineState::Confirming(generation);
            Ok(dispatch(
                generation,
                OnlineAction::StatusConfirmation(action),
            ))
        } else {
            self.enter_online(generation, now_ms)
        }
    }

    /// Completes the optional diagnostic status confirmation gate.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale generation or unexpected state.
    pub fn status_confirmation_completed(
        &mut self,
        generation: OnlineGeneration,
        now_ms: u64,
    ) -> Result<OnlineDirective, OnlineTransitionError> {
        self.require_state(OnlineState::Confirming(generation))?;
        self.enter_online(generation, now_ms)
    }

    /// Emits all currently due scheduled work without overlapping an action.
    #[must_use]
    pub fn poll_due(&mut self, now_ms: u64) -> [Option<OnlineDirective>; 2] {
        let OnlineState::Online(generation) = self.state else {
            return [None, None];
        };
        let heartbeat = if !self.heartbeat_in_flight
            && self.next_heartbeat_ms.is_some_and(|due| due <= now_ms)
        {
            self.heartbeat_in_flight = true;
            Some(dispatch(
                generation,
                OnlineAction::BusinessHeartbeat(self.plan.business_heartbeat),
            ))
        } else {
            None
        };
        let delayed = if !self.delayed_sync_in_flight
            && self.next_delayed_sync_ms.is_some_and(|due| due <= now_ms)
        {
            self.delayed_sync_in_flight = true;
            Some(dispatch(
                generation,
                OnlineAction::DelayedSync(self.plan.delayed_sync),
            ))
        } else {
            None
        };
        [heartbeat, delayed]
    }

    /// Records one response and arms the next server-directed heartbeat interval.
    ///
    /// # Errors
    ///
    /// Returns an error when no heartbeat is in flight or the generation is stale.
    pub fn heartbeat_completed(
        &mut self,
        generation: OnlineGeneration,
        now_ms: u64,
        requested_interval_ms: Option<u64>,
    ) -> Result<(), OnlineTransitionError> {
        self.require_state(OnlineState::Online(generation))?;
        if !self.heartbeat_in_flight {
            return Err(OnlineTransitionError);
        }
        let interval = requested_interval_ms.map_or(self.heartbeat_interval_ms, |value| {
            self.plan.clamp_heartbeat(value)
        });
        self.heartbeat_interval_ms = interval;
        self.next_heartbeat_ms = Some(checked_due(now_ms, interval)?);
        self.heartbeat_in_flight = false;
        Ok(())
    }

    /// Records one delayed-sync response and optionally arms its continuation.
    ///
    /// # Errors
    ///
    /// Returns an error when no delayed sync is in flight or the generation is stale.
    pub fn delayed_sync_completed(
        &mut self,
        generation: OnlineGeneration,
        now_ms: u64,
        delayed_after_ms: Option<u64>,
    ) -> Result<(), OnlineTransitionError> {
        self.require_state(OnlineState::Online(generation))?;
        if !self.delayed_sync_in_flight {
            return Err(OnlineTransitionError);
        }
        self.next_delayed_sync_ms = delayed_after_ms
            .map(|delay| checked_due(now_ms, self.plan.clamp_delayed_sync(delay)))
            .transpose()?;
        self.delayed_sync_in_flight = false;
        Ok(())
    }

    /// Fails closed for any required startup or continuity action.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale generation or already stopped controller.
    pub fn required_action_failed(
        &mut self,
        generation: OnlineGeneration,
    ) -> Result<OnlineDirective, OnlineTransitionError> {
        if generation_of(self.state) != Some(generation)
            || matches!(
                self.state,
                OnlineState::Stopped | OnlineState::ProtectiveOffline(_)
            )
        {
            return Err(OnlineTransitionError);
        }
        self.clear_schedule();
        self.state = OnlineState::ProtectiveOffline(generation);
        Ok(OnlineDirective::ProtectiveOffline(generation))
    }

    /// Stops the controller and invalidates every pending generation action.
    pub fn stop(&mut self) {
        self.clear_schedule();
        self.state = OnlineState::Stopped;
    }

    fn enter_online(
        &mut self,
        generation: OnlineGeneration,
        now_ms: u64,
    ) -> Result<OnlineDirective, OnlineTransitionError> {
        self.next_heartbeat_ms = Some(checked_due(now_ms, self.plan.initial_heartbeat_ms)?);
        self.state = OnlineState::Online(generation);
        Ok(OnlineDirective::EnteredOnline(generation))
    }

    fn require_state(&self, expected: OnlineState) -> Result<(), OnlineTransitionError> {
        if self.state == expected {
            Ok(())
        } else {
            Err(OnlineTransitionError)
        }
    }

    fn clear_schedule(&mut self) {
        self.next_heartbeat_ms = None;
        self.next_delayed_sync_ms = None;
        self.heartbeat_interval_ms = self.plan.initial_heartbeat_ms;
        self.heartbeat_in_flight = false;
        self.delayed_sync_in_flight = false;
    }
}

fn dispatch(generation: OnlineGeneration, action: OnlineAction) -> OnlineDirective {
    debug_assert_ne!(action.id().as_bytes(), [0; 16]);
    OnlineDirective::Dispatch { generation, action }
}

fn generation_of(state: OnlineState) -> Option<OnlineGeneration> {
    match state {
        OnlineState::Stopped => None,
        OnlineState::Synchronizing(generation)
        | OnlineState::Bootstrapping(generation)
        | OnlineState::Confirming(generation)
        | OnlineState::Online(generation)
        | OnlineState::ProtectiveOffline(generation) => Some(generation),
    }
}

fn checked_due(now_ms: u64, delay_ms: u64) -> Result<u64, OnlineTransitionError> {
    now_ms.checked_add(delay_ms).ok_or(OnlineTransitionError)
}

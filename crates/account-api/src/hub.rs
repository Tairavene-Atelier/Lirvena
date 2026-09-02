use tokio::sync::broadcast;

use crate::AccountEvent;

const MAX_EVENT_CAPACITY: usize = 4_096;

/// Bounded in-process event hub shared by all adapters.
#[derive(Debug)]
pub struct AccountEventHub {
    sender: broadcast::Sender<AccountEvent>,
}

impl AccountEventHub {
    /// Creates an event hub with a closed capacity bound.
    ///
    /// # Errors
    ///
    /// Returns an error when capacity is zero or exceeds the compiled maximum.
    pub fn new(capacity: usize) -> Result<Self, EventHubError> {
        if capacity == 0 || capacity > MAX_EVENT_CAPACITY {
            return Err(EventHubError::InvalidCapacity);
        }
        let (sender, _receiver) = broadcast::channel(capacity);
        Ok(Self { sender })
    }

    /// Returns a cloneable event publisher.
    #[must_use]
    pub fn publisher(&self) -> AccountEventPublisher {
        AccountEventPublisher {
            sender: self.sender.clone(),
        }
    }

    /// Subscribes at the current live event cursor.
    #[must_use]
    pub fn subscribe(&self) -> AccountEventSubscription {
        AccountEventSubscription {
            receiver: self.sender.subscribe(),
        }
    }
}

/// Cloneable account-event publishing handle.
#[derive(Clone, Debug)]
pub struct AccountEventPublisher {
    sender: broadcast::Sender<AccountEvent>,
}

impl AccountEventPublisher {
    /// Publishes one event without blocking QQ transport progress.
    ///
    /// Returns whether at least one adapter was listening. A missing listener is not an error and
    /// never alters the account safety state.
    #[must_use]
    pub fn publish(&self, event: AccountEvent) -> bool {
        self.sender.send(event).is_ok()
    }
}

/// Bounded live event subscription for one adapter.
#[derive(Debug)]
pub struct AccountEventSubscription {
    receiver: broadcast::Receiver<AccountEvent>,
}

impl AccountEventSubscription {
    /// Receives the next event.
    ///
    /// # Errors
    ///
    /// Returns a closed error after shutdown or a lag error when the adapter failed to keep up.
    /// Lag is explicit; the adapter must not pretend skipped events were delivered.
    pub async fn receive(&mut self) -> Result<AccountEvent, EventHubError> {
        self.receiver.recv().await.map_err(|error| match error {
            broadcast::error::RecvError::Closed => EventHubError::Closed,
            broadcast::error::RecvError::Lagged(_) => EventHubError::Lagged,
        })
    }
}

/// Event hub configuration or delivery failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventHubError {
    /// Queue capacity violated the compiled bound.
    InvalidCapacity,
    /// Event fields violated their public bound.
    InvalidEvent,
    /// All event publishers have stopped.
    Closed,
    /// The receiving adapter fell behind and skipped one or more events.
    Lagged,
}

impl core::fmt::Display for EventHubError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidCapacity => "account event capacity is invalid",
            Self::InvalidEvent => "account event is invalid",
            Self::Closed => "account event stream is closed",
            Self::Lagged => "account event subscriber lagged",
        })
    }
}

impl std::error::Error for EventHubError {}

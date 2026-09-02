use serde_json::Value;
use tokio::sync::broadcast;

const MAX_EVENT_CAPACITY: usize = 4_096;

/// Bounded canonical `OneBot` event fan-out shared by every transport.
#[derive(Debug)]
pub struct OneBotEventBus {
    sender: broadcast::Sender<Value>,
}

impl OneBotEventBus {
    /// Creates a bounded event bus.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero or excessive capacity.
    pub fn new(capacity: usize) -> Result<Self, EventBusError> {
        if capacity == 0 || capacity > MAX_EVENT_CAPACITY {
            return Err(EventBusError::InvalidCapacity);
        }
        let (sender, _receiver) = broadcast::channel(capacity);
        Ok(Self { sender })
    }

    /// Publishes an event without blocking QQ processing.
    #[must_use]
    pub fn publish(&self, event: Value) -> bool {
        self.sender.send(event).is_ok()
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<Value> {
        self.sender.subscribe()
    }
}

/// Event bus configuration failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventBusError {
    /// Capacity violates the compiled bound.
    InvalidCapacity,
}

impl core::fmt::Display for EventBusError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("OneBot event bus capacity is invalid")
    }
}

impl std::error::Error for EventBusError {}

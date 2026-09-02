use serde_json::{Number, Value};

/// Endpoint-specific `OneBot` identifier output encoding.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum IdFormat {
    /// Decimal strings avoid precision loss in JavaScript clients.
    #[default]
    String,
    /// JSON numbers for compatibility with older bots.
    Number,
}

impl IdFormat {
    pub(crate) fn value(self, id: u64) -> Value {
        match self {
            Self::String => Value::String(id.to_string()),
            Self::Number => Value::Number(Number::from(id)),
        }
    }
}

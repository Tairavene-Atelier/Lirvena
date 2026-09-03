use serde::Deserialize;
use serde_json::{Number, Value};

/// Endpoint-specific `OneBot` identifier output encoding.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
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

    pub(crate) fn project_data(self, value: &mut Value) {
        match value {
            Value::Array(values) => {
                for value in values {
                    self.project_data(value);
                }
            }
            Value::Object(values) => {
                for (key, value) in values {
                    if is_identifier_key(key) {
                        self.project_identifier(value);
                    } else {
                        self.project_data(value);
                    }
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }

    fn project_identifier(self, value: &mut Value) {
        let parsed = match value {
            Value::Number(number) => number.as_u64(),
            Value::String(value) => value.parse().ok(),
            _ => None,
        };
        if let Some(parsed) = parsed {
            *value = self.value(parsed);
        }
    }
}

fn is_identifier_key(value: &str) -> bool {
    matches!(
        value,
        "self_id"
            | "user_id"
            | "group_id"
            | "operator_id"
            | "invitor_id"
            | "message_id"
            | "real_id"
    )
}

use serde::Serialize;
use serde_json::Value;

use crate::BackendError;

/// Canonical `OneBot` 11 action response shared by HTTP and WebSocket.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ActionResponse {
    status: &'static str,
    retcode: i32,
    data: Value,
    message: String,
    wording: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    echo: Option<Value>,
}

impl ActionResponse {
    /// Creates a successful response from backend data.
    #[must_use]
    pub fn success(data: Value, echo: Option<Value>) -> Self {
        Self::new("ok", 0, data, String::new(), echo)
    }

    /// Creates the standard accepted asynchronous response.
    #[must_use]
    pub fn asynchronous(echo: Option<Value>) -> Self {
        Self::new("async", 1, Value::Null, String::new(), echo)
    }

    /// Creates a malformed-request response.
    #[must_use]
    pub fn bad_request(echo: Option<Value>, message: impl Into<String>) -> Self {
        Self::new("failed", 1400, Value::Null, message.into(), echo)
    }

    /// Creates an account-selection error.
    #[must_use]
    pub fn account_required(echo: Option<Value>) -> Self {
        Self::new(
            "failed",
            1400,
            Value::Null,
            "self_id is required for a multi-account endpoint".to_owned(),
            echo,
        )
    }

    /// Creates a response from an honest account backend failure.
    #[must_use]
    pub fn backend_failure(echo: Option<Value>, error: &BackendError) -> Self {
        let retcode = match error {
            BackendError::ActionNotFound | BackendError::Unsupported => 1404,
            BackendError::BadParameters(_) => 1400,
            BackendError::AccountUnavailable => 2001,
            BackendError::Overloaded => 2002,
            BackendError::Failed(_) => 2000,
        };
        Self::new("failed", retcode, Value::Null, error.to_string(), echo)
    }

    pub(crate) fn from_nested_failure(response: Self, echo: Option<Value>) -> Self {
        Self { echo, ..response }
    }

    /// Returns the numeric retcode.
    #[must_use]
    pub const fn retcode(&self) -> i32 {
        self.retcode
    }

    fn new(
        status: &'static str,
        retcode: i32,
        data: Value,
        message: String,
        echo: Option<Value>,
    ) -> Self {
        Self {
            status,
            retcode,
            data,
            wording: message.clone(),
            message,
            echo,
        }
    }
}

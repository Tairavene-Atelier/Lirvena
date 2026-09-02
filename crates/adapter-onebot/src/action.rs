use serde_json::{Map, Value};

use crate::ActionResponse;

const MAX_ACTION_BYTES: usize = 128;

/// One validated `OneBot` action call.
#[derive(Clone, Debug, PartialEq)]
pub struct ActionRequest {
    action: String,
    params: Map<String, Value>,
    echo: Option<Value>,
    self_id: Option<u64>,
    mode: ActionMode,
}

impl ActionRequest {
    /// Parses the WebSocket or canonical HTTP action object.
    ///
    /// This validates only protocol shape. It deliberately does not maintain an action whitelist;
    /// standard actions and implementation extensions reach the selected account backend alike.
    ///
    /// # Errors
    ///
    /// Returns a failed `OneBot` response when the object shape, action name or account ID is invalid.
    pub fn from_json(value: Value) -> Result<Self, Box<ActionResponse>> {
        let Value::Object(mut object) = value else {
            return Err(Box::new(ActionResponse::bad_request(
                None,
                "action request must be an object",
            )));
        };
        let echo = object.remove("echo");
        let action = object
            .remove("action")
            .and_then(|value| value.as_str().map(str::to_owned))
            .ok_or_else(|| {
                Box::new(ActionResponse::bad_request(
                    echo.clone(),
                    "action is required",
                ))
            })?;
        let (action, mode) = normalize_action(&action).ok_or_else(|| {
            Box::new(ActionResponse::bad_request(
                echo.clone(),
                "action name is invalid",
            ))
        })?;
        let params = match object.remove("params") {
            None | Some(Value::Null) => Map::new(),
            Some(Value::Object(params)) => params,
            Some(_) => {
                return Err(Box::new(ActionResponse::bad_request(
                    echo,
                    "params must be an object",
                )));
            }
        };
        let self_id = parse_optional_id(object.remove("self_id"), &params)
            .map_err(|error| Box::new(ActionResponse::bad_request(echo.clone(), error)))?;
        Ok(Self {
            action,
            params,
            echo,
            self_id,
            mode,
        })
    }

    /// Builds a request used by the HTTP `/:action` transport.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid action name or `self_id`.
    pub fn from_http(
        action: &str,
        mut params: Map<String, Value>,
    ) -> Result<Self, Box<ActionResponse>> {
        let (action, mode) = normalize_action(action)
            .ok_or_else(|| Box::new(ActionResponse::bad_request(None, "action name is invalid")))?;
        let self_id = parse_optional_id(None, &params)
            .map_err(|error| Box::new(ActionResponse::bad_request(None, error)))?;
        params.remove("self_id");
        Ok(Self {
            action,
            params,
            echo: None,
            self_id,
            mode,
        })
    }

    /// Returns the suffix-free action name.
    #[must_use]
    pub fn action(&self) -> &str {
        &self.action
    }

    /// Returns the unmodified action parameters.
    #[must_use]
    pub const fn params(&self) -> &Map<String, Value> {
        &self.params
    }

    /// Returns the request echo.
    #[must_use]
    pub const fn echo(&self) -> Option<&Value> {
        self.echo.as_ref()
    }

    /// Returns the requested account when supplied.
    #[must_use]
    pub const fn self_id(&self) -> Option<u64> {
        self.self_id
    }

    /// Returns standard suffix behavior.
    #[must_use]
    pub const fn mode(&self) -> ActionMode {
        self.mode
    }

    pub(crate) fn into_backend(self) -> Self {
        Self { echo: None, ..self }
    }

    pub(crate) fn echo_owned(&self) -> Option<Value> {
        self.echo.clone()
    }
}

/// Standard `OneBot` action execution suffix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionMode {
    /// Await the QQ result.
    Synchronous,
    /// Enqueue and return the standard asynchronous acknowledgement.
    Asynchronous,
    /// Await execution through the account's serialized rate-limited lane.
    RateLimited,
}

fn normalize_action(action: &str) -> Option<(String, ActionMode)> {
    if action.is_empty()
        || action.len() > MAX_ACTION_BYTES
        || action.chars().any(char::is_control)
        || action.trim() != action
    {
        return None;
    }
    let (base, mode) = if let Some(base) = action.strip_suffix("_async") {
        (base, ActionMode::Asynchronous)
    } else if let Some(base) = action.strip_suffix("_rate_limited") {
        (base, ActionMode::RateLimited)
    } else {
        (action, ActionMode::Synchronous)
    };
    (!base.is_empty()).then(|| (base.to_owned(), mode))
}

fn parse_optional_id(
    top_level: Option<Value>,
    params: &Map<String, Value>,
) -> Result<Option<u64>, &'static str> {
    let value = top_level.or_else(|| params.get("self_id").cloned());
    value.map_or(Ok(None), |value| parse_id(&value).map(Some))
}

fn parse_id(value: &Value) -> Result<u64, &'static str> {
    match value {
        Value::Number(number) => number.as_u64().filter(|value| *value != 0),
        Value::String(value) => value.parse().ok().filter(|value| *value != 0),
        _ => None,
    }
    .ok_or("self_id must be a positive integer or decimal string")
}

use account_api::AccountActionError;
use serde_json::Value;

pub(super) fn required_u32(value: Option<&Value>) -> Result<u32, AccountActionError> {
    parse_u32(value)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value != 0)
        .ok_or(AccountActionError::BadParameters)
}

pub(super) fn optional_u32(value: Option<&Value>, default: u32) -> Result<u32, AccountActionError> {
    match value {
        Some(value) => parse_u32(Some(value))
            .and_then(|value| u32::try_from(value).ok())
            .ok_or(AccountActionError::BadParameters),
        None => Ok(default),
    }
}

pub(super) fn optional_bool(
    value: Option<&Value>,
    default: bool,
) -> Result<bool, AccountActionError> {
    match value {
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => Err(AccountActionError::BadParameters),
        None => Ok(default),
    }
}

pub(super) fn required_text(value: Option<&Value>) -> Result<&str, AccountActionError> {
    value
        .and_then(Value::as_str)
        .ok_or(AccountActionError::BadParameters)
}

fn parse_u32(value: Option<&Value>) -> Option<u64> {
    value.and_then(|value| match value {
        Value::Number(number) => number.as_u64(),
        Value::String(value) => value.parse().ok(),
        _ => None,
    })
}

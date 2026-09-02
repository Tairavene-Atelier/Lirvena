use account_api::AccountActionError;
use serde_json::Value;

pub(super) fn required_u32(value: Option<&Value>) -> Result<u32, AccountActionError> {
    value
        .and_then(|value| match value {
            Value::Number(number) => number.as_u64(),
            Value::String(value) => value.parse().ok(),
            _ => None,
        })
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value != 0)
        .ok_or(AccountActionError::BadParameters)
}

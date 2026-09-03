use account_api::{AccountActionError, AccountActionRequest};
use account_message_store::RecallTarget;
use qq_control::group_reaction;
use serde_json::{Value, json};

use super::controls::send_control;
use super::message_registry::MessageRegistry;
use super::packets::PacketRuntime;
use super::parameters::{optional_bool, required_text, required_u32};
use super::push::PushRuntime;
use super::runtime::OnlineContext;

pub(super) async fn update(
    request: &AccountActionRequest,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    messages: &mut MessageRegistry,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let group_code = required_u32(request.params().get("group_id"))?;
    let message_id = required_u32(request.params().get("message_id"))?;
    let code = required_text(request.params().get("code"))?;
    let add = optional_bool(request.params().get("is_add"), true)?;
    let record = messages
        .get(message_id)
        .map_err(|_error| AccountActionError::QqFailure)?
        .ok_or(AccountActionError::QqFailure)?;
    let sequence = group_sequence(record.recall(), group_code)?;
    let control = group_reaction(group_code, sequence, code, add)
        .map_err(|_error| AccountActionError::BadParameters)?;
    send_control(&control, packets, pushes, context).await?;
    Ok(json!({}))
}

fn group_sequence(target: &RecallTarget, expected_group: u32) -> Result<u64, AccountActionError> {
    match target {
        RecallTarget::Group {
            group_code,
            sequence,
            ..
        } if *group_code == expected_group => Ok(*sequence),
        _ => Err(AccountActionError::QqFailure),
    }
}

#[cfg(test)]
mod tests {
    use account_message_store::RecallTarget;

    use super::group_sequence;

    #[test]
    fn reaction_correlation_cannot_cross_groups() {
        let target = RecallTarget::Group {
            group_code: 42,
            sequence: 43,
            random: None,
        };
        assert_eq!(group_sequence(&target, 42), Ok(43));
        assert!(group_sequence(&target, 44).is_err());
        assert!(group_sequence(&RecallTarget::Unavailable, 42).is_err());
    }
}

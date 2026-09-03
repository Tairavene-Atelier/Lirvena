use account_api::{AccountActionError, AccountActionRequest};
use account_message_store::RecallTarget;
use qq_control::{ControlRequest, EmojiChainTarget, group_reaction, join_emoji_chain};
use serde_json::{Value, json};

use super::controls::send_control;
use super::message_registry::MessageRegistry;
use super::packets::PacketRuntime;
use super::parameters::{optional_bool, required_text, required_u32};
use super::push::PushRuntime;
use super::runtime::OnlineContext;

pub(super) async fn execute(
    request: &AccountActionRequest,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    messages: &mut MessageRegistry,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let control = match request.action() {
        "set_group_reaction" => group_control(request, messages)?,
        "set_msg_emoji_like" => message_control(request, messages)?,
        ".join_group_emoji_chain" => legacy_group_control(request, messages)?,
        ".join_friend_emoji_chain" => legacy_friend_control(request, messages)?,
        _ => return Err(AccountActionError::ActionNotFound),
    };
    send_control(&control, packets, pushes, context).await?;
    Ok(json!({}))
}

fn group_control(
    request: &AccountActionRequest,
    messages: &MessageRegistry,
) -> Result<ControlRequest, AccountActionError> {
    let group_code = required_u32(request.params().get("group_id"))?;
    let message_id = required_u32(request.params().get("message_id"))?;
    let code = required_text(request.params().get("code"))?;
    let add = optional_bool(request.params().get("is_add"), true)?;
    let record = messages
        .get(message_id)
        .map_err(|_error| AccountActionError::QqFailure)?
        .ok_or(AccountActionError::QqFailure)?;
    let sequence = group_sequence(record.recall(), group_code)?;
    group_reaction(group_code, sequence, code, add)
        .map_err(|_error| AccountActionError::BadParameters)
}

fn message_control(
    request: &AccountActionRequest,
    messages: &MessageRegistry,
) -> Result<ControlRequest, AccountActionError> {
    let message_id = required_u32(request.params().get("message_id"))?;
    let face_id = required_u32(request.params().get("emoji_id"))?;
    let add = optional_bool(
        request.params().get("set"),
        optional_bool(request.params().get("is_add"), true)?,
    )?;
    let record = retained(messages, message_id)?;
    message_target_control(record.recall(), face_id, add)
}

fn message_target_control(
    target: &RecallTarget,
    face_id: u32,
    add: bool,
) -> Result<ControlRequest, AccountActionError> {
    match target {
        RecallTarget::Group {
            group_code,
            sequence,
            ..
        } => group_reaction(*group_code, *sequence, &face_id.to_string(), add)
            .map_err(|_error| AccountActionError::BadParameters),
        RecallTarget::Private {
            uid,
            peer_uin: Some(_),
            sequence,
            ..
        } if add => join_emoji_chain(EmojiChainTarget::Private(uid), *sequence, face_id)
            .map_err(|_error| AccountActionError::BadParameters),
        RecallTarget::Private { .. } | RecallTarget::Unavailable => {
            Err(AccountActionError::Unsupported)
        }
    }
}

fn legacy_group_control(
    request: &AccountActionRequest,
    messages: &MessageRegistry,
) -> Result<ControlRequest, AccountActionError> {
    let group_code = required_u32(request.params().get("group_id"))?;
    let message_id = required_u32(request.params().get("message_id"))?;
    let face_id = required_u32(request.params().get("emoji_id"))?;
    let record = retained(messages, message_id)?;
    let sequence = group_sequence(record.recall(), group_code)?;
    join_emoji_chain(EmojiChainTarget::Group(group_code), sequence, face_id)
        .map_err(|_error| AccountActionError::BadParameters)
}

fn legacy_friend_control(
    request: &AccountActionRequest,
    messages: &MessageRegistry,
) -> Result<ControlRequest, AccountActionError> {
    let user_id = required_u32(request.params().get("user_id"))?;
    let message_id = required_u32(request.params().get("message_id"))?;
    let face_id = required_u32(request.params().get("emoji_id"))?;
    let record = retained(messages, message_id)?;
    let (uid, sequence) = match record.recall() {
        RecallTarget::Private {
            uid,
            peer_uin: Some(peer_uin),
            sequence,
            ..
        } if *peer_uin == user_id => (uid.as_str(), *sequence),
        _ => return Err(AccountActionError::QqFailure),
    };
    join_emoji_chain(EmojiChainTarget::Private(uid), sequence, face_id)
        .map_err(|_error| AccountActionError::BadParameters)
}

fn retained(
    messages: &MessageRegistry,
    message_id: u32,
) -> Result<account_message_store::MessageRecord, AccountActionError> {
    messages
        .get(message_id)
        .map_err(|_error| AccountActionError::QqFailure)?
        .ok_or(AccountActionError::QqFailure)
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

    use super::{group_sequence, message_target_control};

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

    #[test]
    fn generic_reaction_uses_only_complete_conversation_evidence()
    -> Result<(), Box<dyn std::error::Error>> {
        let group = RecallTarget::Group {
            group_code: 42,
            sequence: 43,
            random: None,
        };
        assert_eq!(
            message_target_control(&group, 44, true)?.command(),
            "OidbSvcTrpcTcp.0x9082_1"
        );

        let private = RecallTarget::Private {
            uid: "u_peer".to_owned(),
            peer_uin: Some(42),
            sequence: 43,
            client_sequence: 45,
            random: 46,
            timestamp: 47,
        };
        assert_eq!(
            message_target_control(&private, 44, true)?.command(),
            "OidbSvcTrpcTcp.0x90ee_1"
        );
        assert!(message_target_control(&private, 44, false).is_err());

        let migrated = RecallTarget::Private {
            uid: "u_peer".to_owned(),
            peer_uin: None,
            sequence: 43,
            client_sequence: 45,
            random: 46,
            timestamp: 47,
        };
        assert!(message_target_control(&migrated, 44, true).is_err());
        Ok(())
    }
}

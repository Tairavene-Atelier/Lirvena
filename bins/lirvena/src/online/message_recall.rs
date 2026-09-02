use account_api::{AccountActionError, AccountActionRequest};
use qq_message::{
    GroupRecallInput, PrivateRecallInput, encode_group_recall, encode_private_recall,
};
use serde_json::{Value, json};

use super::message_registry::{MessageRegistry, RecallTarget};
use super::packets::{PacketContext, PacketRuntime};
use super::parameters::required_u32;
use super::push::PushRuntime;
use super::runtime::OnlineContext;

pub(super) async fn recall_message(
    request: &AccountActionRequest,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    messages: &mut MessageRegistry,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let message_id = required_u32(request.params().get("message_id"))?;
    let target = messages
        .get(message_id)
        .cloned()
        .ok_or(AccountActionError::QqFailure)?;
    let (route, body) = encode_target(&target)?;
    packets
        .send_with_reserve(
            PacketContext::for_account(context, pushes.plan()),
            route,
            &[],
            &body,
        )
        .await
        .map_err(|_error| AccountActionError::QqFailure)?;
    messages.remove(message_id);
    Ok(json!({}))
}

fn encode_target(target: &RecallTarget) -> Result<(&'static str, Vec<u8>), AccountActionError> {
    match target {
        RecallTarget::Group {
            group_code,
            sequence,
        } => encode_group_recall(GroupRecallInput {
            group_uin: u64::from(*group_code),
            sequence: *sequence,
        })
        .map(|body| ("trpc.msg.msg_svc.MsgService.SsoGroupRecallMsg", body))
        .map_err(|_error| AccountActionError::QqFailure),
        RecallTarget::Private {
            uid,
            sequence,
            client_sequence,
            random,
            timestamp,
        } => encode_private_recall(PrivateRecallInput {
            target_uid: uid,
            sequence: *sequence,
            client_sequence: *client_sequence,
            random: *random,
            timestamp: *timestamp,
        })
        .map(|body| ("trpc.msg.msg_svc.MsgService.SsoC2CRecallMsg", body))
        .map_err(|_error| AccountActionError::QqFailure),
        RecallTarget::Unavailable => Err(AccountActionError::QqFailure),
    }
}

#[cfg(test)]
mod tests {
    use super::{RecallTarget, encode_target};
    use account_api::AccountActionError;

    #[test]
    fn unavailable_message_never_becomes_a_fake_success() {
        assert_eq!(
            encode_target(&RecallTarget::Unavailable),
            Err(AccountActionError::QqFailure)
        );
    }
}

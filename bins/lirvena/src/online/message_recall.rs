use account_api::{AccountActionError, AccountActionRequest};
use account_message_store::RecallTarget;
use qq_message::{
    GroupRecallInput, PrivateRecallInput, encode_group_recall, encode_private_recall,
    validate_group_recall_response, validate_private_recall_response,
};
use serde_json::{Value, json};

use crate::opaque::{OpaqueOperation, request_reserve};

use super::message_registry::MessageRegistry;
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
        .map_err(|_error| AccountActionError::QqFailure)?
        .ok_or(AccountActionError::QqFailure)?;
    let recall = encode_target(target.recall())?;
    let reserve = match recall.response {
        RecallResponse::Group { .. } => Vec::new(),
        RecallResponse::Private { .. } => request_reserve(
            context.ceylith,
            context.account_slot_id,
            OpaqueOperation::numeric(7),
            &recall.body,
        )
        .await
        .map_err(|_error| AccountActionError::QqFailure)?,
    };
    let response = packets
        .send_with_reserve(
            PacketContext::for_account(context, pushes.plan()),
            recall.route,
            &reserve,
            &recall.body,
        )
        .await
        .map_err(|_error| AccountActionError::QqFailure)?;
    recall.response.validate(&response)?;
    messages
        .remove(message_id)
        .map_err(|_error| AccountActionError::QqFailure)?;
    Ok(json!({}))
}

struct EncodedRecall {
    route: &'static str,
    body: Vec<u8>,
    response: RecallResponse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RecallResponse {
    Group { sequence: u64 },
    Private { client_sequence: u64 },
}

impl RecallResponse {
    fn validate(self, input: &[u8]) -> Result<(), AccountActionError> {
        match self {
            Self::Group { sequence } => validate_group_recall_response(input, sequence),
            Self::Private { client_sequence } => {
                validate_private_recall_response(input, client_sequence)
            }
        }
        .map_err(|_error| AccountActionError::QqFailure)
    }
}

fn encode_target(target: &RecallTarget) -> Result<EncodedRecall, AccountActionError> {
    match target {
        RecallTarget::Group {
            group_code,
            sequence,
        } => encode_group_recall(GroupRecallInput {
            group_uin: u64::from(*group_code),
            sequence: *sequence,
        })
        .map(|body| EncodedRecall {
            route: "trpc.msg.msg_svc.MsgService.SsoGroupRecallMsg",
            body,
            response: RecallResponse::Group {
                sequence: *sequence,
            },
        })
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
        .map(|body| EncodedRecall {
            route: "trpc.msg.msg_svc.MsgService.SsoC2CRecallMsg",
            body,
            response: RecallResponse::Private {
                client_sequence: *client_sequence,
            },
        })
        .map_err(|_error| AccountActionError::QqFailure),
        RecallTarget::Unavailable => Err(AccountActionError::QqFailure),
    }
}

#[cfg(test)]
mod tests {
    use super::{RecallResponse, encode_target};
    use account_api::AccountActionError;
    use account_message_store::RecallTarget;

    #[test]
    fn unavailable_message_never_becomes_a_fake_success() {
        assert!(matches!(
            encode_target(&RecallTarget::Unavailable),
            Err(AccountActionError::QqFailure)
        ));
    }

    #[test]
    fn response_binding_tracks_the_request_correlation() -> Result<(), AccountActionError> {
        let group = encode_target(&RecallTarget::Group {
            group_code: 12,
            sequence: 34,
        })?;
        assert_eq!(group.response, RecallResponse::Group { sequence: 34 });

        let private = encode_target(&RecallTarget::Private {
            uid: "u_peer".to_owned(),
            sequence: 34,
            client_sequence: 56,
            random: 78,
            timestamp: 90,
        })?;
        assert_eq!(
            private.response,
            RecallResponse::Private {
                client_sequence: 56,
            }
        );
        Ok(())
    }
}

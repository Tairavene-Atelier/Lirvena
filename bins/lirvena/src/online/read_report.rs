use account_api::{AccountActionError, AccountActionRequest};
use account_message_store::RecallTarget;
use qq_message::{ReadReportInput, encode_read_report, validate_read_report_response};
use serde_json::{Value, json};

use crate::opaque::{OpaqueOperation, request_reserve};

use super::message_registry::MessageRegistry;
use super::packets::{PacketContext, PacketRuntime};
use super::parameters::required_u32;
use super::push::PushRuntime;
use super::runtime::OnlineContext;

const ROUTE: &str = "trpc.msg.msg_svc.MsgService.SsoReadedReport";

pub(super) async fn mark_message_read(
    request: &AccountActionRequest,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    messages: &mut MessageRegistry,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let message_id = required_u32(request.params().get("message_id"))?;
    let record = messages
        .get(message_id)
        .map_err(|_error| AccountActionError::QqFailure)?
        .ok_or(AccountActionError::QqFailure)?;
    let body = encode_target(record.recall())?;
    let reserve = request_reserve(
        context.ceylith,
        context.account_slot_id,
        OpaqueOperation::numeric(8),
        &body,
    )
    .await
    .map_err(|_error| AccountActionError::QqFailure)?;
    let response = packets
        .send_with_reserve(
            PacketContext::for_account(context, pushes.plan()),
            ROUTE,
            &reserve,
            &body,
        )
        .await
        .map_err(|_error| AccountActionError::QqFailure)?;
    validate_read_report_response(&response).map_err(|_error| AccountActionError::QqFailure)?;
    Ok(json!({}))
}

fn encode_target(target: &RecallTarget) -> Result<Vec<u8>, AccountActionError> {
    let input = match target {
        RecallTarget::Group {
            group_code,
            sequence,
            ..
        } => ReadReportInput::Group {
            group_uin: u64::from(*group_code),
            sequence: *sequence,
        },
        RecallTarget::Private {
            uid,
            sequence,
            timestamp,
            ..
        } => ReadReportInput::Private {
            target_uid: uid,
            timestamp: *timestamp,
            sequence: *sequence,
        },
        RecallTarget::Unavailable => return Err(AccountActionError::QqFailure),
    };
    encode_read_report(input).map_err(|_error| AccountActionError::QqFailure)
}

#[cfg(test)]
mod tests {
    use account_api::AccountActionError;
    use account_message_store::RecallTarget;

    use super::encode_target;

    #[test]
    fn durable_group_and_private_correlations_compile() -> Result<(), AccountActionError> {
        assert_eq!(
            encode_target(&RecallTarget::Group {
                group_code: 42,
                sequence: 55,
                random: None,
            })?,
            [0x0a, 0x04, 0x08, 0x2a, 0x10, 0x37]
        );
        assert_eq!(
            encode_target(&RecallTarget::Private {
                uid: "u_peer".to_owned(),
                sequence: 55,
                client_sequence: 1,
                random: 2,
                timestamp: 100,
            })?,
            [
                0x12, 0x0c, 0x12, 0x06, 0x75, 0x5f, 0x70, 0x65, 0x65, 0x72, 0x18, 0x64, 0x20, 0x37,
            ]
        );
        Ok(())
    }

    #[test]
    fn unavailable_message_never_becomes_a_fake_success() {
        assert!(matches!(
            encode_target(&RecallTarget::Unavailable),
            Err(AccountActionError::QqFailure)
        ));
    }
}

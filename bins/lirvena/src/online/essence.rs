use account_api::{AccountActionError, AccountActionRequest};
use account_message_store::RecallTarget;
use qq_control::{delete_group_essence, set_group_essence};
use serde_json::{Value, json};

use super::controls::send_control;
use super::message_registry::MessageRegistry;
use super::packets::PacketRuntime;
use super::parameters::required_u32;
use super::push::PushRuntime;
use super::runtime::OnlineContext;

pub(super) async fn update(
    request: &AccountActionRequest,
    set: bool,
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
    let (group_code, sequence, random) = essence_correlation(record.recall())?;
    let control = if set {
        set_group_essence(group_code, sequence, random)
    } else {
        delete_group_essence(group_code, sequence, random)
    }
    .map_err(|_error| AccountActionError::QqFailure)?;
    send_control(&control, packets, pushes, context).await?;
    Ok(json!({}))
}

fn essence_correlation(target: &RecallTarget) -> Result<(u32, u64, u32), AccountActionError> {
    match target {
        RecallTarget::Group {
            group_code,
            sequence,
            random: Some(random),
        } => Ok((*group_code, *sequence, *random)),
        _ => Err(AccountActionError::QqFailure),
    }
}

#[cfg(test)]
mod tests {
    use account_message_store::RecallTarget;

    use super::essence_correlation;

    #[test]
    fn migrated_and_private_records_cannot_fabricate_essence_correlation() {
        assert!(
            essence_correlation(&RecallTarget::Group {
                group_code: 1,
                sequence: 2,
                random: None,
            })
            .is_err()
        );
        assert!(
            essence_correlation(&RecallTarget::Private {
                uid: "u_peer".to_owned(),
                sequence: 1,
                client_sequence: 2,
                random: 3,
                timestamp: 4,
            })
            .is_err()
        );
    }
}

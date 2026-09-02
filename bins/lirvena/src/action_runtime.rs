use std::io;
use std::time::Duration;

use ceylith_client::{
    ActionDirective, ActionFlowUpdate, InstallationClient, action_flow_inputs,
    decode_action_flow_update,
};
use ceylith_protocol::{
    AccountSlotId, ActionFlowContext, ActionFlowId, ActionObservation, ActionObservationKind,
};
use qq_envelope::{EnvelopeMark, encode_marked_reserve};
use qq_login::{CredentialLogin, QrDevice};
use qq_profile::{LinuxNtProfile, PushPlan, decode_online_packet_plan};
use qq_session::AuthenticatedSession;
use sha2::{Digest, Sha256};
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout};

use crate::qq::{QqRequest, authenticated, exchange, prepare, session_key};
use crate::support::{encode_hex, now_ms, random_array, random_nonzero_u32};

const FLOW_LIFETIME_MS: u64 = 30_000;
const LOGIN_EPOCH: u64 = 1;
const ONLINE_EPOCH: u64 = 1;
const TRANSPORT_EPOCH: u64 = 1;
const REQUIRED_RESPONSE_POLICY: u32 = 2;
const MAX_FLOW_ACTIONS: usize = 8;

pub(super) struct BootstrapContext<'a> {
    pub ceylith: &'a InstallationClient,
    pub qq: &'a mut AuthenticatedSession<TcpStream>,
    pub push_plan: &'a PushPlan,
    pub profile: &'a LinuxNtProfile,
    pub device: &'a QrDevice,
    pub credential: &'a CredentialLogin,
    pub uin: u64,
    pub account_slot_id: AccountSlotId,
}

pub(super) async fn run(context: BootstrapContext<'_>) -> Result<(), Box<dyn std::error::Error>> {
    let BootstrapContext {
        ceylith,
        qq,
        push_plan,
        profile,
        device,
        credential,
        uin,
        account_slot_id,
    } = context;
    let key = session_key(credential)?;
    let auth = authenticated(uin, credential, &key)?;
    let flow_id = ActionFlowId::from_bytes(random_array()?);
    let started_at_ms = now_ms()?;
    let expires_at_ms = started_at_ms
        .checked_add(FLOW_LIFETIME_MS)
        .ok_or_else(|| io::Error::other("action-flow deadline overflow"))?;
    let inputs = action_flow_inputs(
        random_array::<8>()?.to_vec(),
        random_array::<16>()?.to_vec(),
    )?;
    let flow = ActionFlowContext {
        flow_id,
        account_slot_id,
        login_epoch: LOGIN_EPOCH,
        online_epoch: ONLINE_EPOCH,
        transport_epoch: TRANSPORT_EPOCH,
        expires_at_ms,
    };
    let request = ceylith.action_flow_begin_request(flow, &inputs, started_at_ms)?;
    let response = ceylith.exchange(request).await?;
    let mut update = decode_action_flow_update(&response, flow_id, now_ms()?)?;
    for _step in 0..MAX_FLOW_ACTIONS {
        let action = match update {
            ActionFlowUpdate::Complete => return Ok(()),
            ActionFlowUpdate::Action(action) => action,
        };
        let observed =
            execute_action(qq, push_plan, profile, device, credential, &auth, &action).await;
        let outcome = observed.kind;
        let request = ceylith.action_observation_request(ActionObservation {
            flow_id,
            action_id: action.action_id(),
            action_digest: action.action_digest(),
            outcome: observed.kind,
            payload: &observed.payload,
            observed_at_ms: now_ms()?,
        })?;
        let response = ceylith.exchange(request).await?;
        let next = decode_action_flow_update(&response, flow_id, now_ms()?)?;
        if outcome != ActionObservationKind::Response {
            return Err(io::Error::other(
                "Ceylith action failed; the QQ transport generation must be discarded",
            )
            .into());
        }
        update = next;
    }
    Err(io::Error::other("Ceylith action flow exceeded the compiled action bound").into())
}

struct ObservedAction {
    kind: ActionObservationKind,
    payload: Vec<u8>,
}

async fn execute_action(
    qq: &mut AuthenticatedSession<TcpStream>,
    push_plan: &PushPlan,
    profile: &LinuxNtProfile,
    device: &QrDevice,
    credential: &CredentialLogin,
    auth: &qq_envelope::SessionAuth<'_>,
    action: &ActionDirective,
) -> ObservedAction {
    let prepared = match prepare_action(profile, device, credential, auth, action) {
        Ok(prepared) => prepared,
        Err(_error) => {
            return observed(ActionObservationKind::LocalEncodeFailure, Vec::new());
        }
    };
    if action.delay_ms() != 0 {
        sleep(Duration::from_millis(u64::from(action.delay_ms()))).await;
    }
    match timeout(
        Duration::from_millis(u64::from(action.timeout_ms())),
        exchange(
            qq,
            push_plan,
            auth,
            prepared.sequence,
            &prepared.command,
            &prepared.frame,
        ),
    )
    .await
    {
        Ok(Ok(payload)) => observed(ActionObservationKind::Response, payload),
        Ok(Err(_error)) => observed(ActionObservationKind::TransportFailure, Vec::new()),
        Err(_elapsed) => observed(ActionObservationKind::Timeout, Vec::new()),
    }
}

struct PreparedAction {
    sequence: u32,
    command: String,
    frame: Vec<u8>,
}

fn prepare_action(
    profile: &LinuxNtProfile,
    device: &QrDevice,
    credential: &CredentialLogin,
    auth: &qq_envelope::SessionAuth<'_>,
    action: &ActionDirective,
) -> Result<PreparedAction, Box<dyn std::error::Error>> {
    if action.transport_epoch() != TRANSPORT_EPOCH
        || action.response_policy() != REQUIRED_RESPONSE_POLICY
        || Sha256::digest(action.body_shard()).as_slice()
            != action.expected_body_digest().as_bytes()
    {
        return Err(io::Error::other("authenticated QQ action failed local admission").into());
    }
    let command = std::str::from_utf8(action.route_shard())?.to_owned();
    let marks = action
        .marks()
        .iter()
        .map(|mark| EnvelopeMark {
            slot: mark.slot(),
            value: mark.value(),
        })
        .collect::<Vec<_>>();
    let reserve = encode_marked_reserve(
        action.envelope_contract(),
        &marks,
        &correlation()?,
        credential.uid(),
    )?;
    let packet_plan = decode_online_packet_plan(profile)?;
    let locale_id = u32::try_from(packet_plan.tuning().spec().locale_id)
        .map_err(|_error| io::Error::other("Profile locale exceeds u32"))?;
    let sequence = random_nonzero_u32()?;
    let device_guid_hex = encode_hex(device.guid());
    let request = QqRequest {
        auth,
        sequence,
        locale_id,
        command: &command,
        device_guid_hex: device_guid_hex.as_bytes(),
        reserve: &reserve,
        payload: action.body_shard(),
    };
    let frame = prepare(profile, &request)?;
    Ok(PreparedAction {
        sequence,
        command,
        frame,
    })
}

fn correlation() -> Result<String, io::Error> {
    Ok(format!(
        "01-{}-{}-01",
        encode_hex(&random_array::<16>()?),
        encode_hex(&random_array::<8>()?)
    ))
}

const fn observed(kind: ActionObservationKind, payload: Vec<u8>) -> ObservedAction {
    ObservedAction { kind, payload }
}

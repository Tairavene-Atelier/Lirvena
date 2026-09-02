use std::io;

use qq_login::{CredentialLogin, QrDevice};
use qq_online::{
    HeartbeatInput, HeartbeatOutcome, InfoSyncInput, InfoSyncOutcome, OnlineDevice,
    OnlineSyncState, RegisterInput, encode_heartbeat, encode_info_sync, encode_register,
    parse_heartbeat_response, parse_info_sync_response, parse_register_response,
};
use qq_profile::{LinuxNtProfile, OnlinePacketPlan, PushPlan, decode_online_packet_plan};
use qq_session::AuthenticatedSession;
use tokio::net::TcpStream;

use crate::qq::{QqRequest, authenticated, execute, prepare, session_key};
use crate::support::{encode_hex, now_seconds, random_nonzero_u32};

pub(super) struct PacketRuntime {
    plan: OnlinePacketPlan,
    device: OnlineDevice,
    device_guid_hex: String,
    locale_id: u32,
    state: OnlineSyncState,
}

pub(super) struct PacketContext<'a> {
    pub qq: &'a mut AuthenticatedSession<TcpStream>,
    pub push_plan: &'a PushPlan,
    pub profile: &'a LinuxNtProfile,
    pub credential: &'a CredentialLogin,
    pub uin: u64,
}

impl PacketRuntime {
    pub(super) fn new(
        profile: &LinuxNtProfile,
        device: &QrDevice,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let plan = decode_online_packet_plan(profile)?;
        let locale_id = u32::try_from(plan.tuning().spec().locale_id)
            .map_err(|_error| io::Error::other("Profile locale exceeds u32"))?;
        let device_guid_hex = encode_hex(device.guid());
        Ok(Self {
            plan,
            device: OnlineDevice::new(
                &device_guid_hex,
                device.name().to_owned(),
                profile.operating_system().to_owned(),
                String::new(),
                profile.operating_system().to_ascii_lowercase(),
                profile.client_version().to_owned(),
                100,
            )?,
            device_guid_hex,
            locale_id,
            state: OnlineSyncState::default(),
        })
    }

    pub(super) fn has_status_confirmation(&self) -> bool {
        self.plan.status_register_route().is_some()
    }

    pub(super) const fn sync_state_mut(&mut self) -> &mut OnlineSyncState {
        &mut self.state
    }

    pub(super) async fn acknowledge_push(
        &self,
        qq: &mut AuthenticatedSession<TcpStream>,
        profile: &LinuxNtProfile,
        auth: &qq_envelope::SessionAuth<'_>,
        route: &str,
        body: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let request = QqRequest {
            auth,
            sequence: random_nonzero_u32()?,
            locale_id: self.locale_id,
            command: route,
            device_guid_hex: self.device_guid_hex.as_bytes(),
            reserve: &[],
            payload: body,
        };
        let frame = prepare(profile, &request)?;
        qq.send(&frame).await.map_err(Into::into)
    }

    pub(super) async fn synchronize(
        &mut self,
        context: PacketContext<'_>,
        delayed: bool,
    ) -> Result<InfoSyncOutcome, Box<dyn std::error::Error>> {
        let request_random = random_nonzero_u32()?;
        let payload = encode_info_sync(InfoSyncInput {
            device: &self.device,
            state: self.state,
            tuning: self.plan.tuning(),
            request_random,
            delayed,
        })?;
        let route = if delayed {
            self.plan.delayed_sync_route()
        } else {
            self.plan.initial_sync_route()
        };
        let response = self.send(context, route, &payload).await?;
        let outcome = parse_info_sync_response(&response)?;
        if !outcome.success {
            return Err(io::Error::other("QQ rejected online synchronization").into());
        }
        if outcome.response_random != request_random {
            eprintln!("WARNING: QQ returned a different online synchronization correlation");
        }
        self.apply_silence(outcome.local_silence, outcome.silence_version);
        self.state.first_register = false;
        Ok(outcome)
    }

    pub(super) async fn confirm_status(
        &mut self,
        context: PacketContext<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let route = self
            .plan
            .status_register_route()
            .ok_or_else(|| io::Error::other("Profile omitted required status route"))?;
        let payload = encode_register(RegisterInput {
            device: &self.device,
            state: self.state,
            tuning: self.plan.tuning(),
        })?;
        let response = self.send(context, route, &payload).await?;
        let outcome = parse_register_response(&response)?;
        if outcome.result != 0 {
            return Err(io::Error::other("QQ rejected online status confirmation").into());
        }
        self.apply_silence(outcome.local_silence, outcome.silence_version);
        Ok(())
    }

    pub(super) async fn heartbeat(
        &mut self,
        context: PacketContext<'_>,
    ) -> Result<HeartbeatOutcome, Box<dyn std::error::Error>> {
        let payload = encode_heartbeat(HeartbeatInput {
            state: self.state,
            tuning: self.plan.tuning(),
            unix_seconds: u64::from(now_seconds()?),
            battery_state: self.device.battery_state(),
        })?;
        let response = self
            .send(context, self.plan.heartbeat_route(), &payload)
            .await?;
        parse_heartbeat_response(&response).map_err(Into::into)
    }

    pub(super) async fn send_with_reserve(
        &self,
        context: PacketContext<'_>,
        command: &str,
        reserve: &[u8],
        payload: &[u8],
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let key = session_key(context.credential)?;
        let auth = authenticated(context.uin, context.credential, &key)?;
        execute(
            context.qq,
            context.profile,
            context.push_plan,
            QqRequest {
                auth: &auth,
                sequence: random_nonzero_u32()?,
                locale_id: self.locale_id,
                command,
                device_guid_hex: self.device_guid_hex.as_bytes(),
                reserve,
                payload,
            },
        )
        .await
    }

    async fn send(
        &self,
        context: PacketContext<'_>,
        command: &str,
        payload: &[u8],
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        self.send_with_reserve(context, command, &[], payload).await
    }

    fn apply_silence(&mut self, local: Option<u32>, version: Option<u32>) {
        if let Some(value) = local {
            self.state.local_silence = value;
        }
        if let Some(value) = version {
            self.state.silence_version = value;
        }
    }
}

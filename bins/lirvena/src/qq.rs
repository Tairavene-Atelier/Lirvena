use std::io;

use qq_envelope::{
    ExpectedSsoResponse, QqTeaKey, SessionAuth, SessionRequestParts, decode_session_response,
    encode_session_request,
};
use qq_login::CredentialLogin;
use qq_profile::{LinuxNtProfile, PushPlan};
use qq_session::AuthenticatedSession;
use tokio::net::TcpStream;

pub(crate) struct QqRequest<'a> {
    pub auth: &'a SessionAuth<'a>,
    pub sequence: u32,
    pub locale_id: u32,
    pub command: &'a str,
    pub device_guid_hex: &'a [u8],
    pub reserve: &'a [u8],
    pub payload: &'a [u8],
}

pub(crate) fn session_key(credential: &CredentialLogin) -> Result<QqTeaKey, io::Error> {
    let bytes: [u8; QqTeaKey::LENGTH] = credential
        .secrets()
        .d2_key()
        .try_into()
        .map_err(|_error| io::Error::other("QQ returned an invalid session key"))?;
    Ok(QqTeaKey::new(bytes))
}

pub(crate) fn authenticated<'a>(
    uin: u64,
    credential: &'a CredentialLogin,
    key: &'a QqTeaKey,
) -> Result<SessionAuth<'a>, qq_envelope::EnvelopeError> {
    let uin = u32::try_from(uin).map_err(|_error| qq_envelope::EnvelopeError::InvalidField)?;
    SessionAuth::authenticated(
        uin,
        credential.secrets().tgt(),
        credential.secrets().d2(),
        key,
    )
}

pub(crate) async fn execute(
    qq: &mut AuthenticatedSession<TcpStream>,
    profile: &LinuxNtProfile,
    push_plan: &PushPlan,
    request: QqRequest<'_>,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let frame = prepare(profile, &request)?;
    exchange(
        qq,
        push_plan,
        request.auth,
        request.sequence,
        request.command,
        &frame,
    )
    .await
}

pub(crate) async fn execute_anonymous(
    qq: &mut qq_transport::QqTransport<TcpStream>,
    profile: &LinuxNtProfile,
    request: QqRequest<'_>,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let frame = prepare(profile, &request)?;
    qq.write_frame(&frame).await?;
    let response = qq.read_frame().await?;
    let sso = decode_session_response(
        &response,
        ExpectedSsoResponse {
            auth: request.auth,
            sequence: request.sequence,
            command: request.command,
        },
    )?;
    if sso.return_code() != 0 {
        return Err(io::Error::other("QQ rejected the request").into());
    }
    Ok(sso.payload().to_vec())
}

pub(crate) fn prepare(
    profile: &LinuxNtProfile,
    request: &QqRequest<'_>,
) -> Result<Vec<u8>, qq_envelope::EnvelopeError> {
    encode_session_request(SessionRequestParts {
        auth: request.auth,
        sequence: request.sequence,
        sub_app_id: profile.sub_app_id(),
        locale_id: request.locale_id,
        command: request.command,
        device_guid_hex: request.device_guid_hex,
        client_version: profile.client_version(),
        reserve: request.reserve,
        payload: request.payload,
    })
}

pub(crate) async fn exchange(
    qq: &mut AuthenticatedSession<TcpStream>,
    push_plan: &PushPlan,
    auth: &SessionAuth<'_>,
    sequence: u32,
    command: &str,
    frame: &[u8],
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    qq.exchange(auth, sequence, command, frame, |route| {
        push_plan.find(route).is_some()
    })
    .await
    .map_err(Into::into)
}

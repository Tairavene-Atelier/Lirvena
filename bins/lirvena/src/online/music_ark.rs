use account_api::{AccountActionError, AccountActionRequest};
use reqwest::{Method, Url};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::packets::PacketRuntime;
use super::push::PushRuntime;
use super::runtime::OnlineContext;
use super::ticket::{TicketAccess, TicketRuntime};
use crate::support::now_seconds;

const DOMAIN: &str = "docs.qq.com";
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_FIELD_BYTES: usize = 8 * 1024;

pub(super) async fn get(
    request: &AccountActionRequest,
    uin: u64,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    tickets: &mut TicketRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let input = MusicInput::parse(request)?;
    let login_url = endpoint("https://docs.qq.com/api/user/qq/login")?;
    let response = tickets
        .authenticated_request(
            Method::GET,
            login_url,
            DOMAIN,
            TicketAccess::new(uin, packets, pushes, context),
        )
        .await?
        .header("accept", "application/json, text/plain, */*")
        .header("referer", "https://docs.qq.com")
        .send()
        .await
        .map_err(|_error| AccountActionError::QqFailure)?;
    let body = TicketRuntime::bounded_body(response, MAX_RESPONSE_BYTES).await?;
    let login: LoginResponse =
        serde_json::from_slice(&body).map_err(|_error| AccountActionError::QqFailure)?;
    let login = login.result.ok_or(AccountActionError::QqFailure)?;
    validate_secret(&login.uid)?;
    validate_secret(&login.uid_key)?;

    let timestamp = now_seconds().map_err(|_error| AccountActionError::QqFailure)?;
    let ark = serde_json::to_string(&MusicArk::new(input, uin, timestamp))
        .map_err(|_error| AccountActionError::QqFailure)?;
    let request_body = serde_json::to_vec(&ArkRequest {
        ark,
        object_id: "YjONkUwkdtFr",
    })
    .map_err(|_error| AccountActionError::QqFailure)?;
    let response = tickets
        .authenticated_request_with_cookie_suffix(
            Method::POST,
            endpoint("https://docs.qq.com/v2/push/ark_sig")?,
            DOMAIN,
            TicketAccess::new(uin, packets, pushes, context),
            &format!("uid={}; uid_key={}", login.uid, login.uid_key),
        )
        .await?
        .header("content-type", "application/json")
        .body(request_body)
        .send()
        .await
        .map_err(|_error| AccountActionError::QqFailure)?;
    let body = TicketRuntime::bounded_body(response, MAX_RESPONSE_BYTES).await?;
    let response: ArkResponse =
        serde_json::from_slice(&body).map_err(|_error| AccountActionError::QqFailure)?;
    let ark = response
        .result
        .and_then(|result| result.ark_with_sig)
        .ok_or(AccountActionError::QqFailure)?;
    validate_field(&ark, false)?;
    Ok(Value::String(ark))
}

fn endpoint(value: &str) -> Result<Url, AccountActionError> {
    Url::parse(value).map_err(|_error| AccountActionError::QqFailure)
}

#[derive(Clone, Copy)]
struct MusicInput<'a> {
    title: &'a str,
    description: &'a str,
    jump_url: &'a str,
    music_url: &'a str,
    source_icon: &'a str,
    tag: &'a str,
    preview: &'a str,
}

impl<'a> MusicInput<'a> {
    fn parse(request: &'a AccountActionRequest) -> Result<Self, AccountActionError> {
        let input = Self {
            title: parameter(request, "title")?,
            description: parameter(request, "desc")?,
            jump_url: parameter(request, "jumpUrl")?,
            music_url: parameter(request, "musicUrl")?,
            source_icon: parameter(request, "source_icon")?,
            tag: parameter(request, "tag")?,
            preview: parameter(request, "preview")?,
        };
        validate_field(input.title, false)?;
        for value in [
            input.description,
            input.jump_url,
            input.music_url,
            input.source_icon,
            input.tag,
            input.preview,
        ] {
            validate_field(value, true)?;
        }
        Ok(input)
    }
}

fn parameter<'a>(
    request: &'a AccountActionRequest,
    name: &str,
) -> Result<&'a str, AccountActionError> {
    request
        .params()
        .get(name)
        .and_then(Value::as_str)
        .ok_or(AccountActionError::BadParameters)
}

fn validate_field(value: &str, allow_empty: bool) -> Result<(), AccountActionError> {
    if (allow_empty || !value.is_empty())
        && value.len() <= MAX_FIELD_BYTES
        && !value.chars().any(char::is_control)
    {
        Ok(())
    } else {
        Err(AccountActionError::BadParameters)
    }
}

fn validate_secret(value: &str) -> Result<(), AccountActionError> {
    if !value.is_empty()
        && value.len() <= 4096
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        Ok(())
    } else {
        Err(AccountActionError::QqFailure)
    }
}

#[derive(Deserialize)]
struct LoginResponse {
    result: Option<LoginResult>,
}

#[derive(Deserialize)]
struct LoginResult {
    uid: String,
    uid_key: String,
}

#[derive(Serialize)]
struct ArkRequest<'a> {
    ark: String,
    object_id: &'a str,
}

#[derive(Deserialize)]
struct ArkResponse {
    result: Option<ArkResult>,
}

#[derive(Deserialize)]
struct ArkResult {
    ark_with_sig: Option<String>,
}

#[derive(Serialize)]
struct MusicArk<'a> {
    app: &'static str,
    config: MusicConfig,
    meta: MusicMeta<'a>,
    extra: MusicExtra,
    prompt: String,
    ver: &'static str,
    view: &'static str,
}

impl<'a> MusicArk<'a> {
    fn new(input: MusicInput<'a>, uin: u64, timestamp: u32) -> Self {
        Self {
            app: "com.tencent.tdoc.qqpush",
            config: MusicConfig {
                ctime: timestamp,
                kind: "normal",
                forward: 1,
            },
            meta: MusicMeta {
                music: MusicMetaBody {
                    app_type: 1,
                    ctime: timestamp,
                    description: input.description,
                    jump_url: input.jump_url,
                    music_url: input.music_url,
                    preview: input.preview,
                    source_message_id: "0",
                    source_icon: input.source_icon,
                    tag: input.tag,
                    title: input.title,
                    uin,
                },
            },
            extra: MusicExtra { app_id: 1, uin },
            prompt: format!("[分享]{}", input.title),
            ver: "0.0.0.1",
            view: "music",
        }
    }
}

#[derive(Serialize)]
struct MusicConfig {
    ctime: u32,
    #[serde(rename = "type")]
    kind: &'static str,
    forward: u32,
}

#[derive(Serialize)]
struct MusicMeta<'a> {
    music: MusicMetaBody<'a>,
}

#[derive(Serialize)]
struct MusicMetaBody<'a> {
    app_type: u32,
    ctime: u32,
    #[serde(rename = "desc")]
    description: &'a str,
    #[serde(rename = "jumpUrl")]
    jump_url: &'a str,
    #[serde(rename = "musicUrl")]
    music_url: &'a str,
    preview: &'a str,
    #[serde(rename = "sourceMsgId")]
    source_message_id: &'static str,
    source_icon: &'a str,
    tag: &'a str,
    title: &'a str,
    uin: u64,
}

#[derive(Serialize)]
struct MusicExtra {
    #[serde(rename = "appid")]
    app_id: u32,
    uin: u64,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{MusicArk, MusicInput};

    #[test]
    fn ark_matches_the_frozen_assigned_field_shape() -> Result<(), Box<dyn std::error::Error>> {
        let ark = serde_json::to_value(MusicArk::new(
            MusicInput {
                title: "title",
                description: "desc",
                jump_url: "https://example.invalid/jump",
                music_url: "https://example.invalid/music",
                source_icon: "https://example.invalid/icon",
                tag: "tag",
                preview: "https://example.invalid/preview",
            },
            42,
            100,
        ))?;
        assert_eq!(ark["app"], json!("com.tencent.tdoc.qqpush"));
        assert_eq!(
            ark["config"],
            json!({"ctime": 100, "type": "normal", "forward": 1})
        );
        assert_eq!(ark["extra"], json!({"appid": 1, "uin": 42}));
        assert_eq!(ark["meta"]["music"]["sourceMsgId"], json!("0"));
        assert!(ark["meta"]["music"].get("action").is_none());
        Ok(())
    }
}

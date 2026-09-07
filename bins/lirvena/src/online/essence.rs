use account_api::{AccountActionError, AccountActionRequest};
use account_message_store::RecallTarget;
use qq_control::{delete_group_essence, set_group_essence};
use reqwest::{Method, Url};
use serde::Deserialize;
use serde_json::{Value, json};

use super::controls::send_control;
use super::message_registry::MessageRegistry;
use super::packets::PacketRuntime;
use super::parameters::required_u32;
use super::push::PushRuntime;
use super::runtime::OnlineContext;
use super::ticket::{TicketAccess, TicketRuntime};

const DOMAIN: &str = "qun.qq.com";
const MAX_PAGES: u32 = 50;
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_MESSAGES_PER_PAGE: usize = 20;
const MAX_SEGMENTS_PER_MESSAGE: usize = 100;
const MAX_TEXT_BYTES: usize = 64 * 1024;
const MAX_URL_BYTES: usize = 4096;

pub(super) async fn list(
    request: &AccountActionRequest,
    uin: u64,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    tickets: &mut TicketRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let group_id = required_u32(request.params().get("group_id"))?;
    let csrf = tickets.csrf(uin, packets, pushes, context).await?;
    let mut output = Vec::new();
    for page in 0..MAX_PAGES {
        let response = request_page(
            group_id,
            page,
            csrf,
            tickets,
            TicketAccess::new(uin, packets, pushes, context),
        )
        .await?;
        let is_end = response.data.is_end;
        let messages = response.data.messages.unwrap_or_default();
        let page_is_empty = messages.is_empty();
        if messages.len() > MAX_MESSAGES_PER_PAGE {
            return Err(AccountActionError::QqFailure);
        }
        output.extend(
            messages
                .into_iter()
                .map(project_message)
                .collect::<Result<Vec<_>, _>>()?,
        );
        if is_end || page_is_empty {
            return Ok(Value::Array(output));
        }
    }
    Err(AccountActionError::QqFailure)
}

async fn request_page(
    group_id: u32,
    page: u32,
    csrf: i32,
    tickets: &mut TicketRuntime,
    access: TicketAccess<'_, '_>,
) -> Result<EssenceResponse, AccountActionError> {
    let mut url = Url::parse("https://qun.qq.com/cgi-bin/group_digest/digest_list")
        .map_err(|_error| AccountActionError::QqFailure)?;
    url.query_pairs_mut()
        .append_pair("random", "7800")
        .append_pair("X-CROSS-ORIGIN", "fetch")
        .append_pair("group_code", &group_id.to_string())
        .append_pair("page_start", &page.to_string())
        .append_pair("page_limit", "20")
        .append_pair("bkn", &csrf.to_string());
    let response = tickets
        .authenticated_request(Method::GET, url, DOMAIN, access)
        .await?
        .send()
        .await
        .map_err(|_error| AccountActionError::QqFailure)?;
    let body = TicketRuntime::bounded_body(response, MAX_RESPONSE_BYTES).await?;
    let response: EssenceResponse =
        serde_json::from_slice(&body).map_err(|_error| AccountActionError::QqFailure)?;
    if response.return_code != 0 {
        return Err(AccountActionError::QqFailure);
    }
    Ok(response)
}

fn project_message(message: EssenceMessage) -> Result<Value, AccountActionError> {
    let sender_id = parse_identifier(&message.sender_uin)?;
    let operator_id = parse_identifier(&message.operator_uin)?;
    validate_text(&message.sender_nickname, MAX_TEXT_BYTES)?;
    validate_text(&message.operator_nickname, MAX_TEXT_BYTES)?;
    if message.content.len() > MAX_SEGMENTS_PER_MESSAGE {
        return Err(AccountActionError::QqFailure);
    }
    let content = message
        .content
        .into_iter()
        .map(project_segment)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({
        "sender_id": sender_id,
        "sender_nick": message.sender_nickname,
        "sender_time": message.sender_time,
        "operator_id": operator_id,
        "operator_nick": message.operator_nickname,
        "operator_time": message.operator_time,
        "message_id": compatibility_message_id(message.random, message.sequence),
        "content": content,
    }))
}

fn project_segment(segment: EssenceSegment) -> Result<Value, AccountActionError> {
    match segment.kind {
        1 => {
            let text = segment.text.unwrap_or_default();
            validate_text(&text, MAX_TEXT_BYTES)?;
            Ok(json!({"type": "text", "data": {"text": text}}))
        }
        2 => {
            Ok(json!({"type": "face", "data": {"id": segment.face_index.unwrap_or(0).to_string()}}))
        }
        3 => {
            let url = required_url(segment.image_url)?;
            Ok(json!({"type": "image", "data": {
                "file": url, "filename": "", "url": url,
                "summary": "[图片]", "subType": 0
            }}))
        }
        4 => {
            let url = required_url(segment.file_thumbnail_url)?;
            Ok(json!({"type": "video", "data": {"file": url, "url": url}}))
        }
        _ => Err(AccountActionError::QqFailure),
    }
}

fn required_url(value: Option<String>) -> Result<String, AccountActionError> {
    let value = value.ok_or(AccountActionError::QqFailure)?;
    validate_text(&value, MAX_URL_BYTES)?;
    let url = Url::parse(&value).map_err(|_error| AccountActionError::QqFailure)?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(AccountActionError::QqFailure);
    }
    Ok(value)
}

fn parse_identifier(value: &str) -> Result<u32, AccountActionError> {
    if value.is_empty() || value.len() > 10 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(AccountActionError::QqFailure);
    }
    value
        .parse()
        .map_err(|_error| AccountActionError::QqFailure)
}

fn validate_text(value: &str, maximum: usize) -> Result<(), AccountActionError> {
    if value.len() > maximum || value.contains('\0') {
        Err(AccountActionError::QqFailure)
    } else {
        Ok(())
    }
}

const fn compatibility_message_id(random: u32, sequence: u32) -> i32 {
    let value = ((sequence & 0xffff) << 16) | (random & 0xffff);
    i32::from_ne_bytes(value.to_ne_bytes())
}

#[derive(Deserialize)]
struct EssenceResponse {
    #[serde(rename = "retcode")]
    return_code: i64,
    data: EssenceData,
}

#[derive(Deserialize)]
struct EssenceData {
    #[serde(rename = "msg_list")]
    messages: Option<Vec<EssenceMessage>>,
    is_end: bool,
}

#[derive(Deserialize)]
struct EssenceMessage {
    #[serde(rename = "msg_seq")]
    sequence: u32,
    #[serde(rename = "msg_random")]
    random: u32,
    sender_uin: String,
    #[serde(rename = "sender_nick")]
    sender_nickname: String,
    sender_time: u32,
    #[serde(rename = "add_digest_uin")]
    operator_uin: String,
    #[serde(rename = "add_digest_nick")]
    operator_nickname: String,
    #[serde(rename = "add_digest_time")]
    operator_time: u32,
    #[serde(rename = "msg_content")]
    content: Vec<EssenceSegment>,
}

#[derive(Deserialize)]
struct EssenceSegment {
    #[serde(rename = "msg_type")]
    kind: u32,
    text: Option<String>,
    face_index: Option<i32>,
    image_url: Option<String>,
    file_thumbnail_url: Option<String>,
}

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
    use serde_json::json;

    use super::{EssenceResponse, compatibility_message_id, essence_correlation, project_message};

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
                peer_uin: Some(42),
                sequence: 1,
                client_sequence: 2,
                random: 3,
                timestamp: 4,
            })
            .is_err()
        );
    }

    #[test]
    fn digest_projection_matches_frozen_onebot_shape() -> Result<(), Box<dyn std::error::Error>> {
        let source = br#"{
          "retcode": 0,
          "data": {"is_end": true, "msg_list": [{
            "msg_seq": 4660, "msg_random": 22136,
            "sender_uin": "42", "sender_nick": "sender", "sender_time": 10,
            "add_digest_uin": "43", "add_digest_nick": "operator", "add_digest_time": 11,
            "msg_content": [
              {"msg_type": 1, "text": "hello"},
              {"msg_type": 2, "face_index": 14},
              {"msg_type": 3, "image_url": "https://example.invalid/image"},
              {"msg_type": 4, "file_thumbnail_url": "https://example.invalid/video"}
            ]
          }]}
        }"#;
        let parsed: EssenceResponse = serde_json::from_slice(source)?;
        let mut messages = parsed
            .data
            .messages
            .ok_or_else(|| std::io::Error::other("fixture contains no messages"))?;
        let projected = project_message(messages.remove(0))?;
        assert_eq!(projected["sender_id"], 42);
        assert_eq!(projected["operator_id"], 43);
        assert_eq!(
            projected["content"][0],
            json!({"type":"text","data":{"text":"hello"}})
        );
        assert_eq!(projected["content"][1]["data"]["id"], "14");
        assert_eq!(projected["content"][2]["type"], "image");
        assert_eq!(projected["content"][3]["type"], "video");
        assert_eq!(
            projected["message_id"],
            compatibility_message_id(22136, 4660)
        );
        Ok(())
    }

    #[test]
    fn unknown_digest_segment_and_signed_hash_fail_or_match_honestly()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = br#"{
          "msg_seq": 1, "msg_random": 2,
          "sender_uin": "42", "sender_nick": "sender", "sender_time": 10,
          "add_digest_uin": "43", "add_digest_nick": "operator", "add_digest_time": 11,
          "msg_content": [{"msg_type": 99}]
        }"#;
        let message = serde_json::from_slice(source)?;
        assert!(project_message(message).is_err());
        assert!(compatibility_message_id(0xffff, 0xffff).is_negative());
        Ok(())
    }
}

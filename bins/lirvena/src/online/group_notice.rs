use account_api::{AccountActionError, AccountActionRequest};
use reqwest::{Method, Url};
use serde::Deserialize;
use serde_json::{Value, json};

use super::packets::PacketRuntime;
use super::parameters::required_u32;
use super::push::PushRuntime;
use super::runtime::OnlineContext;
use super::ticket::{TicketAccess, TicketRuntime};

const DOMAIN: &str = "qun.qq.com";
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_NOTICES: usize = 100;
const MAX_IMAGES: usize = 20;
const MAX_ID_BYTES: usize = 256;
const MAX_TEXT_BYTES: usize = 16 * 1024;

pub(super) async fn execute(
    request: &AccountActionRequest,
    uin: u64,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    tickets: &mut TicketRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    match request.action() {
        "_get_group_notice" => get(request, uin, packets, pushes, tickets, context).await,
        "_send_group_notice" => send(request, uin, packets, pushes, tickets, context).await,
        "_del_group_notice" => delete(request, uin, packets, pushes, tickets, context).await,
        _ => Err(AccountActionError::Unsupported),
    }
}

async fn get(
    request: &AccountActionRequest,
    uin: u64,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    tickets: &mut TicketRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let group_id = required_u32(request.params().get("group_id"))?;
    let csrf = tickets.csrf(uin, packets, pushes, context).await?;
    let mut url = endpoint("https://web.qun.qq.com/cgi-bin/announce/get_t_list")?;
    url.query_pairs_mut()
        .append_pair("bkn", &csrf.to_string())
        .append_pair("qid", &group_id.to_string())
        .append_pair("ft", "23")
        .append_pair("ni", "1")
        .append_pair("i", "1")
        .append_pair("log_read", "1")
        .append_pair("platform", "1")
        .append_pair("s", "-1")
        .append_pair("n", "20");
    let response = tickets
        .authenticated_request(
            Method::GET,
            url,
            DOMAIN,
            TicketAccess::new(uin, packets, pushes, context),
        )
        .await?
        .send()
        .await
        .map_err(|_error| AccountActionError::QqFailure)?;
    let body = TicketRuntime::bounded_body(response, MAX_RESPONSE_BYTES).await?;
    let response: NoticeList =
        serde_json::from_slice(&body).map_err(|_error| AccountActionError::QqFailure)?;
    project_notices(response)
}

async fn send(
    request: &AccountActionRequest,
    uin: u64,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    tickets: &mut TicketRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let group_id = required_u32(request.params().get("group_id"))?;
    let announcement_text = request
        .params()
        .get("content")
        .and_then(Value::as_str)
        .ok_or(AccountActionError::BadParameters)?;
    if announcement_text.len() > MAX_TEXT_BYTES || announcement_text.contains('\0') {
        return Err(AccountActionError::BadParameters);
    }
    if request
        .params()
        .get("image")
        .and_then(Value::as_str)
        .is_some_and(|value| !value.is_empty())
    {
        return Err(AccountActionError::Unsupported);
    }
    let csrf = tickets.csrf(uin, packets, pushes, context).await?;
    let mut url = endpoint("https://web.qun.qq.com/cgi-bin/announce/add_qun_notice")?;
    url.query_pairs_mut().append_pair("bkn", &csrf.to_string());
    let response = tickets
        .authenticated_request(
            Method::POST,
            url,
            DOMAIN,
            TicketAccess::new(uin, packets, pushes, context),
        )
        .await?
        .header(
            "user-agent",
            "Dalvik/2.1.0 (Linux; U; Android 7.1.2; PCRT00 Build/N2G48H)",
        )
        .form(&[
            ("qid", group_id.to_string()),
            ("bkn", csrf.to_string()),
            ("text", announcement_text.to_owned()),
            ("pinned", "0".to_owned()),
            ("type", "1".to_owned()),
            (
                "settings",
                r#"{"is_show_edit_card":0,"tip_window_type":1,"confirm_required":1}"#.to_owned(),
            ),
        ])
        .send()
        .await
        .map_err(|_error| AccountActionError::QqFailure)?;
    let body = TicketRuntime::bounded_body(response, 64 * 1024).await?;
    let response: NoticeCreated =
        serde_json::from_slice(&body).map_err(|_error| AccountActionError::QqFailure)?;
    validate_text(&response.notice_id, MAX_ID_BYTES)?;
    if response.notice_id.is_empty() {
        return Err(AccountActionError::QqFailure);
    }
    Ok(json!({"notice_id": response.notice_id}))
}

async fn delete(
    request: &AccountActionRequest,
    uin: u64,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    tickets: &mut TicketRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let group_id = required_u32(request.params().get("group_id"))?;
    let notice_id = request
        .params()
        .get("notice_id")
        .and_then(Value::as_str)
        .ok_or(AccountActionError::BadParameters)?;
    if notice_id.is_empty() || !is_bounded_text(notice_id, MAX_ID_BYTES) {
        return Err(AccountActionError::BadParameters);
    }
    let csrf = tickets.csrf(uin, packets, pushes, context).await?;
    let mut url = endpoint("https://web.qun.qq.com/cgi-bin/announce/del_feed")?;
    url.query_pairs_mut()
        .append_pair("fid", notice_id)
        .append_pair("qid", &group_id.to_string())
        .append_pair("bkn", &csrf.to_string())
        .append_pair("ft", "23")
        .append_pair("op", "1");
    let response = tickets
        .authenticated_request(
            Method::GET,
            url,
            DOMAIN,
            TicketAccess::new(uin, packets, pushes, context),
        )
        .await?
        .send()
        .await
        .map_err(|_error| AccountActionError::QqFailure)?;
    let _body = TicketRuntime::bounded_body(response, 64 * 1024).await?;
    Ok(json!({}))
}

fn project_notices(response: NoticeList) -> Result<Value, AccountActionError> {
    let count = response.feeds.len().saturating_add(response.pinned.len());
    if count > MAX_NOTICES {
        return Err(AccountActionError::QqFailure);
    }
    response
        .feeds
        .into_iter()
        .chain(response.pinned)
        .map(project_notice)
        .collect::<Result<Vec<_>, _>>()
        .map(Value::Array)
}

fn project_notice(notice: NoticeFeed) -> Result<Value, AccountActionError> {
    validate_text(&notice.notice_id, MAX_ID_BYTES)?;
    validate_text(&notice.message.text, MAX_TEXT_BYTES)?;
    if notice.notice_id.is_empty() || notice.message.images.len() > MAX_IMAGES {
        return Err(AccountActionError::QqFailure);
    }
    let images = notice
        .message
        .images
        .into_iter()
        .map(|image| {
            validate_text(&image.id, MAX_ID_BYTES)?;
            validate_dimension(&image.height)?;
            validate_dimension(&image.width)?;
            Ok(json!({"id": image.id, "height": image.height, "width": image.width}))
        })
        .collect::<Result<Vec<_>, AccountActionError>>()?;
    Ok(json!({
        "notice_id": notice.notice_id,
        "sender_id": notice.sender_id,
        "publish_time": notice.publish_time,
        "message": {"text": notice.message.text, "images": images}
    }))
}

fn endpoint(value: &str) -> Result<Url, AccountActionError> {
    Url::parse(value).map_err(|_error| AccountActionError::QqFailure)
}

fn validate_text(value: &str, maximum: usize) -> Result<(), AccountActionError> {
    if is_bounded_text(value, maximum) {
        Ok(())
    } else {
        Err(AccountActionError::QqFailure)
    }
}

fn is_bounded_text(value: &str, maximum: usize) -> bool {
    value.len() <= maximum && !value.contains('\0')
}

fn validate_dimension(value: &str) -> Result<(), AccountActionError> {
    if value.is_empty() || value.len() > 10 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        Err(AccountActionError::QqFailure)
    } else {
        Ok(())
    }
}

#[derive(Deserialize)]
struct NoticeList {
    #[serde(default)]
    feeds: Vec<NoticeFeed>,
    #[serde(default, rename = "inst")]
    pinned: Vec<NoticeFeed>,
}

#[derive(Deserialize)]
struct NoticeFeed {
    #[serde(rename = "fid")]
    notice_id: String,
    #[serde(rename = "u")]
    sender_id: u64,
    #[serde(rename = "pubt")]
    publish_time: i64,
    #[serde(rename = "msg")]
    message: NoticeMessage,
}

#[derive(Deserialize)]
struct NoticeMessage {
    #[serde(default)]
    text: String,
    #[serde(default, rename = "pics")]
    images: Vec<NoticeImage>,
}

#[derive(Deserialize)]
struct NoticeImage {
    id: String,
    #[serde(rename = "h")]
    height: String,
    #[serde(rename = "w")]
    width: String,
}

#[derive(Deserialize)]
struct NoticeCreated {
    #[serde(rename = "new_fid")]
    notice_id: String,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn notice_projection_preserves_compatibility_shape_and_bounds()
    -> Result<(), Box<dyn std::error::Error>> {
        let parsed: NoticeList = serde_json::from_value(json!({
            "feeds": [{"fid":"n1","u":42,"pubt":7,"msg":{"text":"hello","pics":[{"id":"p1","h":"10","w":"20"}]}}],
            "inst": []
        }))?;
        let projected = project_notices(parsed)?;
        assert_eq!(projected[0]["notice_id"], "n1");
        assert_eq!(projected[0]["message"]["images"][0]["width"], "20");
        Ok(())
    }

    #[test]
    fn malformed_dimensions_and_excessive_lists_fail_closed() {
        let notice = NoticeFeed {
            notice_id: "n1".to_owned(),
            sender_id: 42,
            publish_time: 7,
            message: NoticeMessage {
                text: "hello".to_owned(),
                images: vec![NoticeImage {
                    id: "p1".to_owned(),
                    height: "10px".to_owned(),
                    width: "20".to_owned(),
                }],
            },
        };
        assert!(project_notice(notice).is_err());
        assert!(
            project_notices(NoticeList {
                feeds: (0..=MAX_NOTICES)
                    .map(|_| NoticeFeed {
                        notice_id: "n".to_owned(),
                        sender_id: 1,
                        publish_time: 1,
                        message: NoticeMessage {
                            text: String::new(),
                            images: Vec::new()
                        },
                    })
                    .collect(),
                pinned: Vec::new(),
            })
            .is_err()
        );
    }
}

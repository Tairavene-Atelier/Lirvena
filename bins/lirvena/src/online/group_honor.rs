use account_api::{AccountActionError, AccountActionRequest};
use reqwest::{Method, Url};
use serde::Deserialize;
use serde_json::{Map, Value};

use super::packets::PacketRuntime;
use super::parameters::required_u32;
use super::push::PushRuntime;
use super::runtime::OnlineContext;
use super::ticket::{TicketAccess, TicketRuntime};

const DOMAIN: &str = "qun.qq.com";
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_MEMBERS: usize = 500;
const MAX_TEXT_BYTES: usize = 4096;
const STATE_MARKER: &str = "window.__INITIAL_STATE__";

const HONORS: [HonorKind; 5] = [
    HonorKind::new("talkative", "talkative_list", 1),
    HonorKind::new("performer", "performer_list", 2),
    HonorKind::new("legend", "legend_list", 3),
    HonorKind::new("strong_newbie", "strong_newbie_list", 5),
    HonorKind::new("emotion", "emotion_list", 6),
];

pub(super) async fn get(
    request: &AccountActionRequest,
    uin: u64,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    tickets: &mut TicketRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let group_id = required_u32(request.params().get("group_id"))?;
    let requested = request
        .params()
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("all");
    if requested != "all" && !HONORS.iter().any(|kind| kind.name == requested) {
        return Err(AccountActionError::BadParameters);
    }

    let mut result = empty_result(group_id);
    for kind in HONORS
        .iter()
        .filter(|kind| requested == "all" || requested == kind.name)
    {
        let state = request_honor(
            group_id,
            *kind,
            tickets,
            TicketAccess::new(uin, packets, pushes, context),
        )
        .await?;
        insert_honor(&mut result, *kind, state)?;
    }
    Ok(Value::Object(result))
}

fn empty_result(group_id: u32) -> Map<String, Value> {
    let mut result = Map::new();
    result.insert("group_id".to_owned(), Value::from(group_id));
    result.insert("current_talkative".to_owned(), Value::Object(Map::new()));
    for kind in HONORS {
        result.insert(kind.output_key.to_owned(), Value::Array(Vec::new()));
    }
    result
}

async fn request_honor(
    group_id: u32,
    kind: HonorKind,
    tickets: &mut TicketRuntime,
    access: TicketAccess<'_, '_>,
) -> Result<HonorState, AccountActionError> {
    let mut url = Url::parse("https://qun.qq.com/interactive/honorlist")
        .map_err(|_error| AccountActionError::QqFailure)?;
    url.query_pairs_mut()
        .append_pair("gc", &group_id.to_string())
        .append_pair("type", &kind.code.to_string());
    let response = tickets
        .authenticated_request(Method::GET, url, DOMAIN, access)
        .await?
        .send()
        .await
        .map_err(|_error| AccountActionError::QqFailure)?;
    let body = TicketRuntime::bounded_body(response, MAX_RESPONSE_BYTES).await?;
    let html = std::str::from_utf8(&body).map_err(|_error| AccountActionError::QqFailure)?;
    let state = extract_initial_state(html)?;
    serde_json::from_str(state).map_err(|_error| AccountActionError::QqFailure)
}

fn extract_initial_state(html: &str) -> Result<&str, AccountActionError> {
    let marker = html
        .find(STATE_MARKER)
        .ok_or(AccountActionError::QqFailure)?;
    let suffix = &html[marker + STATE_MARKER.len()..];
    let equals = suffix.find('=').ok_or(AccountActionError::QqFailure)?;
    let json = suffix[equals + 1..].trim_start();
    let bytes = json.as_bytes();
    if bytes.first() != Some(&b'{') {
        return Err(AccountActionError::QqFailure);
    }
    let mut depth = 0_u32;
    let mut in_string = false;
    let mut escaped = false;
    for (index, byte) in bytes.iter().copied().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => depth = depth.saturating_add(1),
            b'}' | b']' => {
                depth = depth.checked_sub(1).ok_or(AccountActionError::QqFailure)?;
                if depth == 0 {
                    return json.get(..=index).ok_or(AccountActionError::QqFailure);
                }
            }
            _ => {}
        }
    }
    Err(AccountActionError::QqFailure)
}

fn insert_honor(
    result: &mut Map<String, Value>,
    kind: HonorKind,
    state: HonorState,
) -> Result<(), AccountActionError> {
    let members = if kind.code == 1 {
        state.talkative_list
    } else {
        state.actor_list
    };
    if members.len() > MAX_MEMBERS {
        return Err(AccountActionError::QqFailure);
    }
    let projected = members
        .into_iter()
        .map(project_member)
        .collect::<Result<Vec<_>, _>>()?;
    if kind.code == 1 {
        let current = state
            .current_talkative
            .map(project_member)
            .transpose()?
            .or_else(|| projected.first().cloned())
            .unwrap_or_else(|| Value::Object(Map::new()));
        result.insert("current_talkative".to_owned(), current);
    }
    result.insert(kind.output_key.to_owned(), Value::Array(projected));
    Ok(())
}

fn project_member(member: HonorMember) -> Result<Value, AccountActionError> {
    let user_id = parse_identifier(member.uin)?;
    let nickname = member.name.or(member.nickname).unwrap_or_default();
    let description = member.description.unwrap_or_default();
    let avatar = member.avatar.unwrap_or_default();
    for value in [&nickname, &description, &avatar] {
        if value.len() > MAX_TEXT_BYTES || value.contains('\0') {
            return Err(AccountActionError::QqFailure);
        }
    }
    let mut output = Map::new();
    output.insert("user_id".to_owned(), Value::from(user_id));
    output.insert("nickname".to_owned(), Value::from(nickname));
    output.insert("avatar".to_owned(), Value::from(avatar));
    output.insert("description".to_owned(), Value::from(description));
    if let Some(day_count) = member.day_count {
        output.insert("day_count".to_owned(), Value::from(day_count));
    }
    Ok(Value::Object(output))
}

fn parse_identifier(value: Identifier) -> Result<u64, AccountActionError> {
    match value {
        Identifier::Number(value) => Ok(value),
        Identifier::Text(value)
            if !value.is_empty()
                && value.len() <= 20
                && value.bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            value
                .parse()
                .map_err(|_error| AccountActionError::QqFailure)
        }
        Identifier::Text(_) => Err(AccountActionError::QqFailure),
    }
}

#[derive(Clone, Copy)]
struct HonorKind {
    name: &'static str,
    output_key: &'static str,
    code: u8,
}

impl HonorKind {
    const fn new(name: &'static str, output_key: &'static str, code: u8) -> Self {
        Self {
            name,
            output_key,
            code,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HonorState {
    #[serde(default)]
    current_talkative: Option<HonorMember>,
    #[serde(default)]
    talkative_list: Vec<HonorMember>,
    #[serde(default)]
    actor_list: Vec<HonorMember>,
}

#[derive(Deserialize)]
struct HonorMember {
    uin: Identifier,
    #[serde(default)]
    name: Option<String>,
    #[serde(default, rename = "nick")]
    nickname: Option<String>,
    #[serde(default)]
    avatar: Option<String>,
    #[serde(default, rename = "desc")]
    description: Option<String>,
    #[serde(default, rename = "dayCount")]
    day_count: Option<u64>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Identifier {
    Number(u64),
    Text(String),
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{HonorKind, HonorState, empty_result, extract_initial_state, insert_honor};

    #[test]
    fn state_extractor_handles_semicolons_and_braces_inside_strings() {
        let html = r#"<script>window.__INITIAL_STATE__ = {"actorList":[{"uin":"42","name":"a; } b"}]};</script>"#;
        let extracted = extract_initial_state(html);
        assert!(matches!(
            extracted,
            Ok(value) if value == r#"{"actorList":[{"uin":"42","name":"a; } b"}]}"#
        ));
    }

    #[test]
    fn actor_list_projects_onebot_names_and_defaults() -> Result<(), Box<dyn std::error::Error>> {
        let state: HonorState = serde_json::from_value(json!({
            "actorList": [{"uin":"42", "name":"member", "avatar":"https://example.invalid/a", "desc":"active"}]
        }))?;
        let kind = HonorKind::new("performer", "performer_list", 2);
        let mut result = empty_result(123);
        insert_honor(&mut result, kind, state)?;
        assert_eq!(result["group_id"], 123);
        assert_eq!(result["performer_list"][0]["user_id"], 42);
        assert_eq!(result["performer_list"][0]["nickname"], "member");
        Ok(())
    }
}

use serde_json::{Map, Value, json};

use crate::{ActionRequest, BackendError};

pub(crate) fn expand_quick_operation(
    request: &ActionRequest,
) -> Result<Vec<ActionRequest>, BackendError> {
    let context = object(request.params().get("context"))?;
    let operation = object(request.params().get("operation"))?;
    let self_id = context
        .get("self_id")
        .cloned()
        .or_else(|| request.params().get("self_id").cloned());
    let mut actions = Vec::new();
    match context.get("post_type").and_then(Value::as_str) {
        Some("message") => expand_message(context, operation, self_id.as_ref(), &mut actions)?,
        Some("request") => expand_request(context, operation, self_id.as_ref(), &mut actions)?,
        Some(_) => {}
        None => return Err(bad_quick()),
    }
    actions
        .into_iter()
        .map(|value| ActionRequest::from_json(value).map_err(|_response| bad_quick()))
        .collect()
}

fn expand_message(
    context: &Map<String, Value>,
    operation: &Map<String, Value>,
    self_id: Option<&Value>,
    actions: &mut Vec<Value>,
) -> Result<(), BackendError> {
    let message_type = required_str(context, "message_type")?;
    if let Some(reply) = operation.get("reply") {
        let mut params = Map::new();
        params.insert("message".to_owned(), reply.clone());
        params.insert(
            "auto_escape".to_owned(),
            operation
                .get("auto_escape")
                .cloned()
                .unwrap_or(Value::Bool(false)),
        );
        let action = match message_type {
            "private" => {
                params.insert("user_id".to_owned(), required(context, "user_id")?.clone());
                "send_private_msg"
            }
            "group" => {
                params.insert(
                    "group_id".to_owned(),
                    required(context, "group_id")?.clone(),
                );
                if operation.get("at_sender").and_then(Value::as_bool) == Some(true) {
                    params.insert("at_sender".to_owned(), Value::Bool(true));
                    params.insert(
                        "sender_id".to_owned(),
                        required(context, "user_id")?.clone(),
                    );
                }
                "send_group_msg"
            }
            _ => return Err(bad_quick()),
        };
        actions.push(action_value(action, params, self_id));
    }
    if operation.get("delete").and_then(Value::as_bool) == Some(true) {
        actions.push(action_with(
            "delete_msg",
            "message_id",
            required(context, "message_id")?.clone(),
            self_id,
        ));
    }
    if message_type == "group" {
        if operation.get("kick").and_then(Value::as_bool) == Some(true) {
            actions.push(group_member_action("set_group_kick", context, self_id));
        }
        if operation.get("ban").and_then(Value::as_bool) == Some(true) {
            let mut params = group_member_params(context);
            params.insert(
                "duration".to_owned(),
                operation
                    .get("ban_duration")
                    .cloned()
                    .unwrap_or_else(|| json!(1800)),
            );
            actions.push(action_value("set_group_ban", params, self_id));
        }
    }
    Ok(())
}

fn expand_request(
    context: &Map<String, Value>,
    operation: &Map<String, Value>,
    self_id: Option<&Value>,
    actions: &mut Vec<Value>,
) -> Result<(), BackendError> {
    let Some(approve) = operation.get("approve") else {
        return Ok(());
    };
    let mut params = Map::new();
    params.insert("flag".to_owned(), required(context, "flag")?.clone());
    params.insert("approve".to_owned(), approve.clone());
    let action = match required_str(context, "request_type")? {
        "friend" => {
            if let Some(remark) = operation.get("remark") {
                params.insert("remark".to_owned(), remark.clone());
            }
            "set_friend_add_request"
        }
        "group" => {
            params.insert(
                "sub_type".to_owned(),
                required(context, "sub_type")?.clone(),
            );
            if let Some(reason) = operation.get("reason") {
                params.insert("reason".to_owned(), reason.clone());
            }
            "set_group_add_request"
        }
        _ => return Err(bad_quick()),
    };
    actions.push(action_value(action, params, self_id));
    Ok(())
}

fn action_with(action: &str, key: &str, value: Value, self_id: Option<&Value>) -> Value {
    action_value(action, Map::from_iter([(key.to_owned(), value)]), self_id)
}

fn group_member_action(
    action: &str,
    context: &Map<String, Value>,
    self_id: Option<&Value>,
) -> Value {
    action_value(action, group_member_params(context), self_id)
}

fn group_member_params(context: &Map<String, Value>) -> Map<String, Value> {
    let mut params = Map::new();
    if let Some(group_id) = context.get("group_id") {
        params.insert("group_id".to_owned(), group_id.clone());
    }
    if let Some(user_id) = context.get("user_id") {
        params.insert("user_id".to_owned(), user_id.clone());
    }
    params
}

fn action_value(action: &str, params: Map<String, Value>, self_id: Option<&Value>) -> Value {
    let mut object = Map::new();
    object.insert("action".to_owned(), Value::String(action.to_owned()));
    object.insert("params".to_owned(), Value::Object(params));
    if let Some(self_id) = self_id {
        object.insert("self_id".to_owned(), self_id.clone());
    }
    Value::Object(object)
}

fn object(value: Option<&Value>) -> Result<&Map<String, Value>, BackendError> {
    value.and_then(Value::as_object).ok_or_else(bad_quick)
}

fn required<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a Value, BackendError> {
    object.get(key).ok_or_else(bad_quick)
}

fn required_str<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, BackendError> {
    required(object, key)?.as_str().ok_or_else(bad_quick)
}

fn bad_quick() -> BackendError {
    BackendError::BadParameters("quick operation is invalid".to_owned())
}

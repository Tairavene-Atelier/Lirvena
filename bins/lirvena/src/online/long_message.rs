use std::collections::BTreeMap;

use account_api::{AccountActionError, AccountActionRequest, AccountIdentity};
use account_message_store::QuoteTarget;
use adapter_onebot::{ForwardNode, parse_forward_nodes, project_forward_node};
use qq_directory::FriendEntry;
use qq_message::{
    ForwardEntryInput, LongMessageTarget, MessageDecoder, MessageDisposition, SendTextTarget,
    decode_rich_text, encode_forward_entry, encode_long_message_receive, encode_long_message_send,
    parse_long_message_receive, parse_long_message_send,
};
use serde_json::{Value, json};

use crate::opaque::{OpaqueOperation, request_reserve};
use crate::support::{encode_hex, now_seconds, random_array, random_nonzero_u32};

use super::actions::{
    ActionResources, CompiledSegment, compile_segments, resolve_private_uid, send_segments,
};
use super::media::MediaRuntime;
use super::message_registry::MessageRegistry;
use super::packets::{PacketContext, PacketRuntime};
use super::parameters::required_u32;
use super::push::PushRuntime;
use super::runtime::OnlineContext;

const RECEIVE_ROUTE: &str = "trpc.group.long_msg_interface.MsgService.SsoRecvLongMsg";
const SEND_ROUTE: &str = "trpc.group.long_msg_interface.MsgService.SsoSendLongMsg";

pub(super) async fn get_forward_message(
    request: &AccountActionRequest,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let resource_id = request
        .params()
        .get("id")
        .and_then(Value::as_str)
        .ok_or(AccountActionError::BadParameters)?;
    let body = encode_long_message_receive(context.credential.uid(), resource_id)
        .map_err(|_error| AccountActionError::BadParameters)?;
    let reserve = request_reserve(
        context.ceylith,
        context.account_slot_id,
        OpaqueOperation::numeric(9),
        &body,
    )
    .await
    .map_err(|_error| AccountActionError::QqFailure)?;
    let response = packets
        .send_with_reserve(
            PacketContext::for_account(context, pushes.plan()),
            RECEIVE_ROUTE,
            &reserve,
            &body,
        )
        .await
        .map_err(|_error| AccountActionError::QqFailure)?;
    let messages =
        parse_long_message_receive(&response).map_err(|_error| AccountActionError::QqFailure)?;
    let nodes = messages
        .iter()
        .map(|message| project_node(message))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({"message": nodes}))
}

pub(super) async fn send_group_forward_message(
    request: &AccountActionRequest,
    identity: &AccountIdentity,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    resources: &mut ActionResources<'_>,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let group_code = required_u32(request.params().get("group_id"))?;
    let forward_resources = ForwardResources {
        identity,
        packets,
        pushes,
        messages: resources.messages,
        media: resources.media,
    };
    send_forward_message(
        request,
        SendTextTarget::Group { group_code },
        LongMessageTarget::Group {
            group_uin: group_code,
        },
        forward_resources,
        context,
    )
    .await
}

pub(super) async fn send_private_forward_message(
    request: &AccountActionRequest,
    identity: &AccountIdentity,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    friends: &mut BTreeMap<u32, FriendEntry>,
    resources: &mut ActionResources<'_>,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let uin = required_u32(request.params().get("user_id"))?;
    let uid = resolve_private_uid(uin, packets, pushes, friends, context).await?;
    let forward_resources = ForwardResources {
        identity,
        packets,
        pushes,
        messages: resources.messages,
        media: resources.media,
    };
    send_forward_message(
        request,
        SendTextTarget::Private { uin, uid: &uid },
        LongMessageTarget::Private { peer_uid: &uid },
        forward_resources,
        context,
    )
    .await
}

pub(super) async fn upload_forward_message(
    request: &AccountActionRequest,
    identity: &AccountIdentity,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    resources: &mut ActionResources<'_>,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let current_uid = context.credential.uid().to_owned();
    let mut forward_resources = ForwardResources {
        identity,
        packets,
        pushes,
        messages: resources.messages,
        media: resources.media,
    };
    let uploaded = upload_forward(
        request,
        &LongMessageTarget::Private {
            peer_uid: &current_uid,
        },
        &mut forward_resources,
        context,
    )
    .await?;
    Ok(Value::String(uploaded.resource_id))
}

struct ForwardResources<'a> {
    identity: &'a AccountIdentity,
    packets: &'a PacketRuntime,
    pushes: &'a PushRuntime,
    messages: &'a mut MessageRegistry,
    media: &'a mut MediaRuntime,
}

async fn send_forward_message(
    request: &AccountActionRequest,
    target: SendTextTarget<'_>,
    long_target: LongMessageTarget<'_>,
    mut resources: ForwardResources<'_>,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let uploaded = upload_forward(request, &long_target, &mut resources, context).await?;
    let card = forward_card(&uploaded.resource_id, uploaded.total, &uploaded.previews)?;
    let mut sent = send_segments(
        target,
        &[CompiledSegment::Forward {
            resource_id: uploaded.resource_id.clone(),
            card,
        }],
        resources.identity,
        resources.packets,
        resources.pushes,
        resources.messages,
        context,
    )
    .await?;
    sent.as_object_mut()
        .ok_or(AccountActionError::QqFailure)?
        .insert("forward_id".to_owned(), Value::String(uploaded.resource_id));
    Ok(sent)
}

struct UploadedForward {
    resource_id: String,
    total: usize,
    previews: Vec<String>,
}

async fn upload_forward(
    request: &AccountActionRequest,
    long_target: &LongMessageTarget<'_>,
    resources: &mut ForwardResources<'_>,
    context: &mut OnlineContext<'_>,
) -> Result<UploadedForward, AccountActionError> {
    let nodes = parse_forward_nodes(
        request
            .params()
            .get("messages")
            .ok_or(AccountActionError::BadParameters)?,
    )
    .map_err(|_error| AccountActionError::BadParameters)?;
    let current_account =
        u32::try_from(context.uin).map_err(|_error| AccountActionError::QqFailure)?;
    let current_uid = context.credential.uid().to_owned();
    let self_target = SendTextTarget::Private {
        uin: current_account,
        uid: &current_uid,
    };
    let unix_seconds = now_seconds().map_err(|_error| AccountActionError::QqFailure)?;
    let mut encoded = Vec::with_capacity(nodes.len());
    let mut previews = Vec::with_capacity(nodes.len().min(4));
    for node in &nodes {
        let compiled = compile_forward_node(
            node,
            &self_target,
            resources.packets,
            resources.pushes,
            resources.media,
            context,
        )
        .await?;
        if previews.len() < 4 {
            previews.push(preview(node, &compiled));
        }
        let outbound = compiled
            .iter()
            .map(CompiledSegment::borrowed)
            .collect::<Vec<_>>();
        encoded.push(
            encode_forward_entry(&ForwardEntryInput {
                sender_uin: node.user_id(),
                sender_name: node.nickname(),
                self_uid: &current_uid,
                segments: &outbound,
                random: random_nonzero_u32().map_err(|_error| AccountActionError::QqFailure)?,
                sequence: synthetic_sequence()?,
                unix_seconds,
            })
            .map_err(|_error| AccountActionError::BadParameters)?,
        );
    }
    let upload = encode_long_message_send(long_target, &encoded)
        .map_err(|_error| AccountActionError::BadParameters)?;
    let reserve = request_reserve(
        context.ceylith,
        context.account_slot_id,
        OpaqueOperation::numeric(10),
        &upload,
    )
    .await
    .map_err(|_error| AccountActionError::QqFailure)?;
    let response = resources
        .packets
        .send_with_reserve(
            PacketContext::for_account(context, resources.pushes.plan()),
            SEND_ROUTE,
            &reserve,
            &upload,
        )
        .await
        .map_err(|_error| AccountActionError::QqFailure)?;
    let resource_id =
        parse_long_message_send(&response).map_err(|_error| AccountActionError::QqFailure)?;
    Ok(UploadedForward {
        resource_id,
        total: nodes.len(),
        previews,
    })
}

async fn compile_forward_node(
    node: &ForwardNode,
    target: &SendTextTarget<'_>,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    media: &mut MediaRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Vec<CompiledSegment>, AccountActionError> {
    compile_segments(
        node.content(),
        target,
        &BTreeMap::<u32, (bool, QuoteTarget)>::new(),
        packets,
        pushes,
        media,
        context,
    )
    .await
}

fn synthetic_sequence() -> Result<u32, AccountActionError> {
    random_nonzero_u32()
        .map(|value| value % 9_000_000 + 1_000_000)
        .map_err(|_error| AccountActionError::QqFailure)
}

fn preview(node: &ForwardNode, segments: &[CompiledSegment]) -> String {
    let content = segments
        .iter()
        .map(CompiledSegment::preview_text)
        .collect::<Vec<_>>()
        .join(" ");
    format!("{}: {content}", node.nickname())
}

fn forward_card(
    resource_id: &str,
    total: usize,
    previews: &[String],
) -> Result<String, AccountActionError> {
    let identifier =
        encode_hex(&random_array::<16>().map_err(|_error| AccountActionError::QqFailure)?);
    serde_json::to_string(&json!({
        "app": "com.tencent.multimsg",
        "config": {"autosize": 1, "forward": 1, "round": 1, "type": "normal", "width": 300},
        "desc": "[聊天记录]",
        "extra": serde_json::to_string(&json!({"filename": identifier, "tsum": previews.len()}))
            .map_err(|_error| AccountActionError::QqFailure)?,
        "meta": {"detail": {
            "news": previews.iter().map(|text| json!({"text": text})).collect::<Vec<_>>(),
            "resid": resource_id,
            "source": "聊天记录",
            "summary": format!("查看{total}条转发消息"),
            "uniseq": identifier
        }},
        "prompt": "[聊天记录]",
        "ver": "0.0.0.5",
        "view": "contact"
    }))
    .map_err(|_error| AccountActionError::QqFailure)
}

fn project_node(input: &[u8]) -> Result<Value, AccountActionError> {
    let mut decoder = MessageDecoder::default();
    let MessageDisposition::New(envelope) = decoder
        .decode_embedded(input)
        .map_err(|_error| AccountActionError::QqFailure)?
    else {
        return Err(AccountActionError::QqFailure);
    };
    let rich_text = envelope
        .payload()
        .rich_text()
        .map(decode_rich_text)
        .transpose()
        .map_err(|_error| AccountActionError::QqFailure)?
        .filter(|rich| !rich.elements().is_empty())
        .ok_or(AccountActionError::QqFailure)?;
    project_forward_node(&envelope, Some(&rich_text))
        .map_err(|_error| AccountActionError::QqFailure)
}

#[cfg(test)]
mod tests {
    use prost::Message;
    use serde_json::{Value, json};

    use super::{forward_card, project_node, synthetic_sequence};

    #[test]
    fn long_message_entry_projects_as_onebot_node() -> Result<(), Box<dyn std::error::Error>> {
        let input = CommonMessageFixture {
            response: Some(ResponseFixture {
                from_uin: 42,
                friend: Some(FriendFixture {
                    name: Some("sender".to_owned()),
                }),
            }),
            content: Some(ContentFixture {
                message_type: 166,
                timestamp: Some(10),
            }),
            body: Some(BodyFixture {
                rich_text: Some(RichFixture {
                    elements: vec![ElementFixture {
                        text: Some(TextFixture {
                            value: Some("hello".to_owned()),
                        }),
                    }],
                }),
            }),
        }
        .encode_to_vec();
        assert_eq!(
            project_node(&input)?,
            json!({
                "type": "node",
                "data": {
                    "user_id": "42",
                    "nickname": "sender",
                    "content": [{"type": "text", "data": {"text": "hello"}}]
                }
            })
        );
        let empty = CommonMessageFixture {
            response: Some(ResponseFixture {
                from_uin: 42,
                friend: None,
            }),
            content: None,
            body: None,
        }
        .encode_to_vec();
        assert!(project_node(&empty).is_err());
        Ok(())
    }

    #[test]
    fn forward_card_has_bounded_preview_and_opaque_resource()
    -> Result<(), Box<dyn std::error::Error>> {
        let card = forward_card(
            "resource-1",
            7,
            &["Alice: hello".to_owned(), "Bob: [图片]".to_owned()],
        )?;
        let value: Value = serde_json::from_str(&card)?;
        assert_eq!(value["app"], "com.tencent.multimsg");
        assert_eq!(value["meta"]["detail"]["resid"], "resource-1");
        assert_eq!(value["meta"]["detail"]["summary"], "查看7条转发消息");
        assert_eq!(
            value["meta"]["detail"]["news"].as_array().map(Vec::len),
            Some(2)
        );
        let extra: Value = serde_json::from_str(value["extra"].as_str().ok_or("missing extra")?)?;
        assert_eq!(extra["tsum"], 2);
        let sequence = synthetic_sequence()?;
        assert!((1_000_000..10_000_000).contains(&sequence));
        Ok(())
    }

    #[derive(Clone, PartialEq, Message)]
    struct CommonMessageFixture {
        #[prost(message, optional, tag = "1")]
        response: Option<ResponseFixture>,
        #[prost(message, optional, tag = "2")]
        content: Option<ContentFixture>,
        #[prost(message, optional, tag = "3")]
        body: Option<BodyFixture>,
    }

    #[derive(Clone, PartialEq, Message)]
    struct ResponseFixture {
        #[prost(uint32, tag = "1")]
        from_uin: u32,
        #[prost(message, optional, tag = "7")]
        friend: Option<FriendFixture>,
    }

    #[derive(Clone, PartialEq, Message)]
    struct FriendFixture {
        #[prost(string, optional, tag = "6")]
        name: Option<String>,
    }

    #[derive(Clone, Copy, PartialEq, Message)]
    struct ContentFixture {
        #[prost(uint32, tag = "1")]
        message_type: u32,
        #[prost(int64, optional, tag = "6")]
        timestamp: Option<i64>,
    }

    #[derive(Clone, PartialEq, Message)]
    struct BodyFixture {
        #[prost(message, optional, tag = "1")]
        rich_text: Option<RichFixture>,
    }

    #[derive(Clone, PartialEq, Message)]
    struct RichFixture {
        #[prost(message, repeated, tag = "2")]
        elements: Vec<ElementFixture>,
    }

    #[derive(Clone, PartialEq, Message)]
    struct ElementFixture {
        #[prost(message, optional, tag = "1")]
        text: Option<TextFixture>,
    }

    #[derive(Clone, PartialEq, Message)]
    struct TextFixture {
        #[prost(string, optional, tag = "1")]
        value: Option<String>,
    }
}

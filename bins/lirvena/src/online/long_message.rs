use account_api::{AccountActionError, AccountActionRequest};
use adapter_onebot::project_forward_node;
use qq_message::{
    MessageDecoder, MessageDisposition, decode_rich_text, encode_long_message_receive,
    parse_long_message_receive,
};
use serde_json::{Value, json};

use crate::opaque::{OpaqueOperation, request_reserve};

use super::packets::{PacketContext, PacketRuntime};
use super::push::PushRuntime;
use super::runtime::OnlineContext;

const RECEIVE_ROUTE: &str = "trpc.group.long_msg_interface.MsgService.SsoRecvLongMsg";

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
    use serde_json::json;

    use super::project_node;

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

//! Authenticated group-system notice codec contracts.

use prost::Message;
use qq_message::{
    GroupNotice, MemberDecreaseKind, MemberIncreaseKind, MessageDecoder, MessageDisposition,
    decode_group_notice,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Clone, PartialEq, Message)]
struct PushBody {
    #[prost(message, optional, tag = "1")]
    response: Option<Response>,
    #[prost(message, optional, tag = "2")]
    content: Option<Content>,
    #[prost(message, optional, tag = "3")]
    body: Option<Body>,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct Response {
    #[prost(uint32, tag = "1")]
    from_uin: u32,
    #[prost(uint32, tag = "5")]
    to_uin: u32,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct Content {
    #[prost(uint32, tag = "1")]
    message_type: u32,
    #[prost(uint64, optional, tag = "5")]
    sequence: Option<u64>,
}

#[derive(Clone, PartialEq, Message)]
struct Body {
    #[prost(bytes = "vec", optional, tag = "2")]
    content: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, Message)]
struct Change {
    #[prost(uint32, tag = "1")]
    group_id: u32,
    #[prost(string, tag = "3")]
    member_uid: String,
    #[prost(uint32, tag = "4")]
    primary_type: u32,
    #[prost(bytes = "vec", optional, tag = "5")]
    operator: Option<Vec<u8>>,
    #[prost(uint32, tag = "6")]
    secondary_type: u32,
}

#[derive(Clone, PartialEq, Message)]
struct Administrator {
    #[prost(uint32, tag = "1")]
    group_id: u32,
    #[prost(message, optional, tag = "4")]
    body: Option<AdministratorBody>,
}

#[derive(Clone, PartialEq, Message)]
struct AdministratorBody {
    #[prost(message, optional, tag = "1")]
    disable: Option<AdministratorMember>,
    #[prost(message, optional, tag = "2")]
    enable: Option<AdministratorMember>,
}

#[derive(Clone, PartialEq, Message)]
struct AdministratorMember {
    #[prost(string, tag = "1")]
    uid: String,
}

#[test]
fn administrator_and_increase_keep_uid_evidence() -> TestResult {
    let administrator = envelope(
        44,
        Administrator {
            group_id: 88,
            body: Some(AdministratorBody {
                disable: None,
                enable: Some(AdministratorMember {
                    uid: "u_admin".to_owned(),
                }),
            }),
        }
        .encode_to_vec(),
    )?;
    assert_eq!(
        decode_group_notice(&administrator)?,
        Some(GroupNotice::Administrator {
            group_id: 88,
            member_uid: "u_admin".to_owned(),
            enabled: true,
        })
    );

    let increase = envelope(
        33,
        Change {
            group_id: 88,
            member_uid: "u_member".to_owned(),
            primary_type: 131,
            operator: Some(b"u_inviter".to_vec()),
            secondary_type: 0,
        }
        .encode_to_vec(),
    )?;
    assert_eq!(
        decode_group_notice(&increase)?,
        Some(GroupNotice::MemberIncrease {
            group_id: 88,
            member_uid: "u_member".to_owned(),
            operator_uid: Some("u_inviter".to_owned()),
            kind: MemberIncreaseKind::Invite,
        })
    );
    Ok(())
}

#[test]
fn decrease_subtypes_and_nested_operator_are_strict() -> TestResult {
    let nested_operator = [0x0a, 0x0c, 0x0a, 0x0a]
        .into_iter()
        .chain(b"u_operator".iter().copied())
        .collect();
    let decrease = envelope(
        34,
        Change {
            group_id: 88,
            member_uid: "u_self".to_owned(),
            primary_type: 3,
            operator: Some(nested_operator),
            secondary_type: 0,
        }
        .encode_to_vec(),
    )?;
    assert_eq!(
        decode_group_notice(&decrease)?,
        Some(GroupNotice::MemberDecrease {
            group_id: 88,
            member_uid: "u_self".to_owned(),
            operator_uid: Some("u_operator".to_owned()),
            kind: MemberDecreaseKind::KickMe,
        })
    );

    let unknown = envelope(
        34,
        Change {
            group_id: 88,
            member_uid: "u_member".to_owned(),
            primary_type: 999,
            operator: None,
            secondary_type: 0,
        }
        .encode_to_vec(),
    )?;
    assert_eq!(
        decode_group_notice(&unknown)?,
        Some(GroupNotice::MemberDecrease {
            group_id: 88,
            member_uid: "u_member".to_owned(),
            operator_uid: None,
            kind: MemberDecreaseKind::Unknown(999),
        })
    );
    Ok(())
}

fn envelope(
    message_type: u32,
    notice: Vec<u8>,
) -> Result<qq_message::MessageEnvelope, Box<dyn std::error::Error>> {
    let body = PushBody {
        response: Some(Response {
            from_uin: 1,
            to_uin: 2,
        }),
        content: Some(Content {
            message_type,
            sequence: Some(7),
        }),
        body: Some(Body {
            content: Some(notice),
        }),
    };
    let mut decoder = MessageDecoder::default();
    let MessageDisposition::New(envelope) = decoder.decode_embedded(&body.encode_to_vec())? else {
        return Err("expected new notice".into());
    };
    Ok(*envelope)
}

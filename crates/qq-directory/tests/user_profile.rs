//! Regression vectors for bounded public QQ profile queries.

use prost::Message;
use qq_directory::{UserGender, UserProfile, encode_user_profile_request, parse_user_profile};

const NICKNAME: u32 = 20_002;
const GENDER: u32 = 20_009;
const AGE: u32 = 20_037;
const QID: u32 = 27_394;
const LEVEL: u32 = 105;
const AVATAR: u32 = 101;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn request_uses_numeric_identity_and_the_bounded_public_field_set() -> TestResult {
    let encoded = encode_user_profile_request(42)?;
    let outer = qq_wire::decode_oidb_request(&encoded)?;
    assert_eq!((outer.command(), outer.subcommand()), (0x0fe1, 2));
    assert_eq!(outer.reserved(), 1);
    let request = TestRequest::decode(outer.body())?;
    assert_eq!(request.uin, 42);
    assert_eq!(
        request.keys.iter().map(|key| key.key).collect::<Vec<_>>(),
        [NICKNAME, GENDER, AGE, QID, 102, LEVEL, 20_026, AVATAR]
    );
    assert!(encode_user_profile_request(0).is_err());
    Ok(())
}

#[test]
fn response_projects_only_bounded_public_values() -> TestResult {
    let avatar = TestAvatar {
        url: "https://avatar.invalid/spec=".to_owned(),
    }
    .encode_to_vec();
    let response = response(
        42,
        vec![
            TestNumber {
                key: GENDER,
                value: 1,
            },
            TestNumber {
                key: AGE,
                value: 24,
            },
            TestNumber {
                key: LEVEL,
                value: 64,
            },
        ],
        vec![
            TestBytes {
                key: NICKNAME,
                value: b"tester".to_vec(),
            },
            TestBytes {
                key: QID,
                value: b"public-id".to_vec(),
            },
            TestBytes {
                key: AVATAR,
                value: avatar,
            },
        ],
    )?;
    assert_eq!(
        parse_user_profile(&response)?,
        UserProfile {
            uin: 42,
            nickname: "tester".to_owned(),
            gender: UserGender::Male,
            age: 24,
            qid: Some("public-id".to_owned()),
            signature: None,
            level: 64,
            registered_at: 0,
            avatar_url: Some("https://avatar.invalid/spec=640".to_owned()),
        }
    );
    Ok(())
}

#[test]
fn duplicate_or_excessive_properties_fail_closed() -> TestResult {
    let duplicate = response(
        42,
        vec![],
        vec![
            TestBytes {
                key: NICKNAME,
                value: b"first".to_vec(),
            },
            TestBytes {
                key: NICKNAME,
                value: b"second".to_vec(),
            },
        ],
    )?;
    assert!(parse_user_profile(&duplicate).is_err());
    let excessive_age = response(
        42,
        vec![TestNumber {
            key: AGE,
            value: 201,
        }],
        vec![TestBytes {
            key: NICKNAME,
            value: b"tester".to_vec(),
        }],
    )?;
    assert!(parse_user_profile(&excessive_age).is_err());
    Ok(())
}

fn response(uin: u32, numbers: Vec<TestNumber>, bytes: Vec<TestBytes>) -> TestResultValue {
    let body = TestResponse {
        user: Some(TestUser {
            properties: Some(TestProperties { numbers, bytes }),
            uin,
        }),
    }
    .encode_to_vec();
    Ok(qq_wire::encode_oidb_request(0x0fe1, 2, &body, 0)?)
}

type TestResultValue = Result<Vec<u8>, Box<dyn std::error::Error>>;

#[derive(Clone, PartialEq, Message)]
struct TestRequest {
    #[prost(uint32, tag = "1")]
    uin: u32,
    #[prost(message, repeated, tag = "3")]
    keys: Vec<TestKey>,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct TestKey {
    #[prost(uint32, tag = "1")]
    key: u32,
}

#[derive(Clone, PartialEq, Message)]
struct TestResponse {
    #[prost(message, optional, tag = "1")]
    user: Option<TestUser>,
}

#[derive(Clone, PartialEq, Message)]
struct TestUser {
    #[prost(message, optional, tag = "2")]
    properties: Option<TestProperties>,
    #[prost(uint32, tag = "3")]
    uin: u32,
}

#[derive(Clone, PartialEq, Message)]
struct TestProperties {
    #[prost(message, repeated, tag = "1")]
    numbers: Vec<TestNumber>,
    #[prost(message, repeated, tag = "2")]
    bytes: Vec<TestBytes>,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct TestNumber {
    #[prost(uint32, tag = "1")]
    key: u32,
    #[prost(uint32, tag = "2")]
    value: u32,
}

#[derive(Clone, PartialEq, Message)]
struct TestBytes {
    #[prost(uint32, tag = "1")]
    key: u32,
    #[prost(bytes = "vec", tag = "2")]
    value: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
struct TestAvatar {
    #[prost(string, tag = "5")]
    url: String,
}

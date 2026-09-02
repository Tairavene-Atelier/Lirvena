use account_api::AccountActionError;
use qq_directory::{UserGender, UserProfile, encode_user_profile_request, parse_user_profile};
use serde_json::{Value, json};

use super::packets::{PacketContext, PacketRuntime};
use super::parameters::{optional_bool, required_u32};
use super::push::PushRuntime;
use super::runtime::OnlineContext;

pub(super) async fn stranger_info(
    user_id: Option<&Value>,
    no_cache: Option<&Value>,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let user_id = required_u32(user_id)?;
    let _no_cache = optional_bool(no_cache, false)?;
    let body =
        encode_user_profile_request(user_id).map_err(|_error| AccountActionError::QqFailure)?;
    let response = packets
        .send_with_reserve(
            PacketContext::for_account(context, pushes.plan()),
            "OidbSvcTrpcTcp.0xfe1_2",
            &[],
            &body,
        )
        .await
        .map_err(|_error| AccountActionError::QqFailure)?;
    let profile = parse_user_profile(&response).map_err(|_error| AccountActionError::QqFailure)?;
    project(profile, user_id)
}

fn project(profile: UserProfile, expected_uin: u32) -> Result<Value, AccountActionError> {
    if profile.uin != expected_uin {
        return Err(AccountActionError::QqFailure);
    }
    let sex = match profile.gender {
        UserGender::Male => "male",
        UserGender::Female => "female",
        UserGender::Unknown => "unknown",
    };
    let mut value = json!({
        "user_id": profile.uin,
        "nickname": profile.nickname,
        "sex": sex,
        "age": profile.age,
        "level": profile.level,
        "register_time": profile.registered_at,
    });
    let object = value.as_object_mut().ok_or(AccountActionError::QqFailure)?;
    if let Some(qid) = profile.qid {
        object.insert("qid".to_owned(), Value::String(qid));
    }
    if let Some(signature) = profile.signature {
        object.insert("sign".to_owned(), Value::String(signature));
    }
    if let Some(avatar) = profile.avatar_url {
        object.insert("avatar".to_owned(), Value::String(avatar));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use account_api::AccountActionError;
    use qq_directory::{UserGender, UserProfile};
    use serde_json::json;

    use super::project;

    #[test]
    fn projection_binds_identity_and_omits_unobserved_extensions() {
        let profile = UserProfile {
            uin: 42,
            nickname: "tester".to_owned(),
            gender: UserGender::Unknown,
            age: 0,
            qid: None,
            signature: None,
            level: 0,
            registered_at: 0,
            avatar_url: None,
        };
        assert_eq!(
            project(profile.clone(), 42),
            Ok(json!({
                "user_id": 42,
                "nickname": "tester",
                "sex": "unknown",
                "age": 0,
                "level": 0,
                "register_time": 0,
            }))
        );
        assert!(matches!(
            project(profile, 43),
            Err(AccountActionError::QqFailure)
        ));
    }
}

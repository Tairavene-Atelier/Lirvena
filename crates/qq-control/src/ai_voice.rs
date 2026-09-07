use prost::Message;
use qq_wire::decode_oidb_response;

use crate::{ControlError, ControlRequest, request};

const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_CATEGORIES: usize = 32;
const MAX_CHARACTERS_PER_CATEGORY: usize = 256;
const MAX_TEXT_BYTES: usize = 1024;
const MAX_MESSAGE_INFO_BYTES: usize = 1024 * 1024;

/// One QQ-provided AI voice character.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AiVoiceCharacter {
    /// QQ voice identity used for synthesis.
    pub character_id: String,
    /// Display name returned by QQ.
    pub character_name: String,
    /// QQ preview media URL.
    pub preview_url: String,
}

/// One QQ-provided AI voice category.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AiVoiceCategory {
    /// QQ category discriminator.
    pub kind: String,
    /// Characters belonging to this category.
    pub characters: Vec<AiVoiceCharacter>,
}

/// Current outcome of one QQ AI voice generation poll.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AiVoiceGeneration {
    /// QQ is still generating the media.
    Pending,
    /// QQ returned complete message material ready for download or group sending.
    Ready(Vec<u8>),
}

/// Encodes `get_ai_characters` for voice or song synthesis.
///
/// # Errors
///
/// Returns an error for a zero group identifier or an unsupported chat type.
pub fn ai_voice_characters(group_id: u32, chat_type: u32) -> Result<ControlRequest, ControlError> {
    if group_id == 0 || !matches!(chat_type, 1 | 2) {
        return Err(ControlError);
    }
    request(
        0x929d,
        0,
        "OidbSvcTrpcTcp.0x929d_0",
        None,
        &AiVoiceCharactersRequest {
            group_id,
            chat_type,
        },
    )
}

/// Encodes one stable poll for AI voice generation.
///
/// The caller must reuse the same non-zero `request_random` for every poll in one generation.
///
/// # Errors
///
/// Returns an error for invalid identifiers, chat type, or text fields.
pub fn generate_ai_voice(
    group_id: u32,
    character_id: &str,
    text: &str,
    chat_type: u32,
    request_random: u32,
) -> Result<ControlRequest, ControlError> {
    if group_id == 0
        || request_random == 0
        || !matches!(chat_type, 1 | 2)
        || !valid_text(character_id)
        || !valid_text(text)
    {
        return Err(ControlError);
    }
    request(
        0x929b,
        0,
        "OidbSvcTrpcTcp.0x929b_0",
        None,
        &AiVoiceGenerationRequest {
            group_id,
            character_id: character_id.to_owned(),
            text: text.to_owned(),
            chat_type,
            client: Some(AiVoiceGenerationClient { request_random }),
        },
    )
}

/// Parses one bounded AI voice generation poll.
///
/// # Errors
///
/// Returns an error for rejection, an unknown state, or incomplete/excessive message material.
pub fn parse_ai_voice_generation_response(input: &[u8]) -> Result<AiVoiceGeneration, ControlError> {
    if input.len() > MAX_RESPONSE_BYTES {
        return Err(ControlError);
    }
    let outer = decode_oidb_response(input).map_err(|_error| ControlError)?;
    if outer.error_code() != 0 {
        return Err(ControlError);
    }
    let response =
        AiVoiceGenerationResponse::decode(outer.body()).map_err(|_error| ControlError)?;
    match response.state {
        1 if !response.message_info.is_empty()
            && response.message_info.len() <= MAX_MESSAGE_INFO_BYTES =>
        {
            Ok(AiVoiceGeneration::Ready(response.message_info))
        }
        2 if response.message_info.is_empty() => Ok(AiVoiceGeneration::Pending),
        _ => Err(ControlError),
    }
}

/// Parses the bounded character catalogue returned by QQ.
///
/// # Errors
///
/// Returns an error for rejection, malformed protobuf, or excessive/unusable fields.
pub fn parse_ai_voice_characters_response(
    input: &[u8],
) -> Result<Vec<AiVoiceCategory>, ControlError> {
    if input.len() > MAX_RESPONSE_BYTES {
        return Err(ControlError);
    }
    let outer = decode_oidb_response(input).map_err(|_error| ControlError)?;
    if outer.error_code() != 0 {
        return Err(ControlError);
    }
    let response =
        AiVoiceCharactersResponse::decode(outer.body()).map_err(|_error| ControlError)?;
    if response.categories.len() > MAX_CATEGORIES {
        return Err(ControlError);
    }
    response
        .categories
        .into_iter()
        .map(project_category)
        .collect()
}

fn project_category(category: AiVoiceCategoryWire) -> Result<AiVoiceCategory, ControlError> {
    if !valid_text(&category.kind) || category.characters.len() > MAX_CHARACTERS_PER_CATEGORY {
        return Err(ControlError);
    }
    let characters = category
        .characters
        .into_iter()
        .map(|character| {
            if !valid_text(&character.character_id)
                || !valid_text(&character.character_name)
                || !valid_url(&character.preview_url)
            {
                return Err(ControlError);
            }
            Ok(AiVoiceCharacter {
                character_id: character.character_id,
                character_name: character.character_name,
                preview_url: character.preview_url,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AiVoiceCategory {
        kind: category.kind,
        characters,
    })
}

fn valid_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_TEXT_BYTES
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn valid_url(value: &str) -> bool {
    value.len() <= MAX_TEXT_BYTES
        && (value.starts_with("https://") || value.starts_with("http://"))
        && !value.chars().any(char::is_whitespace)
}

#[derive(Clone, Copy, PartialEq, Message)]
struct AiVoiceCharactersRequest {
    #[prost(uint32, tag = "1")]
    group_id: u32,
    #[prost(uint32, tag = "2")]
    chat_type: u32,
}

#[derive(Clone, PartialEq, Message)]
struct AiVoiceGenerationRequest {
    #[prost(uint32, tag = "1")]
    group_id: u32,
    #[prost(string, tag = "2")]
    character_id: String,
    #[prost(string, tag = "3")]
    text: String,
    #[prost(uint32, tag = "4")]
    chat_type: u32,
    #[prost(message, optional, tag = "5")]
    client: Option<AiVoiceGenerationClient>,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct AiVoiceGenerationClient {
    #[prost(uint32, tag = "1")]
    request_random: u32,
}

#[derive(Clone, PartialEq, Message)]
struct AiVoiceGenerationResponse {
    #[prost(uint32, tag = "1")]
    state: u32,
    #[prost(uint32, optional, tag = "2")]
    server_code: Option<u32>,
    #[prost(uint32, tag = "3")]
    progress: u32,
    #[prost(bytes = "vec", tag = "4")]
    message_info: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
struct AiVoiceCharactersResponse {
    #[prost(message, repeated, tag = "1")]
    categories: Vec<AiVoiceCategoryWire>,
}

#[derive(Clone, PartialEq, Message)]
struct AiVoiceCategoryWire {
    #[prost(string, tag = "1")]
    kind: String,
    #[prost(message, repeated, tag = "2")]
    characters: Vec<AiVoiceCharacterWire>,
}

#[derive(Clone, PartialEq, Message)]
struct AiVoiceCharacterWire {
    #[prost(string, tag = "1")]
    character_id: String,
    #[prost(string, tag = "2")]
    character_name: String,
    #[prost(string, tag = "3")]
    preview_url: String,
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::{
        AiVoiceCategoryWire, AiVoiceCharacterWire, AiVoiceCharactersRequest,
        AiVoiceCharactersResponse, AiVoiceGeneration, AiVoiceGenerationRequest,
        AiVoiceGenerationResponse, ai_voice_characters, generate_ai_voice,
        parse_ai_voice_characters_response, parse_ai_voice_generation_response,
    };

    #[test]
    fn request_matches_frozen_shape_and_is_unsigned() -> Result<(), Box<dyn std::error::Error>> {
        let request = ai_voice_characters(123, 2)?;
        assert_eq!(request.command(), "OidbSvcTrpcTcp.0x929d_0");
        assert_eq!(request.signing_operation(), None);
        let outer = qq_wire::decode_oidb_request(request.body())?;
        assert_eq!((outer.command(), outer.subcommand()), (0x929d, 0));
        let body = AiVoiceCharactersRequest::decode(outer.body())?;
        assert_eq!((body.group_id, body.chat_type), (123, 2));
        assert!(ai_voice_characters(123, 3).is_err());
        Ok(())
    }

    #[test]
    fn generation_reuses_the_callers_random_and_closes_unknown_states()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = generate_ai_voice(123, "voice-id", "hello", 1, 456)?;
        assert_eq!(request.command(), "OidbSvcTrpcTcp.0x929b_0");
        assert_eq!(request.signing_operation(), None);
        let outer = qq_wire::decode_oidb_request(request.body())?;
        let body = AiVoiceGenerationRequest::decode(outer.body())?;
        assert_eq!(body.client.ok_or("client missing")?.request_random, 456);

        let pending = oidb_response(&AiVoiceGenerationResponse {
            state: 2,
            server_code: Some(319),
            progress: 20,
            message_info: Vec::new(),
        });
        assert_eq!(
            parse_ai_voice_generation_response(&pending)?,
            AiVoiceGeneration::Pending
        );
        let ready = oidb_response(&AiVoiceGenerationResponse {
            state: 1,
            server_code: None,
            progress: 100,
            message_info: vec![0x0a, 0x00],
        });
        assert_eq!(
            parse_ai_voice_generation_response(&ready)?,
            AiVoiceGeneration::Ready(vec![0x0a, 0x00])
        );
        let unknown = oidb_response(&AiVoiceGenerationResponse {
            state: 3,
            server_code: None,
            progress: 0,
            message_info: Vec::new(),
        });
        assert!(parse_ai_voice_generation_response(&unknown).is_err());
        Ok(())
    }

    fn oidb_response(body: &AiVoiceGenerationResponse) -> Vec<u8> {
        TestOidbResponse {
            error_code: 0,
            body: body.encode_to_vec(),
        }
        .encode_to_vec()
    }

    #[test]
    fn response_projects_the_frozen_onebot_fields() -> Result<(), Box<dyn std::error::Error>> {
        let body = AiVoiceCharactersResponse {
            categories: vec![AiVoiceCategoryWire {
                kind: "voice".to_owned(),
                characters: vec![AiVoiceCharacterWire {
                    character_id: "voice-id".to_owned(),
                    character_name: "Voice Name".to_owned(),
                    preview_url: "https://example.invalid/preview.amr".to_owned(),
                }],
            }],
        }
        .encode_to_vec();
        let response = TestOidbResponse {
            error_code: 0,
            body,
        }
        .encode_to_vec();
        let categories = parse_ai_voice_characters_response(&response)?;
        assert_eq!(categories[0].kind, "voice");
        assert_eq!(categories[0].characters[0].character_id, "voice-id");
        Ok(())
    }

    #[derive(Clone, PartialEq, Message)]
    struct TestOidbResponse {
        #[prost(uint32, tag = "3")]
        error_code: u32,
        #[prost(bytes = "vec", tag = "4")]
        body: Vec<u8>,
    }
}

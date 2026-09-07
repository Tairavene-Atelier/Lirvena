use prost::Message;
use qq_wire::decode_oidb_response;

use crate::{ControlError, ControlRequest, request};

const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_CATEGORIES: usize = 32;
const MAX_CHARACTERS_PER_CATEGORY: usize = 256;
const MAX_TEXT_BYTES: usize = 1024;

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
        AiVoiceCharactersResponse, ai_voice_characters, parse_ai_voice_characters_response,
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

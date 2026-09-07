use prost::Message;

use crate::{ControlError, ControlRequest, request};

const MAX_IMAGE_URL_BYTES: usize = 16 * 1024;
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_DETECTIONS: usize = 2_048;
const MAX_COORDINATES: usize = 64;
const MAX_TEXT_BYTES: usize = 64 * 1024;
const MAX_LANGUAGE_BYTES: usize = 256;

/// One coordinate returned by QQ image recognition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageOcrCoordinate {
    /// Horizontal image coordinate.
    pub x: i32,
    /// Vertical image coordinate.
    pub y: i32,
}

/// One bounded text region returned by QQ image recognition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageOcrDetection {
    /// Text recognized inside the polygon.
    pub text: String,
    /// Confidence value reported by QQ.
    pub confidence: u32,
    /// Polygon coordinates in QQ response order.
    pub coordinates: Vec<ImageOcrCoordinate>,
}

/// Bounded image-recognition result returned by QQ.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageOcrResult {
    /// Recognized text regions in QQ response order.
    pub texts: Vec<ImageOcrDetection>,
    /// Language identifier reported by QQ.
    pub language: String,
}

/// Encodes an OCR request for a QQ-hosted image URL.
///
/// # Errors
///
/// Returns an error for an empty, excessive, or control-bearing URL.
pub fn image_ocr(image_url: &str) -> Result<ControlRequest, ControlError> {
    if image_url.is_empty()
        || image_url.len() > MAX_IMAGE_URL_BYTES
        || image_url.trim() != image_url
        || image_url.chars().any(char::is_control)
    {
        return Err(ControlError);
    }
    request(
        0x0e07,
        0,
        "OidbSvcTrpcTcp.0xe07_0",
        None,
        &ImageOcrRequest {
            version: 1,
            client: 0,
            entrance: 1,
            body: Some(ImageOcrRequestBody {
                image_url: image_url.to_owned(),
                language_type: 0,
                scene: 0,
                origin_md5: String::new(),
                compressed_md5: String::new(),
                compressed_file_size: String::new(),
                compressed_width: String::new(),
                compressed_height: String::new(),
                is_cut: false,
            }),
        },
    )
}

/// Parses a bounded QQ OCR response.
///
/// # Errors
///
/// Returns an error for rejected, malformed, incomplete, or excessive data.
pub fn parse_image_ocr_response(input: &[u8]) -> Result<ImageOcrResult, ControlError> {
    if input.len() > MAX_RESPONSE_BYTES {
        return Err(ControlError);
    }
    let outer = qq_wire::decode_oidb_response(input).map_err(|_error| ControlError)?;
    if outer.error_code() != 0 {
        return Err(ControlError);
    }
    let response = ImageOcrResponse::decode(outer.body()).map_err(|_error| ControlError)?;
    if response.result != 0 {
        return Err(ControlError);
    }
    let body = response.body.ok_or(ControlError)?;
    if body.detections.len() > MAX_DETECTIONS || !valid_language(&body.language) {
        return Err(ControlError);
    }
    let texts = body
        .detections
        .into_iter()
        .map(|detection| {
            if !valid_recognized_text(&detection.text) {
                return Err(ControlError);
            }
            let coordinates = detection.polygon.ok_or(ControlError)?.coordinates;
            if coordinates.len() > MAX_COORDINATES {
                return Err(ControlError);
            }
            Ok(ImageOcrDetection {
                text: detection.text,
                confidence: detection.confidence,
                coordinates: coordinates
                    .into_iter()
                    .map(|coordinate| ImageOcrCoordinate {
                        x: coordinate.x,
                        y: coordinate.y,
                    })
                    .collect(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ImageOcrResult {
        texts,
        language: body.language,
    })
}

fn valid_language(value: &str) -> bool {
    value.len() <= MAX_LANGUAGE_BYTES && !value.chars().any(char::is_control)
}

fn valid_recognized_text(value: &str) -> bool {
    value.len() <= MAX_TEXT_BYTES
        && !value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
}

#[derive(Clone, PartialEq, Message)]
struct ImageOcrRequest {
    #[prost(uint32, tag = "1")]
    version: u32,
    #[prost(uint32, tag = "2")]
    client: u32,
    #[prost(uint32, tag = "3")]
    entrance: u32,
    #[prost(message, optional, tag = "10")]
    body: Option<ImageOcrRequestBody>,
}

#[derive(Clone, PartialEq, Message)]
struct ImageOcrRequestBody {
    #[prost(string, tag = "1")]
    image_url: String,
    #[prost(uint32, tag = "2")]
    language_type: u32,
    #[prost(uint32, tag = "3")]
    scene: u32,
    #[prost(string, tag = "10")]
    origin_md5: String,
    #[prost(string, tag = "11")]
    compressed_md5: String,
    #[prost(string, tag = "12")]
    compressed_file_size: String,
    #[prost(string, tag = "13")]
    compressed_width: String,
    #[prost(string, tag = "14")]
    compressed_height: String,
    #[prost(bool, tag = "15")]
    is_cut: bool,
}

#[derive(Clone, PartialEq, Message)]
struct ImageOcrResponse {
    #[prost(int32, tag = "1")]
    result: i32,
    #[prost(string, tag = "2")]
    error_message: String,
    #[prost(string, tag = "3")]
    wording: String,
    #[prost(message, optional, tag = "10")]
    body: Option<ImageOcrResponseBody>,
}

#[derive(Clone, PartialEq, Message)]
struct ImageOcrResponseBody {
    #[prost(message, repeated, tag = "1")]
    detections: Vec<WireDetection>,
    #[prost(string, tag = "2")]
    language: String,
}

#[derive(Clone, PartialEq, Message)]
struct WireDetection {
    #[prost(string, tag = "1")]
    text: String,
    #[prost(uint32, tag = "2")]
    confidence: u32,
    #[prost(message, optional, tag = "3")]
    polygon: Option<WirePolygon>,
}

#[derive(Clone, PartialEq, Message)]
struct WirePolygon {
    #[prost(message, repeated, tag = "1")]
    coordinates: Vec<WireCoordinate>,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct WireCoordinate {
    #[prost(int32, tag = "1")]
    x: i32,
    #[prost(int32, tag = "2")]
    y: i32,
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::{
        ImageOcrRequest, ImageOcrResponse, ImageOcrResponseBody, WireCoordinate, WireDetection,
        WirePolygon, image_ocr, parse_image_ocr_response,
    };

    #[test]
    fn request_preserves_the_frozen_shape() -> Result<(), Box<dyn std::error::Error>> {
        let request = image_ocr("https://example.invalid/image")?;
        assert_eq!(request.command(), "OidbSvcTrpcTcp.0xe07_0");
        assert_eq!(request.signing_operation(), None);
        let outer = qq_wire::decode_oidb_request(request.body())?;
        assert_eq!((outer.command(), outer.subcommand()), (0x0e07, 0));
        let body = ImageOcrRequest::decode(outer.body())?;
        assert_eq!((body.version, body.client, body.entrance), (1, 0, 1));
        let image = body.body.ok_or("OCR body missing")?;
        assert_eq!(image.image_url, "https://example.invalid/image");
        assert_eq!((image.language_type, image.scene), (0, 0));
        assert_eq!(
            (
                image.origin_md5,
                image.compressed_md5,
                image.compressed_file_size,
                image.compressed_width,
                image.compressed_height,
                image.is_cut,
            ),
            (
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                false,
            )
        );
        Ok(())
    }

    #[test]
    fn response_preserves_text_confidence_coordinates_and_language()
    -> Result<(), Box<dyn std::error::Error>> {
        let inner = ImageOcrResponse {
            result: 0,
            error_message: String::new(),
            wording: String::new(),
            body: Some(ImageOcrResponseBody {
                detections: vec![WireDetection {
                    text: "first\nsecond".to_owned(),
                    confidence: 97,
                    polygon: Some(WirePolygon {
                        coordinates: vec![
                            WireCoordinate { x: 10, y: 20 },
                            WireCoordinate { x: 30, y: 40 },
                        ],
                    }),
                }],
                language: "zh".to_owned(),
            }),
        }
        .encode_to_vec();
        let outer = qq_wire::encode_oidb_request(0x0e07, 0, &inner, 0)?;
        let result = parse_image_ocr_response(&outer)?;
        assert_eq!(result.language, "zh");
        assert_eq!(result.texts[0].text, "first\nsecond");
        assert_eq!(result.texts[0].confidence, 97);
        assert_eq!(
            (
                result.texts[0].coordinates[1].x,
                result.texts[0].coordinates[1].y
            ),
            (30, 40)
        );
        Ok(())
    }

    #[test]
    fn rejected_or_incomplete_responses_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
        assert!(image_ocr(" bad").is_err());
        let rejected = ImageOcrResponse {
            result: 12,
            error_message: String::new(),
            wording: String::new(),
            body: None,
        }
        .encode_to_vec();
        let outer = qq_wire::encode_oidb_request(0x0e07, 0, &rejected, 0)?;
        assert!(parse_image_ocr_response(&outer).is_err());

        let missing_polygon = ImageOcrResponse {
            result: 0,
            error_message: String::new(),
            wording: String::new(),
            body: Some(ImageOcrResponseBody {
                detections: vec![WireDetection {
                    text: "text".to_owned(),
                    confidence: 1,
                    polygon: None,
                }],
                language: String::new(),
            }),
        }
        .encode_to_vec();
        let outer = qq_wire::encode_oidb_request(0x0e07, 0, &missing_polygon, 0)?;
        assert!(parse_image_ocr_response(&outer).is_err());
        Ok(())
    }
}

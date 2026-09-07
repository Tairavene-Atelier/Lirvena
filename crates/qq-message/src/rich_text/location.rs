use serde_json::{Value, json};

use crate::MessageDecodeError;

const MAX_COORDINATE_BYTES: usize = 32;
const MAX_TITLE_BYTES: usize = 256;
const MAX_CONTENT_BYTES: usize = 1_024;

/// One QQ location card decoded from the `com.tencent.map` light application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocationSegment {
    latitude: String,
    longitude: String,
    title: String,
    content: String,
}

impl LocationSegment {
    pub(super) fn from_light_app(value: &Value) -> Option<Self> {
        if value.get("app")?.as_str()? != "com.tencent.map" {
            return None;
        }
        let location = value.pointer("/meta/Location.Search")?;
        let latitude = bounded_string(location.get("lat")?, MAX_COORDINATE_BYTES)?;
        let longitude = bounded_string(location.get("lng")?, MAX_COORDINATE_BYTES)?;
        let title = bounded_string(location.get("name")?, MAX_TITLE_BYTES)?;
        let content = bounded_string(location.get("address")?, MAX_CONTENT_BYTES)?;
        Some(Self {
            latitude,
            longitude,
            title,
            content,
        })
    }

    /// Returns the latitude supplied by QQ.
    #[must_use]
    pub fn latitude(&self) -> &str {
        &self.latitude
    }

    /// Returns the longitude supplied by QQ.
    #[must_use]
    pub fn longitude(&self) -> &str {
        &self.longitude
    }

    /// Returns the display title supplied by QQ.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns the address text supplied by QQ.
    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }
}

pub(crate) fn encode_location(
    latitude: &str,
    longitude: &str,
    title: &str,
    content: &str,
) -> Result<String, MessageDecodeError> {
    validate_coordinate(latitude, -90.0, 90.0)?;
    validate_coordinate(longitude, -180.0, 180.0)?;
    validate_text(title, MAX_TITLE_BYTES)?;
    validate_text(content, MAX_CONTENT_BYTES)?;
    Ok(json!({
        "app": "com.tencent.map",
        "desc": "",
        "from": 1,
        "meta": {
            "Location.Search": {
                "address": content,
                "enum_relation_type": 1,
                "from": "plusPanel",
                "id": "",
                "lat": latitude,
                "lng": longitude,
                "name": title
            }
        },
        "prompt": format!("[Location]{content}"),
        "ver": "1.1.2.21",
        "view": "LocationShare"
    })
    .to_string())
}

fn bounded_string(value: &Value, max_bytes: usize) -> Option<String> {
    let value = value.as_str()?;
    (value.len() <= max_bytes && !value.contains('\0')).then(|| value.to_owned())
}

fn validate_coordinate(value: &str, minimum: f64, maximum: f64) -> Result<(), MessageDecodeError> {
    if value.is_empty() || value.len() > MAX_COORDINATE_BYTES || value.contains('\0') {
        return Err(MessageDecodeError);
    }
    let coordinate = value.parse::<f64>().map_err(|_error| MessageDecodeError)?;
    if !coordinate.is_finite() || !(minimum..=maximum).contains(&coordinate) {
        return Err(MessageDecodeError);
    }
    Ok(())
}

fn validate_text(value: &str, max_bytes: usize) -> Result<(), MessageDecodeError> {
    if value.len() > max_bytes || value.contains('\0') {
        return Err(MessageDecodeError);
    }
    Ok(())
}

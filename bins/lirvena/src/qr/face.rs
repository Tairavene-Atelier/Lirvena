use std::io;
use std::time::Duration;

use reqwest::header::CONTENT_TYPE;
use serde::{Deserialize, Serialize};

const FACE_ENDPOINT: &str = "https://ntlogin.qq.com/qr/getFace";
const MAX_RESPONSE_BYTES: usize = 16 * 1024;

pub(super) struct FaceResolver {
    client: reqwest::Client,
}

#[derive(Serialize)]
struct FaceRequest<'a> {
    appid: u32,
    #[serde(rename = "faceUpdateTime")]
    face_update_time: u8,
    qrsig: &'a str,
}

#[derive(Deserialize)]
struct FaceResponse {
    #[serde(rename = "retCode")]
    return_code: i32,
    uin: u32,
}

impl FaceResolver {
    pub(super) fn new() -> Result<Self, io::Error> {
        crate::support::ensure_rustls_provider()?;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|_error| invalid_response())?;
        Ok(Self { client })
    }

    pub(super) async fn resolve(
        &self,
        app_id: u32,
        query_signature: &str,
    ) -> Result<Option<u32>, io::Error> {
        if app_id == 0 || query_signature.is_empty() || query_signature.len() > 2_048 {
            return Err(invalid_response());
        }
        let body = serde_json::to_vec(&FaceRequest {
            appid: app_id,
            face_update_time: 0,
            qrsig: query_signature,
        })
        .map_err(|_error| invalid_response())?;
        let response = self
            .client
            .post(FACE_ENDPOINT)
            .header(CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
            .map_err(|_error| invalid_response())?;
        if !response.status().is_success() {
            return Err(invalid_response());
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|_error| invalid_response())?;
        if bytes.len() > MAX_RESPONSE_BYTES {
            return Err(invalid_response());
        }
        let response: FaceResponse =
            serde_json::from_slice(&bytes).map_err(|_error| invalid_response())?;
        Ok((response.return_code == 0 && response.uin != 0).then_some(response.uin))
    }
}

fn invalid_response() -> io::Error {
    io::Error::other("QQ QR identity binding failed")
}

#[cfg(test)]
mod tests {
    use super::FaceResponse;

    #[test]
    fn decodes_bounded_identity_response() -> Result<(), Box<dyn std::error::Error>> {
        let response: FaceResponse = serde_json::from_slice(
            br#"{"retCode":0,"errMsg":"","qrSig":"opaque","uin":123,"faceUrl":"","faceUpdateTime":0}"#,
        )?;
        assert_eq!(response.return_code, 0);
        assert_eq!(response.uin, 123);
        Ok(())
    }
}

use axum::http::{HeaderMap, Uri};
use subtle::ConstantTimeEq;

const MAX_ACCESS_TOKEN_BYTES: usize = 1_024;

pub(super) struct AccessToken(Box<[u8]>);

impl AccessToken {
    pub(super) fn new(value: Option<Vec<u8>>) -> Result<Option<Self>, ()> {
        value
            .map(|value| {
                if value.is_empty() || value.len() > MAX_ACCESS_TOKEN_BYTES || value.contains(&0) {
                    return Err(());
                }
                Ok(Self(value.into_boxed_slice()))
            })
            .transpose()
    }

    pub(super) fn authorize(&self, headers: &HeaderMap, uri: &Uri) -> Authorization {
        let header = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .map(str::as_bytes);
        let query = uri
            .query()
            .and_then(|query| query.split('&').find_map(access_token_parameter))
            .map(str::as_bytes);
        match header.or(query) {
            None => Authorization::Missing,
            Some(candidate) if bool::from(candidate.ct_eq(&self.0)) => Authorization::Allowed,
            Some(_) => Authorization::Denied,
        }
    }
}

impl core::fmt::Debug for AccessToken {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("AccessToken(<redacted>)")
    }
}

fn access_token_parameter(parameter: &str) -> Option<&str> {
    let (name, value) = parameter.split_once('=')?;
    (name == "access_token").then_some(value)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Authorization {
    Allowed,
    Missing,
    Denied,
}

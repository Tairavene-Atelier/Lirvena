use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use account_api::{AccountActionError, AccountActionRequest};
use futures_util::StreamExt;
use qq_control::{
    client_key_request, domain_ticket_request, parse_client_key_response,
    parse_domain_ticket_response,
};
use reqwest::cookie::{CookieStore, Jar};
use reqwest::redirect::Policy;
use reqwest::{Client, Url};
use reqwest::{Method, RequestBuilder, Response};
use serde_json::{Value, json};
use zeroize::Zeroize;

use super::controls::send_control_response;
use super::packets::PacketRuntime;
use super::push::PushRuntime;
use super::runtime::OnlineContext;
use crate::support::now_seconds;

const CLIENT_KEY_FALLBACK_SECONDS: u64 = 30 * 60;
const DOMAIN_TICKET_SECONDS: u64 = 20 * 60;
const SKEY_SECONDS: u64 = 24 * 60 * 60;

pub(super) struct TicketRuntime {
    client: Client,
    jar: Arc<Jar>,
    client_key: Option<CachedSecret>,
    skey: Option<CachedSecret>,
    domain_tickets: BTreeMap<String, CachedSecret>,
}

pub(super) struct TicketAccess<'a, 'context> {
    uin: u64,
    packets: &'a PacketRuntime,
    pushes: &'a PushRuntime,
    online: &'a mut OnlineContext<'context>,
}

impl<'a, 'context> TicketAccess<'a, 'context> {
    pub(super) const fn new(
        uin: u64,
        packets: &'a PacketRuntime,
        pushes: &'a PushRuntime,
        online: &'a mut OnlineContext<'context>,
    ) -> Self {
        Self {
            uin,
            packets,
            pushes,
            online,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TicketRuntimeError;

impl core::fmt::Display for TicketRuntimeError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("QQ web ticket runtime configuration failed")
    }
}

impl std::error::Error for TicketRuntimeError {}

impl TicketRuntime {
    pub(super) fn new() -> Result<Self, TicketRuntimeError> {
        ensure_crypto_provider()?;
        let jar = Arc::new(Jar::default());
        let client = Client::builder()
            .cookie_provider(jar.clone())
            .redirect(Policy::limited(5))
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|_error| TicketRuntimeError)?;
        Ok(Self {
            client,
            jar,
            client_key: None,
            skey: None,
            domain_tickets: BTreeMap::new(),
        })
    }

    pub(super) async fn execute(
        &mut self,
        request: &AccountActionRequest,
        uin: u64,
        packets: &PacketRuntime,
        pushes: &PushRuntime,
        context: &mut OnlineContext<'_>,
    ) -> Result<Value, AccountActionError> {
        match request.action() {
            "get_cookies" => {
                let domain = required_domain(request)?;
                let cookies = self.cookies(domain, uin, packets, pushes, context).await?;
                Ok(json!({"cookies": cookies}))
            }
            "get_csrf_token" => {
                let skey = self.skey(uin, packets, pushes, context).await?;
                Ok(json!({"token": csrf_token(&skey)}))
            }
            "get_credentials" => {
                let domain = required_domain(request)?;
                let cookies = self.cookies(domain, uin, packets, pushes, context).await?;
                let skey = self.skey(uin, packets, pushes, context).await?;
                Ok(json!({"cookies": cookies, "csrf_token": csrf_token(&skey)}))
            }
            _ => Err(AccountActionError::Unsupported),
        }
    }

    async fn cookies(
        &mut self,
        domain: &str,
        uin: u64,
        packets: &PacketRuntime,
        pushes: &PushRuntime,
        context: &mut OnlineContext<'_>,
    ) -> Result<String, AccountActionError> {
        let skey = self.skey(uin, packets, pushes, context).await?;
        let ticket = self.domain_ticket(domain, packets, pushes, context).await?;
        Ok(format!(
            "p_uin=o{uin}; p_skey={ticket}; skey={skey}; uin=o{uin}"
        ))
    }

    pub(super) async fn csrf(
        &mut self,
        uin: u64,
        packets: &PacketRuntime,
        pushes: &PushRuntime,
        context: &mut OnlineContext<'_>,
    ) -> Result<i32, AccountActionError> {
        let skey = self.skey(uin, packets, pushes, context).await?;
        Ok(csrf_token(&skey))
    }

    pub(super) async fn cookie_header(
        &mut self,
        domain: &str,
        access: TicketAccess<'_, '_>,
    ) -> Result<String, AccountActionError> {
        self.cookies(
            domain,
            access.uin,
            access.packets,
            access.pushes,
            access.online,
        )
        .await
    }

    pub(super) async fn authenticated_request(
        &mut self,
        method: Method,
        url: Url,
        domain: &str,
        access: TicketAccess<'_, '_>,
    ) -> Result<RequestBuilder, AccountActionError> {
        let cookies = self
            .cookies(
                domain,
                access.uin,
                access.packets,
                access.pushes,
                access.online,
            )
            .await?;
        Ok(self.client.request(method, url).header("cookie", cookies))
    }

    pub(super) async fn authenticated_request_with_cookie_suffix(
        &mut self,
        method: Method,
        url: Url,
        domain: &str,
        access: TicketAccess<'_, '_>,
        suffix: &str,
    ) -> Result<RequestBuilder, AccountActionError> {
        if suffix.is_empty() || suffix.chars().any(char::is_control) {
            return Err(AccountActionError::BadParameters);
        }
        let cookies = self.cookie_header(domain, access).await?;
        Ok(self
            .client
            .request(method, url)
            .header("cookie", format!("{cookies}; {suffix}")))
    }

    pub(super) async fn bounded_body(
        response: Response,
        maximum: usize,
    ) -> Result<Vec<u8>, AccountActionError> {
        let response = response
            .error_for_status()
            .map_err(|_error| AccountActionError::QqFailure)?;
        if response
            .content_length()
            .is_some_and(|length| length > maximum as u64)
        {
            return Err(AccountActionError::QqFailure);
        }
        let mut stream = response.bytes_stream();
        let mut body = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_error| AccountActionError::QqFailure)?;
            if body.len().saturating_add(chunk.len()) > maximum {
                return Err(AccountActionError::QqFailure);
            }
            body.extend_from_slice(&chunk);
        }
        Ok(body)
    }

    async fn domain_ticket(
        &mut self,
        domain: &str,
        packets: &PacketRuntime,
        pushes: &PushRuntime,
        context: &mut OnlineContext<'_>,
    ) -> Result<String, AccountActionError> {
        let now = u64::from(now_seconds().map_err(|_error| AccountActionError::QqFailure)?);
        if let Some(secret) = self
            .domain_tickets
            .get(domain)
            .filter(|value| value.fresh(now))
        {
            return Ok(secret.value.clone());
        }
        let domains = vec![domain.to_owned()];
        let request =
            domain_ticket_request(&domains).map_err(|_error| AccountActionError::BadParameters)?;
        let response = send_control_response(&request, packets, pushes, context).await?;
        let ticket = parse_domain_ticket_response(&response, &domains)
            .map_err(|_error| AccountActionError::QqFailure)?
            .into_iter()
            .next()
            .ok_or(AccountActionError::QqFailure)?;
        let value = ticket.value().to_owned();
        self.domain_tickets.insert(
            domain.to_owned(),
            CachedSecret::new(value.clone(), now.saturating_add(DOMAIN_TICKET_SECONDS)),
        );
        Ok(value)
    }

    async fn skey(
        &mut self,
        uin: u64,
        packets: &PacketRuntime,
        pushes: &PushRuntime,
        context: &mut OnlineContext<'_>,
    ) -> Result<String, AccountActionError> {
        let now = u64::from(now_seconds().map_err(|_error| AccountActionError::QqFailure)?);
        if let Some(secret) = self.skey.as_ref().filter(|value| value.fresh(now)) {
            return Ok(secret.value.clone());
        }
        let client_key = self.client_key(now, packets, pushes, context).await?;
        let uin_text = uin.to_string();
        let endpoint = Url::parse("https://ssl.ptlogin2.qq.com/jump")
            .map_err(|_error| AccountActionError::QqFailure)?;
        let response = self
            .client
            .get(endpoint)
            .query(&[
                ("ptlang", "1033"),
                ("clientuin", uin_text.as_str()),
                ("clientkey", &client_key),
                (
                    "u1",
                    "https://h5.qzone.qq.com/qqnt/qzoneinpcqq/friend?refresh=0&clientuin=0&darkMode=0",
                ),
                ("keyindex", "19"),
                ("random", "2599"),
            ])
            .send()
            .await
            .map_err(|_error| AccountActionError::QqFailure)?
            .error_for_status()
            .map_err(|_error| AccountActionError::QqFailure)?;
        drop(response);
        let skey = self.cookie_value("skey")?;
        self.skey = Some(CachedSecret::new(
            skey.clone(),
            now.saturating_add(SKEY_SECONDS),
        ));
        Ok(skey)
    }

    async fn client_key(
        &mut self,
        now: u64,
        packets: &PacketRuntime,
        pushes: &PushRuntime,
        context: &mut OnlineContext<'_>,
    ) -> Result<String, AccountActionError> {
        if let Some(secret) = self.client_key.as_ref().filter(|value| value.fresh(now)) {
            return Ok(secret.value.clone());
        }
        let request = client_key_request().map_err(|_error| AccountActionError::QqFailure)?;
        let response = send_control_response(&request, packets, pushes, context).await?;
        let (value, raw_expiration) =
            parse_client_key_response(&response).map_err(|_error| AccountActionError::QqFailure)?;
        let expires_at = resolve_expiration(now, raw_expiration);
        self.client_key = Some(CachedSecret::new(value.clone(), expires_at));
        Ok(value)
    }

    fn cookie_value(&self, name: &str) -> Result<String, AccountActionError> {
        for endpoint in ["https://qq.com/", "https://ssl.ptlogin2.qq.com/"] {
            let url = Url::parse(endpoint).map_err(|_error| AccountActionError::QqFailure)?;
            let Some(header) = self.jar.cookies(&url) else {
                continue;
            };
            let value = header
                .to_str()
                .map_err(|_error| AccountActionError::QqFailure)?;
            for pair in value.split(';').map(str::trim) {
                if let Some((key, secret)) = pair.split_once('=')
                    && key == name
                    && !secret.is_empty()
                    && secret.len() <= 4 * 1024
                    && !secret.chars().any(char::is_control)
                {
                    return Ok(secret.to_owned());
                }
            }
        }
        Err(AccountActionError::QqFailure)
    }
}

fn ensure_crypto_provider() -> Result<(), TicketRuntimeError> {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _result = rustls::crypto::ring::default_provider().install_default();
    }
    rustls::crypto::CryptoProvider::get_default()
        .is_some()
        .then_some(())
        .ok_or(TicketRuntimeError)
}

struct CachedSecret {
    value: String,
    expires_at: u64,
}

impl CachedSecret {
    const fn new(value: String, expires_at: u64) -> Self {
        Self { value, expires_at }
    }

    const fn fresh(&self, now: u64) -> bool {
        now < self.expires_at
    }
}

impl Drop for CachedSecret {
    fn drop(&mut self) {
        self.value.zeroize();
    }
}

fn required_domain(request: &AccountActionRequest) -> Result<&str, AccountActionError> {
    request
        .params()
        .get("domain")
        .and_then(Value::as_str)
        .ok_or(AccountActionError::BadParameters)
}

fn resolve_expiration(now: u64, raw: u32) -> u64 {
    let raw = u64::from(raw);
    if raw == 0 {
        now.saturating_add(CLIENT_KEY_FALLBACK_SECONDS)
    } else if raw > now.saturating_sub(86_400) {
        raw
    } else {
        now.saturating_add(raw)
    }
}

fn csrf_token(skey: &str) -> i32 {
    skey.bytes().fold(5_381_i32, |hash, byte| {
        hash.wrapping_mul(33).wrapping_add(i32::from(byte))
    }) & 0x7fff_ffff
}

#[cfg(test)]
mod tests {
    use reqwest::Url;

    use super::{TicketRuntime, csrf_token, resolve_expiration};

    #[test]
    fn csrf_and_expiration_match_qq_web_semantics() {
        assert_eq!(csrf_token("abc"), 193_485_963);
        assert_eq!(resolve_expiration(1_000_000, 0), 1_001_800);
        assert_eq!(resolve_expiration(1_000_000, 300), 1_000_300);
        assert_eq!(resolve_expiration(1_000_000, 999_999), 999_999);
    }

    #[test]
    fn cookie_lookup_reads_only_the_named_bounded_cookie() -> Result<(), Box<dyn std::error::Error>>
    {
        let runtime = TicketRuntime::new()?;
        runtime.jar.add_cookie_str(
            "skey=web-secret; Domain=qq.com; Path=/; Secure",
            &Url::parse("https://qq.com/")?,
        );
        assert_eq!(runtime.cookie_value("skey")?, "web-secret");
        assert!(runtime.cookie_value("p_skey").is_err());
        Ok(())
    }
}

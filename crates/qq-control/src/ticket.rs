use std::collections::BTreeSet;

use prost::Message;
use qq_wire::{decode_oidb_response, encode_empty_oidb_request};

use crate::{ControlError, ControlRequest, request};

const MAX_DOMAINS: usize = 16;
const MAX_DOMAIN_BYTES: usize = 253;
const MAX_TICKET_BYTES: usize = 8 * 1024;
const MAX_CLIENT_KEY_BYTES: usize = 4 * 1024;

/// One domain-bound QQ web ticket.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DomainTicket {
    domain: String,
    value: String,
}

impl DomainTicket {
    /// Returns the requested domain.
    #[must_use]
    pub fn domain(&self) -> &str {
        &self.domain
    }

    /// Returns the opaque QQ ticket value.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
}

/// Encodes the 52194 domain-ticket request.
///
/// # Errors
///
/// Returns an error for an empty, duplicate, excessive, or unsafe domain set.
pub fn domain_ticket_request(domains: &[String]) -> Result<ControlRequest, ControlError> {
    if domains.is_empty() || domains.len() > MAX_DOMAINS {
        return Err(ControlError);
    }
    let mut unique = BTreeSet::new();
    for domain in domains {
        if !valid_domain(domain) || !unique.insert(domain.as_str()) {
            return Err(ControlError);
        }
    }
    request(
        0x102a,
        0,
        "OidbSvcTrpcTcp.0x102a_0",
        None,
        &DomainTicketRequest {
            domains: domains.to_vec(),
        },
    )
}

/// Decodes domain tickets and binds every item to the original request order.
///
/// # Errors
///
/// Returns an error for rejection, missing, duplicate, unrequested, or unsafe tickets.
pub fn parse_domain_ticket_response(
    response: &[u8],
    requested: &[String],
) -> Result<Vec<DomainTicket>, ControlError> {
    if requested.is_empty()
        || requested.len() > MAX_DOMAINS
        || requested.iter().any(|value| !valid_domain(value))
    {
        return Err(ControlError);
    }
    let outer = decode_oidb_response(response).map_err(|_error| ControlError)?;
    if outer.error_code() != 0 {
        return Err(ControlError);
    }
    let body = DomainTicketResponse::decode(outer.body()).map_err(|_error| ControlError)?;
    if body.items.len() != requested.len() || body.items.len() > MAX_DOMAINS {
        return Err(ControlError);
    }
    let mut decoded = Vec::with_capacity(requested.len());
    for domain in requested {
        let mut matches = body.items.iter().filter(|item| item.domain == *domain);
        let item = matches.next().ok_or(ControlError)?;
        if matches.next().is_some() || item.value.is_empty() || item.value.len() > MAX_TICKET_BYTES
        {
            return Err(ControlError);
        }
        let value = std::str::from_utf8(&item.value).map_err(|_error| ControlError)?;
        if value.chars().any(char::is_control) {
            return Err(ControlError);
        }
        decoded.push(DomainTicket {
            domain: domain.clone(),
            value: value.to_owned(),
        });
    }
    Ok(decoded)
}

/// Encodes the 52194 QQ web `ClientKey` request.
///
/// # Errors
///
/// Returns an error only if the shared OIDB envelope rejects its fixed command.
pub fn client_key_request() -> Result<ControlRequest, ControlError> {
    Ok(ControlRequest {
        command: "OidbSvcTrpcTcp.0x102a_1",
        body: encode_empty_oidb_request(0x102a, 1, 0).map_err(|_error| ControlError)?,
        signing_operation: None,
    })
}

/// Decodes a bounded QQ web `ClientKey` response.
///
/// # Errors
///
/// Returns an error for rejection, empty or unsafe key material, or malformed protobuf.
pub fn parse_client_key_response(response: &[u8]) -> Result<(String, u32), ControlError> {
    let outer = decode_oidb_response(response).map_err(|_error| ControlError)?;
    if outer.error_code() != 0 {
        return Err(ControlError);
    }
    let body = ClientKeyResponse::decode(outer.body()).map_err(|_error| ControlError)?;
    if body.client_key.is_empty()
        || body.client_key.len() > MAX_CLIENT_KEY_BYTES
        || body.client_key.chars().any(char::is_control)
    {
        return Err(ControlError);
    }
    Ok((body.client_key, body.expiration))
}

fn valid_domain(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_DOMAIN_BYTES
        && !value.starts_with('.')
        && !value.ends_with('.')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
}

#[derive(Clone, PartialEq, Message)]
struct DomainTicketRequest {
    #[prost(string, repeated, tag = "1")]
    domains: Vec<String>,
}

#[derive(Clone, PartialEq, Message)]
struct DomainTicketResponse {
    #[prost(message, repeated, tag = "1")]
    items: Vec<TicketProperty>,
}

#[derive(Clone, PartialEq, Message)]
struct TicketProperty {
    #[prost(string, tag = "1")]
    domain: String,
    #[prost(bytes = "vec", tag = "2")]
    value: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
struct ClientKeyResponse {
    #[prost(uint32, tag = "2")]
    _kind: u32,
    #[prost(string, tag = "3")]
    client_key: String,
    #[prost(uint32, tag = "4")]
    expiration: u32,
}

#[cfg(test)]
mod tests {
    use qq_wire::{decode_oidb_request, encode_oidb_request};

    use super::*;

    #[test]
    fn ticket_request_and_response_are_bounded_and_ordered()
    -> Result<(), Box<dyn std::error::Error>> {
        let domains = vec!["qun.qq.com".to_owned(), "docs.qq.com".to_owned()];
        let request = domain_ticket_request(&domains)?;
        let outer = decode_oidb_request(request.body())?;
        assert_eq!((outer.command(), outer.subcommand()), (0x102a, 0));
        assert_eq!(DomainTicketRequest::decode(outer.body())?.domains, domains);

        let response = encode_oidb_request(
            0x102a,
            0,
            &DomainTicketResponse {
                items: vec![
                    TicketProperty {
                        domain: "docs.qq.com".to_owned(),
                        value: b"docs-ticket".to_vec(),
                    },
                    TicketProperty {
                        domain: "qun.qq.com".to_owned(),
                        value: b"qun-ticket".to_vec(),
                    },
                ],
            }
            .encode_to_vec(),
            0,
        )?;
        let tickets = parse_domain_ticket_response(&response, &domains)?;
        assert_eq!(tickets[0].value(), "qun-ticket");
        assert_eq!(tickets[1].value(), "docs-ticket");
        Ok(())
    }

    #[test]
    fn client_key_uses_empty_inner_message_and_rejects_empty_key()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = client_key_request()?;
        let outer = decode_oidb_request(request.body())?;
        assert_eq!((outer.command(), outer.subcommand()), (0x102a, 1));
        assert!(outer.body().is_empty());
        let response = encode_oidb_request(
            0x102a,
            1,
            &ClientKeyResponse {
                _kind: 1,
                client_key: String::new(),
                expiration: 0,
            }
            .encode_to_vec(),
            0,
        )?;
        assert!(parse_client_key_response(&response).is_err());
        Ok(())
    }
}

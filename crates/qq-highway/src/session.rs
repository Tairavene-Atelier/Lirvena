use std::net::Ipv4Addr;

use prost::Message;

use crate::HighwayError;
use crate::proto::{SessionOuter, SessionRequestWire, SessionResponseOuter};

const MAX_SESSION_BYTES: usize = 64 * 1024;
const MAX_SECRET_BYTES: usize = 8 * 1024;
const MAX_ENDPOINTS: usize = 32;

/// One bounded IPv4 upload endpoint returned by QQ.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct HighwayEndpoint {
    service_type: u32,
    address: Ipv4Addr,
    port: u16,
}

impl HighwayEndpoint {
    /// Returns the QQ service class associated with this endpoint.
    #[must_use]
    pub const fn service_type(self) -> u32 {
        self.service_type
    }

    /// Returns the validated IPv4 address.
    #[must_use]
    pub const fn address(self) -> Ipv4Addr {
        self.address
    }

    /// Returns the validated TCP port.
    #[must_use]
    pub const fn port(self) -> u16 {
        self.port
    }
}

/// An authenticated, bounded QQ upload session.
#[derive(Clone, Eq, PartialEq)]
pub struct HighwaySession {
    ticket: Vec<u8>,
    endpoints: Vec<HighwayEndpoint>,
}

impl HighwaySession {
    /// Returns the opaque QQ session ticket.
    #[must_use]
    pub fn ticket(&self) -> &[u8] {
        &self.ticket
    }

    /// Returns QQ-provided endpoints in preference order.
    #[must_use]
    pub fn endpoints(&self) -> &[HighwayEndpoint] {
        &self.endpoints
    }
}

impl core::fmt::Debug for HighwaySession {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("HighwaySession")
            .field("ticket", &"[REDACTED]")
            .field("endpoint_count", &self.endpoints.len())
            .finish()
    }
}

/// Encodes the audited 52194 upload-session request.
///
/// # Errors
///
/// Returns an error when the in-memory login ticket is empty or unbounded.
pub fn encode_session_request(tgt: &[u8]) -> Result<Vec<u8>, HighwayError> {
    if tgt.is_empty() || tgt.len() > MAX_SECRET_BYTES {
        return Err(HighwayError::InvalidInput);
    }
    Ok(SessionOuter {
        connection: Some(SessionRequestWire {
            field_1: 0,
            field_2: 0,
            field_3: 16,
            field_4: 1,
            tgt_hex: lower_hex(tgt),
            field_6: 3,
            service_types: vec![1, 5, 10, 21],
            field_9: 2,
            field_10: 9,
            field_11: 8,
            version: "1.0.1".to_owned(),
        }),
    }
    .encode_to_vec())
}

/// Decodes one bounded upload-session response.
///
/// # Errors
///
/// Returns an error for malformed responses, secrets outside their bounds, or
/// a response without a usable endpoint.
pub fn decode_session_response(input: &[u8]) -> Result<HighwaySession, HighwayError> {
    if input.is_empty() || input.len() > MAX_SESSION_BYTES {
        return Err(HighwayError::MalformedFrame);
    }
    let response = SessionResponseOuter::decode(input).map_err(|_| HighwayError::MalformedFrame)?;
    let connection = response.connection.ok_or(HighwayError::UnusableSession)?;
    if connection.ticket.is_empty()
        || connection.ticket.len() > MAX_SECRET_BYTES
        || connection.session_key.len() > MAX_SECRET_BYTES
    {
        return Err(HighwayError::UnusableSession);
    }
    let mut endpoints = Vec::new();
    for server in connection.servers {
        for candidate in server.addresses {
            if endpoints.len() == MAX_ENDPOINTS {
                break;
            }
            let address = Ipv4Addr::from(candidate.ipv4.to_le_bytes());
            let Ok(port) = u16::try_from(candidate.port) else {
                continue;
            };
            if port == 0 || !is_usable_endpoint(address) {
                continue;
            }
            let endpoint = HighwayEndpoint {
                service_type: server.service_type,
                address,
                port,
            };
            if !endpoints.contains(&endpoint) {
                endpoints.push(endpoint);
            }
        }
    }
    if endpoints.is_empty() {
        return Err(HighwayError::UnusableSession);
    }
    endpoints.sort_by_key(|endpoint| u8::from(endpoint.service_type != 1));
    Ok(HighwaySession {
        ticket: connection.ticket,
        endpoints,
    })
}

fn lower_hex(input: &[u8]) -> String {
    use core::fmt::Write as _;

    let mut output = String::with_capacity(input.len() * 2);
    for byte in input {
        let _ignored = write!(output, "{byte:02x}");
    }
    output
}

const fn is_usable_endpoint(address: Ipv4Addr) -> bool {
    let [a, b, c, d] = address.octets();
    !(a == 0
        || a == 127
        || a >= 224
        || (a == 169 && b == 254)
        || (a == 192 && b == 0 && c == 0)
        || (a == 192 && b == 0 && c == 2)
        || (a == 198 && (b == 18 || b == 19))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
        || (a == 255 && b == 255 && c == 255 && d == 255))
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::{decode_session_response, encode_session_request};
    use crate::proto::{
        ServerAddressWire, ServerInfoWire, SessionRequestWire, SessionResponseOuter,
        SessionResponseWire,
    };

    #[test]
    fn request_uses_lowercase_ticket_and_audited_slots() -> Result<(), Box<dyn std::error::Error>> {
        let encoded = encode_session_request(&[0x0a, 0xbc])?;
        let outer = crate::proto::SessionOuter::decode(encoded.as_slice())?;
        assert_eq!(
            outer.connection,
            Some(SessionRequestWire {
                field_1: 0,
                field_2: 0,
                field_3: 16,
                field_4: 1,
                tgt_hex: "0abc".to_owned(),
                field_6: 3,
                service_types: vec![1, 5, 10, 21],
                field_9: 2,
                field_10: 9,
                field_11: 8,
                version: "1.0.1".to_owned(),
            })
        );
        Ok(())
    }

    #[test]
    fn response_rejects_local_addresses_and_prefers_service_one()
    -> Result<(), Box<dyn std::error::Error>> {
        let encoded = SessionResponseOuter {
            connection: Some(SessionResponseWire {
                ticket: vec![7, 8],
                session_key: vec![9],
                servers: vec![
                    ServerInfoWire {
                        service_type: 5,
                        addresses: vec![ServerAddressWire {
                            address_type: 0,
                            ipv4: u32::from_le_bytes([8, 8, 8, 8]),
                            port: 80,
                            area: 0,
                        }],
                    },
                    ServerInfoWire {
                        service_type: 1,
                        addresses: vec![
                            ServerAddressWire {
                                address_type: 0,
                                ipv4: u32::from_le_bytes([127, 0, 0, 1]),
                                port: 80,
                                area: 0,
                            },
                            ServerAddressWire {
                                address_type: 0,
                                ipv4: u32::from_le_bytes([1, 1, 1, 1]),
                                port: 443,
                                area: 0,
                            },
                        ],
                    },
                ],
            }),
        }
        .encode_to_vec();

        let session = decode_session_response(&encoded)?;
        assert_eq!(session.ticket(), &[7, 8]);
        assert_eq!(session.endpoints()[0].service_type(), 1);
        assert_eq!(session.endpoints()[0].address().octets(), [1, 1, 1, 1]);
        Ok(())
    }
}

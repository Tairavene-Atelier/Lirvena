use std::io::{Read as _, Write as _};

use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use prost::Message;

use crate::MessageDecodeError;

const MAX_RICH_BYTES: usize = 1024 * 1024;

pub(super) fn compress(input: &str) -> Result<Vec<u8>, MessageDecodeError> {
    validate_text(input)?;
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    encoder
        .write_all(input.as_bytes())
        .map_err(|_error| MessageDecodeError)?;
    let compressed = encoder.finish().map_err(|_error| MessageDecodeError)?;
    if compressed.len() + 1 > MAX_RICH_BYTES {
        return Err(MessageDecodeError);
    }
    let mut framed = Vec::with_capacity(compressed.len() + 1);
    framed.push(1);
    framed.extend_from_slice(&compressed);
    Ok(framed)
}

pub(super) fn decompress(input: &[u8]) -> Result<String, MessageDecodeError> {
    if input.len() < 2 || input.len() > MAX_RICH_BYTES || input[0] != 1 {
        return Err(MessageDecodeError);
    }
    let limit = u64::try_from(MAX_RICH_BYTES).map_err(|_error| MessageDecodeError)? + 1;
    let mut decoded = Vec::new();
    ZlibDecoder::new(&input[1..])
        .take(limit)
        .read_to_end(&mut decoded)
        .map_err(|_error| MessageDecodeError)?;
    if decoded.len() > MAX_RICH_BYTES {
        return Err(MessageDecodeError);
    }
    let text = String::from_utf8(decoded).map_err(|_error| MessageDecodeError)?;
    validate_text(&text)?;
    Ok(text)
}

pub(super) fn encode_poke(kind: u32, strength: u32) -> Vec<u8> {
    PokeWire { kind, strength }.encode_to_vec()
}

pub(super) fn decode_poke(input: &[u8]) -> Result<(u32, u32), MessageDecodeError> {
    if input.is_empty() || input.len() > MAX_RICH_BYTES {
        return Err(MessageDecodeError);
    }
    let poke = PokeWire::decode(input).map_err(|_error| MessageDecodeError)?;
    if poke.kind == 0 {
        return Err(MessageDecodeError);
    }
    Ok((poke.kind, poke.strength))
}

fn validate_text(input: &str) -> Result<(), MessageDecodeError> {
    if input.is_empty() || input.len() > MAX_RICH_BYTES || input.contains('\0') {
        Err(MessageDecodeError)
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Message)]
struct PokeWire {
    #[prost(uint32, tag = "1")]
    kind: u32,
    #[prost(uint32, tag = "7")]
    strength: u32,
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use flate2::Compression;
    use flate2::write::ZlibEncoder;

    use super::{compress, decode_poke, decompress, encode_poke};

    #[test]
    fn rich_payload_round_trips_with_fixed_header() -> Result<(), Box<dyn std::error::Error>> {
        let payload = compress("{\"app\":\"demo\"}")?;
        assert_eq!(&payload[..3], &[1, 0x78, 0xda]);
        assert_eq!(decompress(&payload)?, "{\"app\":\"demo\"}");
        Ok(())
    }

    #[test]
    fn decompression_is_bounded_and_header_is_closed() -> Result<(), Box<dyn std::error::Error>> {
        assert!(decompress(&[0, 1]).is_err());
        let oversized = "x".repeat(1024 * 1024 + 1);
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
        encoder.write_all(oversized.as_bytes())?;
        let mut compressed = vec![1];
        compressed.extend(encoder.finish()?);
        assert!(decompress(&compressed).is_err());
        Ok(())
    }

    #[test]
    fn poke_fields_match_the_compiled_wire_shape() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(decode_poke(&encode_poke(2, 7))?, (2, 7));
        Ok(())
    }
}

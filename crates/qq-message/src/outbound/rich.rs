use std::io::Write as _;

use flate2::Compression;
use flate2::write::ZlibEncoder;
use prost::Message;

use crate::MessageDecodeError;

const MAX_RICH_INPUT_BYTES: usize = 1024 * 1024;
const MAX_RICH_OUTPUT_BYTES: usize = 1024 * 1024;

pub(super) fn compressed_payload(input: &str) -> Result<Vec<u8>, MessageDecodeError> {
    if input.is_empty() || input.len() > MAX_RICH_INPUT_BYTES || input.contains('\0') {
        return Err(MessageDecodeError);
    }
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    encoder
        .write_all(input.as_bytes())
        .map_err(|_error| MessageDecodeError)?;
    let compressed = encoder.finish().map_err(|_error| MessageDecodeError)?;
    if compressed.len() + 1 > MAX_RICH_OUTPUT_BYTES {
        return Err(MessageDecodeError);
    }
    let mut framed = Vec::with_capacity(compressed.len() + 1);
    framed.push(1);
    framed.extend_from_slice(&compressed);
    Ok(framed)
}

pub(super) fn poke_payload(kind: u32, strength: u32) -> Vec<u8> {
    PokeExtra { kind, strength }.encode_to_vec()
}

#[derive(Clone, Copy, PartialEq, Message)]
struct PokeExtra {
    #[prost(uint32, tag = "1")]
    kind: u32,
    #[prost(uint32, tag = "7")]
    strength: u32,
}

#[cfg(test)]
mod tests {
    use std::io::Read as _;

    use flate2::read::ZlibDecoder;
    use prost::Message;

    use super::{PokeExtra, compressed_payload, poke_payload};

    #[test]
    fn rich_payload_uses_one_byte_header_and_zlib() -> Result<(), Box<dyn std::error::Error>> {
        let payload = compressed_payload("{\"app\":\"demo\"}")?;
        assert_eq!(&payload[..3], &[1, 0x78, 0xda]);
        let mut decoded = String::new();
        ZlibDecoder::new(&payload[1..]).read_to_string(&mut decoded)?;
        assert_eq!(decoded, "{\"app\":\"demo\"}");
        Ok(())
    }

    #[test]
    fn poke_fields_match_the_compiled_wire_shape() -> Result<(), Box<dyn std::error::Error>> {
        let decoded = PokeExtra::decode(poke_payload(2, 7).as_slice())?;
        assert_eq!((decoded.kind, decoded.strength), (2, 7));
        Ok(())
    }
}

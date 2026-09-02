use crate::OnlinePacketError;

use super::jce::{MAP, Reader, STRUCT_BEGIN, STRUCT_END, Writer, named_payload};
const MAX_ENVELOPE_LEN: usize = 1024 * 1024;

pub(super) fn build_ack(input: &[u8]) -> Result<Vec<u8>, OnlinePacketError> {
    if input.is_empty() || input.len() > MAX_ENVELOPE_LEN {
        return Err(OnlinePacketError);
    }
    let request = parse_outer(input)?;

    let mut push = Writer::new();
    push.head(0, STRUCT_BEGIN);
    push.integer(1, request.push_type);
    push.integer(2, request.sequence);
    push.head(0, STRUCT_END);

    let mut wup = Writer::new();
    wup.head(0, MAP);
    wup.integer(0, 1);
    wup.string(0, "PushResp")?;
    wup.head(1, MAP);
    wup.integer(0, 1);
    wup.string(0, "ConfigPush.PushResp")?;
    wup.bytes(1, &push.finish())?;

    let mut output = Writer::new();
    output.integer(
        1,
        if request.version == 0 {
            2
        } else {
            request.version
        },
    );
    output.integer(2, request.packet_type);
    output.integer(3, request.message_type);
    output.integer(4, request.request_id);
    output.string(5, &request.servant)?;
    output.string(6, "PushResp")?;
    output.bytes(7, &wup.finish())?;
    output.integer(8, 0);
    output.empty_map(9);
    output.empty_map(10);
    Ok(output.finish())
}

struct ConfigEnvelope {
    version: i64,
    packet_type: i64,
    message_type: i64,
    request_id: i64,
    servant: String,
    push_type: i64,
    sequence: i64,
}

fn parse_outer(input: &[u8]) -> Result<ConfigEnvelope, OnlinePacketError> {
    let mut reader = Reader::new(input);
    let mut request = ConfigEnvelope {
        version: 0,
        packet_type: 0,
        message_type: 0,
        request_id: 0,
        servant: String::new(),
        push_type: 0,
        sequence: 0,
    };
    let mut wup = None;
    while reader.remaining() != 0 {
        let head = reader.head()?;
        match head.tag {
            1 => request.version = reader.integer(head.kind)?,
            2 => request.packet_type = reader.integer(head.kind)?,
            3 => request.message_type = reader.integer(head.kind)?,
            4 => request.request_id = reader.integer(head.kind)?,
            5 => request.servant = reader.string(head.kind)?,
            7 => wup = Some(reader.bytes(head.kind)?),
            _ => reader.skip(head.kind, 0)?,
        }
    }
    let payload = named_payload(
        wup.ok_or(OnlinePacketError)?,
        "PushReq",
        "ConfigPush.PushReq",
    )?;
    let (push_type, sequence) = parse_push(payload)?;
    request.push_type = push_type;
    request.sequence = sequence;
    Ok(request)
}

fn parse_push(input: &[u8]) -> Result<(i64, i64), OnlinePacketError> {
    let mut reader = Reader::new(input);
    if reader.head()?.kind != STRUCT_BEGIN {
        return Err(OnlinePacketError);
    }
    let mut push_type = 0;
    let mut sequence = 0;
    while reader.remaining() != 0 {
        let head = reader.head()?;
        if head.kind == STRUCT_END {
            return Ok((push_type, sequence));
        }
        match head.tag {
            1 => push_type = reader.integer(head.kind)?,
            3 => sequence = reader.integer(head.kind)?,
            _ => reader.skip(head.kind, 0)?,
        }
    }
    Err(OnlinePacketError)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_config_ack_preserves_correlation() -> Result<(), OnlinePacketError> {
        let mut push = Writer::new();
        push.head(0, STRUCT_BEGIN);
        push.integer(1, 1);
        push.bytes(2, b"ignored")?;
        push.integer(3, 42);
        push.head(0, STRUCT_END);
        let mut wup = Writer::new();
        wup.head(0, MAP);
        wup.integer(0, 1);
        wup.string(0, "PushReq")?;
        wup.head(1, MAP);
        wup.integer(0, 1);
        wup.string(0, "ConfigPush.PushReq")?;
        wup.bytes(1, &push.finish())?;
        let mut outer = Writer::new();
        outer.integer(1, 3);
        outer.integer(4, 9);
        outer.string(5, "ConfigPushSvc")?;
        outer.bytes(7, &wup.finish())?;
        let ack = build_ack(&outer.finish())?;
        assert!(!ack.is_empty());
        assert_eq!(build_ack(&[]), Err(OnlinePacketError));
        Ok(())
    }
}

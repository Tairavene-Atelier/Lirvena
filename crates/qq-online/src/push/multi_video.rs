use crate::OnlinePacketError;

use super::jce::{MAP, Reader, STRUCT_BEGIN, STRUCT_END, Writer, named_payload};

const MAX_ENVELOPE_LEN: usize = 1024 * 1024;
const MAX_ACK_LEN: usize = MAX_ENVELOPE_LEN + 4 * 1024;
const MAX_RECIPIENTS: usize = 64;
const MAX_VIDEO_BUFFER_LEN: usize = 512 * 1024;

pub(super) fn build_ack(input: &[u8]) -> Result<Option<Vec<u8>>, OnlinePacketError> {
    if input.is_empty() || input.len() > MAX_ENVELOPE_LEN {
        return Err(OnlinePacketError);
    }
    let envelope = parse_envelope(input)?;
    if envelope.message.type_code >= 32 || envelope.message.type_code == 10 {
        return Ok(None);
    }
    let output = encode_ack(&envelope)?;
    if output.len() > MAX_ACK_LEN {
        return Err(OnlinePacketError);
    }
    Ok(Some(output))
}

struct Envelope {
    version: i32,
    packet_type: i32,
    message_type: i32,
    request_id: i32,
    servant: String,
    function: String,
    message: Message,
}

struct Message {
    version: u8,
    type_code: u8,
    command: i16,
    sender: i64,
    recipients: Vec<i64>,
    video_buffer: Vec<u8>,
    subcommand: i16,
    uid: i64,
    sequence: i64,
    content_type: i64,
    timestamp: i64,
    data_flag: i64,
}

fn parse_envelope(input: &[u8]) -> Result<Envelope, OnlinePacketError> {
    let mut reader = Reader::new(input);
    let mut version = 0;
    let mut packet_type = 0;
    let mut message_type = 0;
    let mut request_id = 0;
    let mut servant = String::new();
    let mut function = String::new();
    let mut wup = None;
    while reader.remaining() != 0 {
        let head = reader.head()?;
        match head.tag {
            1 => version = checked_i32(reader.integer(head.kind)?)?,
            2 => packet_type = checked_i32(reader.integer(head.kind)?)?,
            3 => message_type = checked_i32(reader.integer(head.kind)?)?,
            4 => request_id = checked_i32(reader.integer(head.kind)?)?,
            5 => servant = reader.string(head.kind)?,
            6 => function = reader.string(head.kind)?,
            7 => wup = Some(reader.bytes(head.kind)?),
            _ => reader.skip(head.kind, 0)?,
        }
    }
    let body = named_payload(
        wup.ok_or(OnlinePacketError)?,
        "MultiVideoMsg",
        "SharpSvrPack.MultiVideoMsg",
    )?;
    Ok(Envelope {
        version,
        packet_type,
        message_type,
        request_id,
        servant,
        function,
        message: parse_message(body)?,
    })
}

fn parse_message(input: &[u8]) -> Result<Message, OnlinePacketError> {
    let mut reader = Reader::new(input);
    if reader.head()?.kind != STRUCT_BEGIN {
        return Err(OnlinePacketError);
    }
    let mut message = Message {
        version: 0,
        type_code: 0,
        command: 0,
        sender: 0,
        recipients: Vec::new(),
        video_buffer: Vec::new(),
        subcommand: 0,
        uid: 0,
        sequence: 0,
        content_type: 0,
        timestamp: 0,
        data_flag: 0,
    };
    while reader.remaining() != 0 {
        let head = reader.head()?;
        if head.kind == STRUCT_END {
            return Ok(message);
        }
        match head.tag {
            0 => message.version = checked_u8(reader.integer(head.kind)?)?,
            1 => message.type_code = checked_u8(reader.integer(head.kind)?)?,
            2 => message.command = checked_i16(reader.integer(head.kind)?)?,
            3 => message.sender = reader.integer(head.kind)?,
            4 => message.recipients = reader.integer_list(head.kind, MAX_RECIPIENTS)?,
            5 => {
                let value = reader.bytes(head.kind)?;
                if value.len() > MAX_VIDEO_BUFFER_LEN {
                    return Err(OnlinePacketError);
                }
                message.video_buffer = value.to_vec();
            }
            6 => message.subcommand = checked_i16(reader.integer(head.kind)?)?,
            7 => message.uid = reader.integer(head.kind)?,
            8 => message.sequence = reader.integer(head.kind)?,
            9 => message.content_type = reader.integer(head.kind)?,
            10 => message.timestamp = reader.integer(head.kind)?,
            11 => message.data_flag = reader.integer(head.kind)?,
            _ => reader.skip(head.kind, 0)?,
        }
    }
    Err(OnlinePacketError)
}

fn encode_ack(envelope: &Envelope) -> Result<Vec<u8>, OnlinePacketError> {
    let message = &envelope.message;
    let mut body = Writer::new();
    body.head(0, STRUCT_BEGIN);
    body.integer(0, i64::from(message.version));
    body.integer(1, i64::from(message.type_code));
    body.integer(2, i64::from(message.command));
    body.integer(3, message.sender);
    body.integer_list(4, message.recipients.get(..1).unwrap_or_default())?;
    body.bytes(5, &message.video_buffer)?;
    body.integer(6, i64::from(message.subcommand));
    body.integer(7, message.uid);
    body.integer(8, message.sequence);
    body.integer(9, message.content_type);
    body.integer(10, message.timestamp);
    body.integer(11, message.data_flag);
    body.head(0, STRUCT_END);

    let mut wup = Writer::new();
    wup.head(0, MAP);
    wup.integer(0, 1);
    wup.string(0, "MultiVideoMsg")?;
    wup.head(1, MAP);
    wup.integer(0, 1);
    wup.string(0, "SharpSvrPack.MultiVideoMsg")?;
    wup.bytes(1, &body.finish())?;

    let mut output = Writer::new();
    output.integer(
        1,
        if envelope.version == 0 {
            2
        } else {
            i64::from(envelope.version)
        },
    );
    output.integer(2, i64::from(envelope.packet_type));
    output.integer(3, i64::from(envelope.message_type));
    output.integer(4, i64::from(envelope.request_id));
    output.string(
        5,
        if envelope.servant.is_empty() {
            "MultiVideo"
        } else {
            &envelope.servant
        },
    )?;
    output.string(
        6,
        if envelope.function.is_empty() {
            "MultiVideoMsg"
        } else {
            &envelope.function
        },
    )?;
    output.bytes(7, &wup.finish())?;
    output.integer(8, 0);
    output.empty_map(9);
    output.empty_map(10);
    Ok(output.finish())
}

fn checked_u8(value: i64) -> Result<u8, OnlinePacketError> {
    u8::try_from(value).map_err(|_| OnlinePacketError)
}

fn checked_i16(value: i64) -> Result<i16, OnlinePacketError> {
    i16::try_from(value).map_err(|_| OnlinePacketError)
}

fn checked_i32(value: i64) -> Result<i32, OnlinePacketError> {
    i32::try_from(value).map_err(|_| OnlinePacketError)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_ten_is_consumed_and_supported_type_is_acknowledged() -> Result<(), OnlinePacketError> {
        assert!(build_ack(&request(10)?)?.is_none());
        let ack = build_ack(&request(11)?)?.ok_or(OnlinePacketError)?;
        let parsed = parse_envelope(&ack)?;
        assert_eq!(parsed.request_id, 9);
        assert_eq!(parsed.message.type_code, 11);
        assert_eq!(parsed.message.recipients, vec![42]);
        assert_eq!(parsed.message.video_buffer, b"video");
        Ok(())
    }

    #[test]
    fn malformed_and_out_of_range_messages_fail_closed() -> Result<(), OnlinePacketError> {
        assert_eq!(build_ack(&[]), Err(OnlinePacketError));
        assert_eq!(build_ack(&request(32)?), Ok(None));

        let valid = request(11)?;
        for end in 0..valid.len() {
            assert!(build_ack(&valid[..end]).is_err());
        }

        let mut message = Writer::new();
        message.head(0, STRUCT_BEGIN);
        message.integer(1, 256);
        message.head(0, STRUCT_END);
        assert!(build_ack(&envelope(&message.finish())?).is_err());
        Ok(())
    }

    fn request(message_type: i64) -> Result<Vec<u8>, OnlinePacketError> {
        let mut message = Writer::new();
        message.head(0, STRUCT_BEGIN);
        message.integer(0, 1);
        message.integer(1, message_type);
        message.integer(2, 3);
        message.integer(3, 7);
        message.integer_list(4, &[42, 43])?;
        message.bytes(5, b"video")?;
        message.integer(6, 4);
        message.integer(7, 5);
        message.integer(8, 6);
        message.integer(9, 7);
        message.integer(10, 8);
        message.integer(11, 9);
        message.head(0, STRUCT_END);
        envelope(&message.finish())
    }

    fn envelope(message: &[u8]) -> Result<Vec<u8>, OnlinePacketError> {
        let mut wup = Writer::new();
        wup.head(0, MAP);
        wup.integer(0, 1);
        wup.string(0, "MultiVideoMsg")?;
        wup.head(1, MAP);
        wup.integer(0, 1);
        wup.string(0, "SharpSvrPack.MultiVideoMsg")?;
        wup.bytes(1, message)?;

        let mut outer = Writer::new();
        outer.integer(1, 3);
        outer.integer(4, 9);
        outer.string(5, "MultiVideo")?;
        outer.string(6, "MultiVideoMsg")?;
        outer.bytes(7, &wup.finish())?;
        Ok(outer.finish())
    }
}

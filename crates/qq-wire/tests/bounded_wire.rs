//! Bounded big-endian QQ wire primitive tests.

use qq_wire::{LengthPrefix, WireError, WireReader, WireWriter};

#[test]
fn primitives_and_prefix_modes_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    let mut writer = WireWriter::new(128);
    writer.put_u8(7)?;
    writer.put_u16(0x1234)?;
    writer.put_u32(0x5566_7788)?;
    writer.put_u64(0x0102_0304_0506_0708)?;
    writer.put_prefixed_bytes(LengthPrefix::U16Inclusive, b"hello")?;
    writer.put_prefixed_bytes(LengthPrefix::U32Payload, b"world")?;

    let packet = writer.finish();
    let mut reader = WireReader::new(&packet);
    assert_eq!(reader.read_u8()?, 7);
    assert_eq!(reader.read_u16()?, 0x1234);
    assert_eq!(reader.read_u32()?, 0x5566_7788);
    assert_eq!(reader.read_u64()?, 0x0102_0304_0506_0708);
    assert_eq!(
        reader.read_prefixed_bytes(LengthPrefix::U16Inclusive, 5)?,
        b"hello"
    );
    assert_eq!(
        reader.read_prefixed_bytes(LengthPrefix::U32Payload, 5)?,
        b"world"
    );
    reader.finish()?;
    Ok(())
}

#[test]
fn reader_rejects_truncation_trailing_bytes_and_invalid_inclusive_length() {
    let mut truncated = WireReader::new(&[0, 3, 1]);
    assert!(matches!(
        truncated.read_prefixed_bytes(LengthPrefix::U16Payload, 8),
        Err(WireError::Truncated { .. })
    ));

    let trailing = WireReader::new(&[1]);
    assert!(matches!(
        trailing.finish(),
        Err(WireError::TrailingBytes { remaining: 1 })
    ));

    let mut invalid = WireReader::new(&[0, 1]);
    assert_eq!(
        invalid.read_prefixed_bytes(LengthPrefix::U16Inclusive, 8),
        Err(WireError::InvalidInclusiveLength)
    );
}

#[test]
fn writer_is_unchanged_after_bound_rejection() -> Result<(), Box<dyn std::error::Error>> {
    let mut writer = WireWriter::new(2);
    writer.put_u16(7)?;
    assert!(matches!(
        writer.put_u8(1),
        Err(WireError::LengthLimitExceeded { .. })
    ));
    assert_eq!(writer.finish(), vec![0, 7]);
    Ok(())
}

//! Regression vectors for bounded QQ message read reports.

use qq_message::{ReadReportInput, encode_read_report, validate_read_report_response};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn group_read_report_matches_the_compiled_wire_shape() -> TestResult {
    let encoded = encode_read_report(ReadReportInput::Group {
        group_uin: 42,
        sequence: 55,
    })?;
    assert_eq!(encoded, [0x0a, 0x04, 0x08, 0x2a, 0x10, 0x37]);
    Ok(())
}

#[test]
fn private_read_report_matches_the_compiled_wire_shape() -> TestResult {
    let encoded = encode_read_report(ReadReportInput::Private {
        target_uid: "u_peer",
        timestamp: 100,
        sequence: 55,
    })?;
    assert_eq!(
        encoded,
        [
            0x12, 0x0c, 0x12, 0x06, 0x75, 0x5f, 0x70, 0x65, 0x65, 0x72, 0x18, 0x64, 0x20, 0x37,
        ]
    );
    Ok(())
}

#[test]
fn read_report_rejects_incomplete_or_out_of_generation_correlations() {
    assert!(
        encode_read_report(ReadReportInput::Group {
            group_uin: 0,
            sequence: 1,
        })
        .is_err()
    );
    assert!(
        encode_read_report(ReadReportInput::Group {
            group_uin: 1,
            sequence: u64::from(u32::MAX) + 1,
        })
        .is_err()
    );
    assert!(
        encode_read_report(ReadReportInput::Private {
            target_uid: "",
            timestamp: 1,
            sequence: 1,
        })
        .is_err()
    );
}

#[test]
fn read_report_response_accepts_canonical_success_and_rejects_failure() -> TestResult {
    validate_read_report_response(&[])?;
    validate_read_report_response(&[0x08, 0x00])?;
    assert!(validate_read_report_response(&[0x18, 0x01]).is_err());
    assert!(validate_read_report_response(&[0x08, 0x05, 0x12, 0x03, b'b', b'a', b'd']).is_err());
    assert!(validate_read_report_response(&vec![0; 64 * 1024 + 1]).is_err());
    Ok(())
}

//! Process-level smoke tests for the Lirvena binary.

use std::error::Error;
use std::process::Command;

#[test]
fn binary_reports_product_identity() -> Result<(), Box<dyn Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_lirvena")).output()?;

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(String::from_utf8(output.stdout)?, "Lirvena 0.0.0\n");
    Ok(())
}

//! Compiles the public Ceylith v2 protobuf schema.

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let schema = "../../schemas/ceylith/v2.proto";
    println!("cargo:rerun-if-changed={schema}");

    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    let mut config = prost_build::Config::new();
    config.protoc_executable(protoc);
    config.compile_protos(&[schema], &["../../schemas"])?;
    Ok(())
}

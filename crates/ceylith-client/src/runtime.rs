use ceylith_protocol::{CodecError, Digest32, proto, validate_client_runtime};

use crate::ClientError;

/// Closed platform identifier compiled into Lirvena.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Platform {
    /// Linux userspace.
    Linux = 1,
}

/// Closed architecture identifier compiled into Lirvena.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Architecture {
    /// 64-bit x86.
    X86_64 = 1,
    /// 64-bit Arm.
    Aarch64 = 2,
}

/// Validated public runtime advertisement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDescriptor {
    wire: proto::ClientRuntime,
}

impl RuntimeDescriptor {
    /// Creates one runtime descriptor with closed platform values.
    ///
    /// # Errors
    ///
    /// Returns an error when the ABI, contracts, platform, architecture, or digest is invalid.
    pub fn new(
        runtime_abi: u32,
        envelope_contract: u32,
        action_contracts: Vec<u32>,
        source_contracts: Vec<u32>,
        platform: Platform,
        architecture: Architecture,
        build_digest: Digest32,
    ) -> Result<Self, ClientError> {
        let wire = proto::ClientRuntime {
            runtime_abi,
            envelope_contract,
            action_contracts,
            source_contracts,
            platform: platform as u32,
            architecture: architecture as u32,
            build_digest: build_digest.as_bytes().to_vec(),
        };
        validate_client_runtime(&wire).map_err(|_: CodecError| ClientError::Protocol)?;
        Ok(Self { wire })
    }

    /// Borrows the canonical wire representation.
    #[must_use]
    pub const fn as_wire(&self) -> &proto::ClientRuntime {
        &self.wire
    }
}

use ceylith_client::InstallationClient;
use ceylith_protocol::AccountSlotId;
use qq_envelope::SessionAuth;
use qq_login::{CredentialExchangeRequest, QrDevice, QrUnsignedRequest};
use qq_profile::LinuxNtProfile;
use qq_transport::QqTransport;
use tokio::net::TcpStream;

use super::ceylith::{OpaqueOperation, request_reserve};
use crate::qq::{QqRequest, execute_anonymous};
use crate::support::encode_hex;

pub(super) async fn execute_request(
    ceylith: &InstallationClient,
    qq: &mut QqTransport<TcpStream>,
    profile: &LinuxNtProfile,
    device: &QrDevice,
    account_slot_id: AccountSlotId,
    operation: OpaqueOperation,
    unsigned: &impl UnsignedQqRequest,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let reserve = request_reserve(ceylith, account_slot_id, operation, unsigned.payload()).await?;
    let device_guid_hex = encode_hex(device.guid());
    let auth = SessionAuth::anonymous(unsigned.uin());
    execute_anonymous(
        qq,
        profile,
        QqRequest {
            auth: &auth,
            sequence: unsigned.sequence(),
            locale_id: 2_052,
            command: unsigned.command(),
            device_guid_hex: device_guid_hex.as_bytes(),
            reserve: &reserve,
            payload: unsigned.payload(),
        },
    )
    .await
}

pub(super) trait UnsignedQqRequest {
    fn sequence(&self) -> u32;
    fn uin(&self) -> u32;
    fn command(&self) -> &str;
    fn payload(&self) -> &[u8];
}

impl UnsignedQqRequest for QrUnsignedRequest {
    fn sequence(&self) -> u32 {
        self.sequence()
    }

    fn uin(&self) -> u32 {
        0
    }

    fn command(&self) -> &str {
        self.command()
    }

    fn payload(&self) -> &[u8] {
        self.payload()
    }
}

impl UnsignedQqRequest for CredentialExchangeRequest {
    fn sequence(&self) -> u32 {
        self.sequence()
    }

    fn uin(&self) -> u32 {
        self.uin()
    }

    fn command(&self) -> &str {
        self.command()
    }

    fn payload(&self) -> &[u8] {
        self.payload()
    }
}

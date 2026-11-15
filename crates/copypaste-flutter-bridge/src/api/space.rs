//! The device space: membership, cloud key state, leaving and passphrase.
use copypaste_ipc::{Method, ResponseData};

use super::{CloudAccountStatus, RuntimeError};
use crate::client;

#[derive(Debug, Clone)]
pub struct SpaceStatus {
    /// False after another device removed this one, or after it left.
    pub member: bool,
    pub key_epoch: Option<u64>,
    pub passphrase_recovery: bool,
    /// Pairings of members that have not yet received the current cloud key.
    pub waiting_pairing_ids: Vec<String>,
}

async fn space_response(method: Method) -> Result<SpaceStatus, RuntimeError> {
    let response = client::request(method).await?;
    match response.data {
        Some(ResponseData::Space(space)) => Ok(SpaceStatus {
            member: space.member,
            key_epoch: space.key_epoch,
            passphrase_recovery: space.passphrase_recovery,
            waiting_pairing_ids: space.waiting_pairing_ids,
        }),
        _ => Err(RuntimeError::internal()),
    }
}

pub async fn space_status() -> Result<SpaceStatus, RuntimeError> {
    space_response(Method::Space).await
}

pub async fn leave_space() -> Result<SpaceStatus, RuntimeError> {
    space_response(Method::SpaceLeave).await
}

pub async fn cloud_change_passphrase(
    passphrase: String,
) -> Result<CloudAccountStatus, RuntimeError> {
    super::cloud_status_response(Method::CloudChangePassphrase { passphrase }).await
}

pub async fn cloud_sign_out_others() -> Result<CloudAccountStatus, RuntimeError> {
    super::cloud_status_response(Method::CloudSignOutOthers).await
}

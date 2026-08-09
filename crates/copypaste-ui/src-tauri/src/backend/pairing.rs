use copypaste_ipc::{PairingInviteData, PairingProgressData};

use super::Result;

#[allow(async_fn_in_trait)]
pub trait PairingBackend: Send + Sync + 'static {
    async fn pair_create_invite(&self) -> Result<PairingInviteData>;
    async fn pair_join(&self, code: &str, addr: &str) -> Result<PairingProgressData>;
    async fn pair_progress(&self) -> Result<PairingProgressData>;
    async fn pair_confirm(&self, accept: bool) -> Result<PairingProgressData>;
    async fn pair_cancel(&self) -> Result<PairingProgressData>;
}

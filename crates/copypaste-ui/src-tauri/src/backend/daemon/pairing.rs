use copypaste_ipc::{Method, PairingInviteData, PairingProgressData};

use super::response::{expect_pairing_invite, expect_pairing_progress};
use super::DaemonBackend;
use crate::backend::{PairingBackend, Result};

impl PairingBackend for DaemonBackend {
    async fn pair_create_invite(&self) -> Result<PairingInviteData> {
        expect_pairing_invite(self.call(Method::PairCreateInvite).await?)
    }

    async fn pair_join(&self, code: &str, addr: &str) -> Result<PairingProgressData> {
        expect_pairing_progress(
            self.call(Method::PairJoin {
                code: code.to_string(),
                addr: addr.to_string(),
            })
            .await?,
        )
    }

    async fn pair_progress(&self) -> Result<PairingProgressData> {
        expect_pairing_progress(self.call(Method::PairProgress).await?)
    }

    async fn pair_confirm(&self, accept: bool) -> Result<PairingProgressData> {
        expect_pairing_progress(self.call(Method::PairConfirm { accept }).await?)
    }

    async fn pair_cancel(&self) -> Result<PairingProgressData> {
        expect_pairing_progress(self.call(Method::PairCancel).await?)
    }
}

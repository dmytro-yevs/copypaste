use copypaste_ipc::{PairingInviteData, PairingProgressData};

use super::EmbeddedBackend;
use crate::backend::{BackendError, PairingBackend, Result};

impl PairingBackend for EmbeddedBackend {
    async fn pair_create_invite(&self) -> Result<PairingInviteData> {
        if !self.inner.settings().sync_enabled {
            return Err(BackendError::NotReady);
        }
        self.node().await?.pair_create_invite()
    }

    async fn pair_join(&self, code: &str, addr: &str) -> Result<PairingProgressData> {
        if !self.inner.settings().sync_enabled {
            return Err(BackendError::NotReady);
        }
        self.node().await?.pair_join(&self.inner, code, addr).await
    }

    async fn pair_progress(&self) -> Result<PairingProgressData> {
        Ok(self.node().await?.pair_progress(&self.inner))
    }

    async fn pair_confirm(&self, accept: bool) -> Result<PairingProgressData> {
        if accept && !self.inner.settings().sync_enabled {
            self.node().await?.pair_cancel(&self.inner);
            return Err(BackendError::NotReady);
        }
        self.node().await?.pair_confirm(&self.inner, accept)
    }

    async fn pair_cancel(&self) -> Result<PairingProgressData> {
        Ok(self.node().await?.pair_cancel(&self.inner))
    }
}

use std::path::Path;
use std::sync::Arc;

use tokio::sync::OnceCell;

use super::peers::PeerNode;
use super::state::BackendState;
use super::{BackendError, EmbeddedBackend, Inner, Result};

pub trait Clipboard: Send + Sync + 'static {
    fn set_text(&self, text: &str) -> std::result::Result<(), &'static str>;
}

impl EmbeddedBackend {
    pub fn open(data_dir: &Path, clipboard: Box<dyn Clipboard>) -> Result<Self> {
        Ok(Self {
            inner: Arc::new(Inner {
                state: BackendState::open(data_dir)?,
                node: OnceCell::new(),
                clipboard,
            }),
        })
    }

    pub(super) async fn node(&self) -> Result<&PeerNode> {
        self.inner
            .node
            .get_or_try_init(|| PeerNode::start(&self.inner))
            .await
    }

    pub(super) async fn blocking<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Inner) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let inner = Arc::clone(&self.inner);
        tokio::task::spawn_blocking(move || f(&inner))
            .await
            .map_err(|_| BackendError::internal("the operation did not complete"))?
    }
}

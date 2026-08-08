use std::path::Path;
use std::sync::Arc;

use copypaste_ipc::{EventData, EventKind};
use tokio::sync::OnceCell;

use super::cloud::EmbeddedCloud;
use super::peers::PeerNode;
use super::state::BackendState;
use super::{BackendError, Result};

/// Everything the in-process backend owns. Cheap to clone; `Store` is a
/// reference-counted pool and the rest is behind an `Arc`.
#[derive(Clone)]
pub struct EmbeddedBackend {
    pub(super) inner: Arc<Inner>,
}

pub(super) struct Inner {
    pub(super) state: BackendState,
    /// The peer node, brought up on the first operation that needs it — see
    /// [`super::peers`] for why it is not built in [`EmbeddedBackend::open`].
    pub(super) node: OnceCell<PeerNode>,
    /// Writes the system clipboard. Held as a boxed trait object so this file
    /// does not depend on Tauri's plugin types, and so tests can substitute
    /// one — see `Clipboard`.
    pub(super) clipboard: Box<dyn Clipboard>,
    pub(super) events: tokio::sync::broadcast::Sender<copypaste_ipc::EventData>,
    pub(super) cloud: EmbeddedCloud,
}

impl Inner {
    pub(super) fn settings(&self) -> copypaste_ipc::ConfigData {
        self.state.settings.snapshot().config
    }

    pub(super) fn items_event(&self, captured: bool, swept: u32) -> EventData {
        EventData {
            event: EventKind::Items,
            item_count: self.state.store.count().unwrap_or(0),
            captured,
            swept,
        }
    }

    pub(super) fn publish_items(&self, captured: bool, swept: u32) {
        if self.events.receiver_count() > 0 {
            let _ = self.events.send(self.items_event(captured, swept));
        }
    }

    pub(super) fn note_version_written(&self, created_at: i64) {
        self.cloud.note_version_written(self, created_at);
    }

    pub(super) fn note_oldest_version(&self, created_at: Option<i64>) {
        if let Some(created_at) = created_at {
            self.note_version_written(created_at);
        }
    }

    pub(super) fn note_cloud_version_applied(&self, created_at: i64) {
        if let Some(node) = self.node.get() {
            node.note_cloud_version_applied(created_at);
        }
    }
}

/// Whatever can put text on the system clipboard.
///
/// An indirection rather than a direct call into
/// `tauri_plugin_clipboard_manager` because the plugin needs an `AppHandle`,
/// which would make the backend generic over `tauri::Runtime` and drag that
/// parameter through every signature in `backend::mod`. One trait object at the
/// edge is cheaper than a type parameter on the whole surface.
pub trait Clipboard: Send + Sync + 'static {
    /// Errors are `&'static str`: this string can reach a user, and a
    /// platform error rendered into it is where a path would appear.
    fn set_text(&self, text: &str) -> std::result::Result<(), &'static str>;
}

impl EmbeddedBackend {
    /// Open the store, the keyring and the peer file under `data_dir`.
    ///
    /// The caller supplies the directory rather than this function reading it
    /// from `copypaste_ipc::data_dir()`, because on Android the right answer
    /// comes from the Android context and not from `directories`.
    pub fn open(data_dir: &Path, clipboard: Box<dyn Clipboard>) -> Result<Self> {
        let (events, _) = tokio::sync::broadcast::channel(64);
        let state = BackendState::open(data_dir)?;
        let cloud = EmbeddedCloud::open(&state);
        let inner = Arc::new(Inner {
            state,
            node: OnceCell::new(),
            clipboard,
            events,
            cloud,
        });
        super::retention::sweep(&inner);
        super::retention::start(&inner);
        let backend = Self { inner };
        backend.inner.cloud.ensure_poller(&backend.inner);
        Ok(backend)
    }

    /// The peer node, started on first use.
    ///
    /// `get_or_try_init` rather than `get_or_init`: opening the paired-device
    /// file can fail, and a failure that were cached would make every later
    /// peer operation fail for a reason that may have gone away.
    pub(super) async fn node(&self) -> Result<&PeerNode> {
        self.inner
            .node
            .get_or_try_init(|| PeerNode::start(&self.inner))
            .await
    }

    /// Run blocking work off the reactor.
    ///
    /// SQLite and the AEAD are both blocking, exactly as they are in the
    /// daemon, and the WebView's IPC is answered on the same runtime.
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

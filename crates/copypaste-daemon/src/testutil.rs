//! A fully wired `AppState` on a temporary data directory — one fixture for the
//! whole crate, so there is one definition of what a daemon is.

use std::collections::VecDeque;
use std::sync::Arc;

use copypaste_core::{Keyring, Store};
use copypaste_p2p::peers::PeerStore;
use copypaste_runtime::{Meta, Settings};

use crate::clipboard::{Capture, ClipboardSource};
use crate::AppState;

/// Reads nothing; records what is written, so a test can assert the *absence*
/// of a pasteboard write — the whole difference between `get` and `copy`.
#[derive(Default, Clone)]
struct CaptureFeed(Arc<std::sync::Mutex<CaptureFeedInner>>);

#[derive(Default)]
struct CaptureFeedInner {
    pending: VecDeque<Capture>,
    generation: i64,
    observed: i64,
}

#[derive(Default)]
pub struct FakeClipboard {
    writes: WriteLog,
    feed: CaptureFeed,
}

/// Everything written to a [`FakeClipboard`], shared with whoever made it.
#[derive(Default, Clone)]
pub struct WriteLog(Arc<std::sync::Mutex<Vec<WrittenPayload>>>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WrittenPayload {
    Text(String),
    Image(Vec<u8>),
    File {
        bytes: Vec<u8>,
        metadata: Option<copypaste_core::FileMetadata>,
    },
}

impl WriteLog {
    pub fn count(&self) -> usize {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    pub fn entries(&self) -> Vec<WrittenPayload> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

impl ClipboardSource for FakeClipboard {
    fn poll(&mut self) -> Option<Capture> {
        self.poll_with_policy(crate::clipboard::CapturePolicy::new(
            &copypaste_ipc::ConfigData::default(),
        ))
    }

    fn poll_with_policy(
        &mut self,
        _policy: crate::clipboard::CapturePolicy<'_>,
    ) -> Option<Capture> {
        let mut inner = self.feed.0.lock().unwrap_or_else(|e| e.into_inner());
        if inner.generation == inner.observed {
            return None;
        }
        inner.observed = inner.generation;
        inner.pending.pop_front()
    }

    fn changed(&mut self) -> bool {
        let inner = self.feed.0.lock().unwrap_or_else(|e| e.into_inner());
        inner.generation != inner.observed
    }

    fn set_contents(
        &mut self,
        text: &str,
        _: crate::clipboard::WriteOptions,
    ) -> anyhow::Result<()> {
        self.writes
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(WrittenPayload::Text(text.to_string()));
        Ok(())
    }

    fn write_payload(
        &mut self,
        _item_id: &str,
        payload: &copypaste_core::ClipboardPayload,
        _content_type: &str,
        _options: crate::clipboard::WriteOptions,
    ) -> Result<(), copypaste_core::ClipboardWriteError> {
        use copypaste_core::{ClipboardPayload, ClipboardWriteError};

        let written = match payload {
            ClipboardPayload::Text(text) => WrittenPayload::Text(text.to_string()),
            ClipboardPayload::Image { bytes, .. } => WrittenPayload::Image(bytes.to_vec()),
            ClipboardPayload::File { bytes, metadata } => {
                if let Some(source_reference) = metadata
                    .as_ref()
                    .and_then(|metadata| metadata.source_reference.as_deref())
                {
                    WrittenPayload::Text(source_reference.to_string())
                } else {
                    WrittenPayload::File {
                        bytes: bytes.to_vec(),
                        metadata: metadata.clone(),
                    }
                }
            }
            ClipboardPayload::Unsupported { .. } => {
                return Err(ClipboardWriteError::UnsupportedContent);
            }
        };
        self.writes
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(written);
        Ok(())
    }
    fn backend_name(&self) -> &'static str {
        "fake"
    }
}

/// A daemon state on its own database, with no cloud deployment configured.
///
/// The keyring is deterministic rather than loaded: a test must not touch the
/// developer's real keystore, and two states built with different names get
/// different secrets, which is what makes "re-encrypted under the local key" a
/// meaningful assertion.
pub fn test_state(name: &str) -> (Arc<AppState>, tempfile::TempDir) {
    reopen(tempfile::tempdir().expect("tempdir"), name)
}

pub fn test_state_with_clipboard(
    name: &str,
    clipboard: Box<dyn ClipboardSource>,
) -> (Arc<AppState>, tempfile::TempDir) {
    reopen_with(tempfile::tempdir().expect("tempdir"), name, clipboard)
}

/// A state plus the log of everything written to its clipboard.
pub fn test_state_watching_clipboard(name: &str) -> (Arc<AppState>, tempfile::TempDir, WriteLog) {
    let writes = WriteLog::default();
    let (state, dir) = reopen_with(
        tempfile::tempdir().expect("tempdir"),
        name,
        Box::new(FakeClipboard {
            writes: writes.clone(),
            feed: CaptureFeed::default(),
        }),
    );
    (state, dir, writes)
}

/// A second daemon over an existing data directory — a restart, in other words.
///
/// The keyring is derived from `name`, so passing the same name is what makes
/// the database openable again.
pub fn reopen(dir: tempfile::TempDir, name: &str) -> (Arc<AppState>, tempfile::TempDir) {
    reopen_with(
        dir,
        name,
        Box::new(FakeClipboard {
            writes: WriteLog::default(),
            feed: CaptureFeed::default(),
        }),
    )
}

fn reopen_with(
    dir: tempfile::TempDir,
    name: &str,
    clipboard: Box<dyn ClipboardSource>,
) -> (Arc<AppState>, tempfile::TempDir) {
    let db_path = dir.path().join("copypaste-v2.db");

    let mut secret = [0u8; 32];
    for (slot, byte) in secret.iter_mut().zip(name.bytes().cycle()) {
        *slot = byte;
    }
    let keyring = Arc::new(Keyring::from_secret(&secret));
    let store = Store::open(&db_path, &keyring.db_key()).expect("store");
    let meta = Meta::open(&store, name).expect("meta");
    let peers = PeerStore::open(
        &dir.path().join(copypaste_p2p::peers::DEFAULT_FILE_NAME),
        &keyring.peer_store_key(),
    )
    .expect("peer store");
    let settings = Arc::new(Settings::load(&store));
    // Port 0 is never bound in these tests; discovery degrades either way.
    let state = AppState::open(crate::state::Opened {
        db_path,
        store,
        keyring,
        meta,
        settings,
        peers,
        port: 0,
        clipboard,
    })
    .expect("daemon state");
    state.set_ready(true);
    (Arc::new(state), dir)
}

/// Ingest one item as if it had been captured locally, returning its id.
pub fn add(state: &Arc<AppState>, content: &str) -> String {
    crate::capture::ingest(state, content, "text")
        .expect("ingest")
        .into_item()
        .id
}

/// Every item's plaintext on one device, decrypted through the local key.
pub fn contents(state: &Arc<AppState>) -> Vec<String> {
    let key = state.keyring.item_key();
    state
        .store
        .list(100, 0)
        .expect("list")
        .into_iter()
        .map(|row| {
            let plain = copypaste_core::decrypt(&row.content_ciphertext, &row.nonce, &key, &row.id)
                .expect("decrypt");
            String::from_utf8(plain.to_vec()).expect("utf-8")
        })
        .collect()
}

/// A paired device this daemon has reached before, so [`crate::p2p::poll`]
/// treats it as dialable rather than as unauthenticated discovery hearsay.
pub fn peer_at(state: &Arc<AppState>, name: &str, addr: &str) -> copypaste_p2p::peers::Peer {
    let token = copypaste_p2p::PairingToken::generate();
    let peer = copypaste_p2p::peers::Peer {
        pairing_id: token.pairing_id(),

        device_id: None,
        name: name.to_string(),
        psk: token.psk(),
        last_addr: Some(addr.parse().expect("a peer address")),
        last_seen_ms: copypaste_core::now_ms(),
        profile: None,
        profile_observed_at_ms: 0,
    };
    state
        .p2p
        .peers()
        .upsert(peer.clone())
        .expect("store a peer");
    peer
}

pub fn test_socket_path(
    dir: &tempfile::TempDir,
    name: impl AsRef<std::path::Path>,
) -> std::path::PathBuf {
    let path = dir.path().join(name);
    let cwd = std::env::current_dir().expect("current directory");
    // Relative endpoints keep task-owned evidence directories under SUN_LEN,
    // unless climbing out of a deep working directory makes them longer.
    for (depth, ancestor) in cwd.ancestors().enumerate() {
        if let Ok(tail) = path.strip_prefix(ancestor) {
            let mut relative = std::path::PathBuf::new();
            for _ in 0..depth {
                relative.push("..");
            }
            let relative = relative.join(tail);
            if relative.as_os_str().len() < path.as_os_str().len() {
                return relative;
            }
            break;
        }
    }
    path
}

/// The handler names the daemon tests were written against, each one request
/// through the shared runtime dispatch with this daemon as its host.
pub(crate) mod calls {
    pub use base64::Engine as _;
    pub use copypaste_ipc::{
        ConfigApplied, ConfigData, ConfigPatch, ErrorCode, ExportItem, HistoryQuery, Method,
        PrivateModeData, Response, ResponseData,
    };
    pub use copypaste_p2p::peers::Peer;
    pub use copypaste_p2p::NodeError;
    pub use copypaste_runtime::messages::*;
    pub use std::path::Path;
    pub use std::sync::Arc;
    pub use tokio::net::TcpListener;
    pub use tokio::sync::watch;

    pub(crate) use crate::capture;
    pub use crate::AppState;
    pub use copypaste_core::storage::open_validated;
    pub use copypaste_core::StoredItem;
    pub use copypaste_ipc::MAX_PAGE_CONTENT_BYTES;
    pub use copypaste_runtime::MAX_REORDER_IDS;

    pub fn set_with_effects(
        state: &AppState,
        id: u64,
        patch: &ConfigPatch,
        effects: impl FnOnce(&copypaste_runtime::SettingsTransition),
    ) -> Response {
        state.set_config_with_effects(id, patch, effects)
    }

    pub fn failed(id: u64, error: NodeError) -> Response {
        Response::err(
            id,
            copypaste_core::p2p_contract::node_error_code(&error),
            error.to_string(),
        )
    }

    fn call(state: &AppState, id: u64, method: Method) -> Response {
        crate::server::dispatch::dispatch_store(state, id, method)
    }

    pub fn status(state: &AppState, id: u64) -> Response {
        call(state, id, Method::Status)
    }
    pub fn set_device_name(state: &AppState, id: u64, name: &str) -> Response {
        call(state, id, Method::SetDeviceName { name: name.into() })
    }
    pub fn list(state: &AppState, id: u64, limit: u32, cursor: Option<&str>) -> Response {
        call(
            state,
            id,
            Method::List {
                limit,
                cursor: cursor.map(Into::into),
            },
        )
    }
    pub fn search(state: &AppState, id: u64, query: &str, limit: u32) -> Response {
        call(
            state,
            id,
            Method::Search {
                query: query.into(),
                limit,
            },
        )
    }
    pub fn history_query(
        state: &AppState,
        id: u64,
        query: &HistoryQuery,
        limit: u32,
        cursor: Option<&str>,
    ) -> Response {
        call(
            state,
            id,
            Method::HistoryQuery {
                query: query.clone(),
                limit,
                cursor: cursor.map(Into::into),
            },
        )
    }
    pub fn history_facets(state: &AppState, id: u64) -> Response {
        call(state, id, Method::HistoryFacets)
    }
    pub fn image_preview(
        state: &AppState,
        id: u64,
        item_id: &str,
        max_edge: Option<u32>,
        bounds: Option<copypaste_ipc::ImagePreviewBounds>,
    ) -> Response {
        call(
            state,
            id,
            Method::ImagePreview {
                id: item_id.into(),
                max_edge,
                bounds,
            },
        )
    }
    pub fn add(state: &AppState, id: u64, content: &str) -> Response {
        call(
            state,
            id,
            Method::Add {
                content: content.into(),
            },
        )
    }
    pub fn get(state: &AppState, id: u64, item_id: &str) -> Response {
        call(state, id, Method::Get { id: item_id.into() })
    }
    pub fn delete(state: &AppState, id: u64, item_id: &str) -> Response {
        call(state, id, Method::Delete { id: item_id.into() })
    }
    pub fn delete_all(state: &AppState, id: u64, through: Option<i64>) -> Response {
        call(state, id, Method::DeleteAll { through })
    }
    pub fn set_spoiler_revealed(
        state: &AppState,
        id: u64,
        item_id: Option<&str>,
        revealed: bool,
    ) -> Response {
        call(
            state,
            id,
            Method::SetSpoilerRevealed {
                id: item_id.map(Into::into),
                revealed,
            },
        )
    }
    pub fn set_secret(state: &AppState, id: u64, item_id: &str, secret: bool) -> Response {
        call(
            state,
            id,
            Method::SetSecret {
                id: item_id.into(),
                secret,
            },
        )
    }
    pub fn pin(state: &AppState, id: u64, item_id: &str, pinned: bool) -> Response {
        call(
            state,
            id,
            Method::Pin {
                id: item_id.into(),
                pinned,
            },
        )
    }
    pub fn reorder_pinned(state: &AppState, id: u64, ids: &[String]) -> Response {
        call(state, id, Method::ReorderPinned { ids: ids.to_vec() })
    }
    pub fn copy(state: &AppState, id: u64, item_id: &str) -> Response {
        call(state, id, Method::Copy { id: item_id.into() })
    }
    pub fn copy_plain_text(state: &AppState, id: u64, item_id: &str) -> Response {
        call(state, id, Method::CopyPlainText { id: item_id.into() })
    }
    pub fn copy_text(state: &AppState, id: u64, item_id: &str, text: &str) -> Response {
        call(
            state,
            id,
            Method::CopyText {
                id: item_id.into(),
                text: text.into(),
            },
        )
    }
    pub fn save_file(state: &AppState, id: u64, item_id: &str, destination: &str) -> Response {
        call(
            state,
            id,
            Method::SaveFile {
                id: item_id.into(),
                dest_path: destination.into(),
                download_token: None,
            },
        )
    }
    pub fn set(state: &AppState, id: u64, patch: &ConfigPatch) -> Response {
        call(
            state,
            id,
            Method::SetConfig {
                patch: patch.clone(),
            },
        )
    }
    pub fn set_private_mode(state: &AppState, id: u64, enabled: bool) -> Response {
        call(state, id, Method::SetPrivateMode { enabled })
    }
    pub fn export(state: &AppState, id: u64, limit: u32) -> Response {
        call(state, id, Method::Export { limit })
    }

    async fn request(state: &Arc<AppState>, id: u64, method: Method) -> Response {
        state.runtime.request(id, method).await
    }
    pub async fn pair_create_invite(state: &Arc<AppState>, id: u64) -> Response {
        request(state, id, Method::PairCreateInvite).await
    }
    pub async fn pair_nearby(state: &Arc<AppState>, id: u64, addr: &str) -> Response {
        request(state, id, Method::PairNearby { addr: addr.into() }).await
    }
    pub async fn pair_join(state: &Arc<AppState>, id: u64, code: &str, addr: &str) -> Response {
        request(
            state,
            id,
            Method::PairJoin {
                code: code.into(),
                addr: addr.into(),
            },
        )
        .await
    }
    pub async fn pair_cancel(state: &Arc<AppState>, id: u64) -> Response {
        request(state, id, Method::PairCancel).await
    }
    pub async fn unpair(state: &Arc<AppState>, id: u64, pairing_id: &str) -> Response {
        request(
            state,
            id,
            Method::Unpair {
                pairing_id: pairing_id.into(),
            },
        )
        .await
    }
    pub async fn revoke(state: &Arc<AppState>, id: u64, pairing_id: &str) -> Response {
        request(
            state,
            id,
            Method::Revoke {
                pairing_id: pairing_id.into(),
            },
        )
        .await
    }
    pub async fn peers(state: &Arc<AppState>, id: u64) -> Response {
        request(state, id, Method::Peers).await
    }
    pub async fn sync_now(state: &Arc<AppState>, id: u64, pairing_id: Option<&str>) -> Response {
        request(
            state,
            id,
            Method::SyncNow {
                pairing_id: pairing_id.map(Into::into),
            },
        )
        .await
    }

    pub fn store_source(state: &AppState) -> Arc<copypaste_core::StoreSource> {
        Arc::clone(&state.source)
    }

    pub async fn listen(
        listener: tokio::net::TcpListener,
        state: Arc<AppState>,
        shutdown: tokio::sync::watch::Receiver<bool>,
    ) {
        Arc::clone(&state.runtime).listen(listener, shutdown).await;
    }
}

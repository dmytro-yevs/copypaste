//! A fully wired `AppState` on a temporary data directory — one fixture for the
//! whole crate, so there is one definition of what a daemon is.

use std::collections::VecDeque;
use std::sync::Arc;

use copypaste_core::{Detector, Keyring, Store};
use copypaste_p2p::discovery::Discovery;
use copypaste_p2p::peers::PeerStore;

use crate::clipboard::{Capture, ClipboardSource};
use crate::cloud::Cloud;
use crate::meta::Meta;
use crate::p2p::P2p;
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

    fn set_contents(&mut self, text: &str) -> anyhow::Result<()> {
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
    ) -> Result<(), copypaste_core::ClipboardWriteError> {
        use copypaste_core::{ClipboardPayload, ClipboardWriteError};

        let written = match payload {
            ClipboardPayload::Text(text) => WrittenPayload::Text(text.to_string()),
            ClipboardPayload::Image { bytes, .. } => WrittenPayload::Image(bytes.to_vec()),
            ClipboardPayload::File { bytes, metadata } => WrittenPayload::File {
                bytes: bytes.to_vec(),
                metadata: metadata.clone(),
            },
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
    test_state_with_cloud(name, Cloud::new(None))
}

pub fn test_state_with_clipboard(
    name: &str,
    clipboard: Box<dyn ClipboardSource>,
) -> (Arc<AppState>, tempfile::TempDir) {
    reopen_with(
        tempfile::tempdir().expect("tempdir"),
        Cloud::new(None),
        name,
        clipboard,
    )
}

/// A state plus the log of everything written to its clipboard.
pub fn test_state_watching_clipboard(name: &str) -> (Arc<AppState>, tempfile::TempDir, WriteLog) {
    let writes = WriteLog::default();
    let (state, dir) = reopen_with(
        tempfile::tempdir().expect("tempdir"),
        Cloud::new(None),
        name,
        Box::new(FakeClipboard {
            writes: writes.clone(),
            feed: CaptureFeed::default(),
        }),
    );
    (state, dir, writes)
}

pub fn test_state_with_cloud(name: &str, cloud: Cloud) -> (Arc<AppState>, tempfile::TempDir) {
    reopen(tempfile::tempdir().expect("tempdir"), cloud, name)
}

/// A second daemon over an existing data directory — a restart, in other words.
///
/// The keyring is derived from `name`, so passing the same name is what makes
/// the database openable again.
pub fn reopen(
    dir: tempfile::TempDir,
    cloud: Cloud,
    name: &str,
) -> (Arc<AppState>, tempfile::TempDir) {
    reopen_with(
        dir,
        cloud,
        name,
        Box::new(FakeClipboard {
            writes: WriteLog::default(),
            feed: CaptureFeed::default(),
        }),
    )
}

fn reopen_with(
    dir: tempfile::TempDir,
    cloud: Cloud,
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
    let peers = PeerStore::open(&dir.path().join(copypaste_p2p::peers::DEFAULT_FILE_NAME))
        .expect("peer store");
    // Port 0 is never bound in these tests; discovery degrades either way.
    let discovery = Discovery::dormant(name, 0).expect("discovery");
    let settings = crate::settings::Settings::load(&meta);

    let state = AppState::new(
        store,
        keyring,
        Arc::new(Detector::new().expect("detector")),
        clipboard,
        meta,
        P2p::new(peers, Some(discovery), 0, true),
        cloud,
        settings,
        db_path,
    );
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
        name: name.to_string(),
        psk: token.psk(),
        last_addr: Some(addr.parse().expect("a peer address")),
        last_seen_ms: copypaste_core::now_ms(),
    };
    state
        .p2p
        .peers()
        .upsert(peer.clone())
        .expect("store a peer");
    peer
}

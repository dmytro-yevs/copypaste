//! A fully wired `AppState` on a temporary data directory — one fixture for the
//! whole crate, so there is one definition of what a daemon is.

use std::sync::Arc;

use copypaste_core::{Detector, Keyring, Store};
use copypaste_p2p::discovery::Discovery;
use copypaste_p2p::peers::PeerStore;

use crate::AppState;
use crate::clipboard::{Capture, ClipboardSource};
use crate::cloud::Cloud;
use crate::meta::Meta;
use crate::p2p::P2p;

/// Reads nothing; records what is written, so a test can assert the *absence*
/// of a pasteboard write — the whole difference between `get` and `copy`.
#[derive(Default)]
pub struct FakeClipboard {
    writes: WriteLog,
}

/// Everything written to a [`FakeClipboard`], shared with whoever made it.
#[derive(Default, Clone)]
pub struct WriteLog(Arc<std::sync::Mutex<Vec<String>>>);

impl WriteLog {
    pub fn count(&self) -> usize {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
}

impl ClipboardSource for FakeClipboard {
    fn poll(&mut self) -> Option<Capture> {
        None
    }
    fn set_contents(&mut self, text: &str) -> anyhow::Result<()> {
        self.writes
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(text.to_string());
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

/// A state plus the log of everything written to its clipboard.
pub fn test_state_watching_clipboard(name: &str) -> (Arc<AppState>, tempfile::TempDir, WriteLog) {
    let writes = WriteLog::default();
    let (state, dir) = reopen_with(
        tempfile::tempdir().expect("tempdir"),
        Cloud::new(None),
        name,
        Box::new(FakeClipboard {
            writes: writes.clone(),
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
    reopen_with(dir, cloud, name, Box::new(FakeClipboard::default()))
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
    let discovery = Discovery::start(name, &[], 0).expect("discovery");
    let settings = crate::settings::Settings::load(&meta);

    let state = AppState::new(
        store,
        keyring,
        Arc::new(Detector::new().expect("detector")),
        clipboard,
        meta,
        P2p::new(peers, Some(discovery), 0),
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
            String::from_utf8(plain).expect("utf-8")
        })
        .collect()
}

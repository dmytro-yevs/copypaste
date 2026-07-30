//! The Android backend: `copypaste-core` and `copypaste-p2p` in the app
//!  process.
//!
//! Android will not host a long-lived background daemon, so there is no socket
//! and no second process (ADR-0002). The store, the keyring and the peer file
//! are opened here and the same operations run inline.
//!
//! # What this file will not do, and why that matters
//!
//! Four operations — `add`, `pair_create`, `pair_accept` and `sync` — return
//! [`BackendError::Unsupported`] rather than an implementation. That is a
//! deliberate refusal, not an oversight, and it is worth being precise about
//! because the alternative is the exact defect this rewrite exists to end.
//!
//! **`add` needs the ingest pipeline**, which is
//! `copypaste_daemon::capture::ingest`: trim, hash, dedup-probe, detect,
//! choose the id *before* the seal because the AEAD binds it, encrypt, insert,
//! record origin, evict. Every step of that is a decision with a bug behind it
//! — manifest 01 I-33 (a dedup-probe failure must fall through to the insert),
//! the `cutoff_ms` argument that is an absolute epoch stamp and not a window
//! width, the write-time half of "sensitive items never reach the search
//! index". Re-typing those forty lines here would produce a second ingest path,
//! and `capture.rs`'s own module docs record what happened last time there were
//! two: *"v1 had two ingest paths that drifted: the IPC one forgot the dedup
//! probe."* CLAUDE.md rule 1 names "it's only a few lines" as the failure mode
//! by name.
//!
//! **The three peer operations need a running node** — a TCP listener holding
//! the pre-shared keys, mDNS discovery, and the sync-metadata connection that
//! `copypaste_daemon::p2p::meta` opens onto the same SQLCipher file for the
//! columns `StoredItem` does not carry. `copypaste-p2p` provides the transport,
//! the merge and the protocol; it does not provide the node that owns them, and
//! that node currently lives inside the daemon binary.
//!
//! `copypaste-daemon` has no `[lib]` target, so none of it is importable. The
//! fix is one refactor in a crate this change does not own, and it is written
//! up in ADR-0003: lift `capture::ingest` down into `copypaste-core` (whose
//! three modules — crypto, storage, sensitive — are already the only things it
//! touches) and lift the p2p node up into `copypaste-p2p`. Until then these
//! four report a structural failure that says so, instead of a plausible one
//! that invites a retry.
//!
//! # What it does do
//!
//! Everything reachable through the library API as it stands: the read paths,
//! the clipboard write, delete/clear/pin, status, and the two peer operations
//! that are pure `PeerStore` calls. Those are real and complete.
//!
//! # Unverified
//!
//! Nothing in this file has been run. This host has no Android SDK and no NDK,
//! so it is compiled — under `--features embedded-backend`, on Linux — and
//! never executed. Two things are known-wrong for a shipping Android build and
//! are recorded in ADR-0003 rather than papered over here: the device secret
//! falls through `copypaste_core::Keyring` to the `0600`-file backend because
//! there is no Android Keystore backend yet, and the data directory comes from
//! `copypaste_ipc::data_dir()`, which resolves through `directories` rather
//! than through the Android context.

use std::path::Path;
use std::sync::Arc;

use copypaste_core::{decrypt, Keyring, Store, StoredItem};
use copypaste_ipc::{
    DiscoveredDevice, EventData, Item, PairingData, PeerInfo, StatusData, SyncResult,
};
use copypaste_p2p::PeerStore;

use super::{Backend, BackendError, Page, Result};

/// Server-side clamp on a caller-supplied page size (manifest 04 §3.3).
/// Identical to the daemon's, because it is the same contract seen from the
/// other side — a frontend asking for ten million rows must get the same
/// answer on both platforms.
const MAX_PAGE: u32 = 1_000;
const DEFAULT_LIST_PAGE: u32 = 50;
const DEFAULT_SEARCH_PAGE: u32 = 20;

/// Reported in [`StatusData::clipboard_backend`] so a build that is not polling
/// anything cannot be mistaken for one that is.
const BACKEND_NAME: &str = "android-inprocess";

const MSG_NO_INGEST: &str =
    "Adding items isn't available in this build yet. Copy from another app instead.";
const MSG_NO_PAIRING: &str = "Pairing isn't available in this build yet.";
const MSG_NO_SYNC: &str = "Syncing isn't available in this build yet.";
const MSG_NO_ITEM: &str = "That item is no longer there.";
const MSG_NO_PEER: &str = "That device isn't paired.";
const MSG_NO_WATCH: &str = "Live updates aren't available in this build.";
const MSG_NO_DISCOVERY: &str = "Finding nearby devices isn't available in this build yet.";
const MSG_NO_REORDER: &str =
    "Reordering pinned items isn't available yet. Pinned items keep the order they \
     were pinned in.";

/// Everything the in-process backend owns. Cheap to clone; `Store` is a
/// reference-counted pool and the rest is behind an `Arc`.
#[derive(Clone)]
pub struct EmbeddedBackend {
    inner: Arc<Inner>,
}

struct Inner {
    store: Store,
    keyring: Keyring,
    peers: PeerStore,
    /// Writes the system clipboard. Held as a boxed trait object so this file
    /// does not depend on Tauri's plugin types, and so tests can substitute
    /// one — see `Clipboard`.
    clipboard: Box<dyn Clipboard>,
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
        std::fs::create_dir_all(data_dir)
            .map_err(|e| BackendError::internal(&format!("could not prepare storage: {e}")))?;

        // `load_or_create` reaches the Android Keystore only once core grows a
        // backend for it; today this is the `0600` file, which is a development
        // posture (ADR-0003).
        let keyring = Keyring::load_or_create()
            .map_err(|e| BackendError::internal(&format!("could not open the keystore: {e}")))?;

        // The v2 filename, from the shared crate. Deliberately distinct from
        // v1's so an old database is never touched (CLAUDE.md rule 3).
        let db_path = data_dir.join(
            copypaste_ipc::database_path()
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("copypaste-v2.db")),
        );
        let store = Store::open(&db_path, &keyring.db_key())
            .map_err(|e| BackendError::internal(&format!("could not open history: {e}")))?;

        let peers = PeerStore::open(&data_dir.join("peers.json"))
            .map_err(|e| BackendError::internal(&format!("could not open paired devices: {e}")))?;

        Ok(Self {
            inner: Arc::new(Inner {
                store,
                keyring,
                peers,
                clipboard,
            }),
        })
    }

    /// Run blocking work off the reactor.
    ///
    /// SQLite and the AEAD are both blocking, exactly as they are in the
    /// daemon, and the WebView's IPC is answered on the same runtime.
    async fn blocking<T, F>(&self, f: F) -> Result<T>
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

impl Inner {
    /// Decrypt one stored row into its wire form.
    ///
    /// Two library calls and a struct literal. The item id is the AAD, so a row
    /// decrypted under another row's identity fails authentication rather than
    /// falling back to a plaintext read (CLAUDE.md rule 4, "fail closed").
    fn to_wire(&self, row: StoredItem) -> Result<Item> {
        let key = self.keyring.item_key();
        let plaintext = decrypt(&row.content_ciphertext, &row.nonce, &key, &row.id)
            .map_err(|_| BackendError::internal("that item could not be decrypted"))?;
        Ok(Item {
            id: row.id,
            content: String::from_utf8_lossy(&plaintext).into_owned(),
            content_type: row.content_type,
            created_at: row.created_at,
            pinned: row.pinned,
            is_sensitive: row.is_sensitive,
        })
    }

    /// Decrypt a page, dropping any row that will not open — and **counting**
    /// what was dropped.
    ///
    /// One unreadable row must not blank a whole page: the other items are
    /// still the user's data (CLAUDE.md rule 4). But dropping it silently makes
    /// a short page indistinguishable from a small history, which is parity
    /// finding 17 / `CopyPaste-00zz`. The count is what lets the UI say "3
    /// items could not be read" instead of showing three fewer rows.
    fn to_wire_page(&self, rows: Vec<StoredItem>) -> Page {
        let mut page = Page::default();
        for row in rows {
            let id = row.id.clone();
            match self.to_wire(row) {
                Ok(item) => page.items.push(item),
                Err(_) => {
                    tracing::warn!(%id, "skipping an item that failed to decrypt");
                    page.skipped_undecryptable = page.skipped_undecryptable.saturating_add(1);
                }
            }
        }
        page
    }

    fn fetch(&self, id: &str) -> Result<Item> {
        match self.store.get(id) {
            Ok(Some(row)) => self.to_wire(row),
            Ok(None) => Err(BackendError::NotFound(MSG_NO_ITEM)),
            Err(_) => Err(BackendError::internal("history could not be read")),
        }
    }
}

fn clamp_page(limit: u32, default: u32) -> u32 {
    if limit == 0 {
        default
    } else {
        limit.min(MAX_PAGE)
    }
}

fn peer_info(peer: &copypaste_p2p::Peer, online: bool) -> PeerInfo {
    PeerInfo {
        pairing_id: peer.pairing_id.clone(),
        name: peer.name.clone(),
        // `Peer` holds a parsed `SocketAddr`; the wire type carries the
        // rendered form, because it is shown to a user and never dialled by the
        // frontend.
        last_addr: peer.last_addr.map(|addr| addr.to_string()),
        last_seen_ms: peer.last_seen_ms,
        online,
    }
}

impl Backend for EmbeddedBackend {
    async fn list(&self, limit: u32, offset: u32) -> Result<Page> {
        let limit = clamp_page(limit, DEFAULT_LIST_PAGE);
        self.blocking(move |inner| {
            let rows = inner
                .store
                .list(limit, offset)
                .map_err(|_| BackendError::internal("history could not be read"))?;
            Ok(inner.to_wire_page(rows))
        })
        .await
    }

    async fn search(&self, query: &str, limit: u32) -> Result<Page> {
        let limit = clamp_page(limit, DEFAULT_SEARCH_PAGE);
        let query = query.to_string();
        self.blocking(move |inner| {
            let rows = inner
                .store
                .search(&query, limit)
                .map_err(|_| BackendError::internal("history could not be searched"))?;
            // Read-time enforcement of "sensitive items are never searchable".
            // The store already keeps them out of the index at write time; this
            // is the second of the three layers CLAUDE.md rule 4 demands, and
            // it is what protects a database written before the rule existed.
            // Both backends carrying it is the point of a layered rule, not a
            // duplicated decision.
            let rows: Vec<StoredItem> = rows.into_iter().filter(|r| !r.is_sensitive).collect();
            Ok(inner.to_wire_page(rows))
        })
        .await
    }

    /// See the module docs: this needs `capture::ingest`, which is inside the
    /// `copypaste-daemon` binary.
    async fn add(&self, _content: &str) -> Result<Item> {
        Err(BackendError::Unsupported(MSG_NO_INGEST))
    }

    async fn get(&self, id: &str) -> Result<Item> {
        let id = id.to_string();
        self.blocking(move |inner| inner.fetch(&id)).await
    }

    async fn copy(&self, id: &str) -> Result<Item> {
        let id = id.to_string();
        self.blocking(move |inner| {
            let item = inner.fetch(&id)?;
            // The error is a `&'static str` by the trait's design, so nothing
            // caller-supplied can be interpolated into it and it needs no
            // scrubbing.
            inner
                .clipboard
                .set_text(&item.content)
                .map_err(|msg| BackendError::Internal(msg.to_string()))?;
            Ok(item)
        })
        .await
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let id = id.to_string();
        self.blocking(move |inner| {
            // Read first so an unknown id is `not_found` rather than a silent
            // success: a client that deleted nothing needs to know it deleted
            // nothing. Same rule as the daemon's `items::delete`.
            match inner.store.delete(&id) {
                Ok(true) => Ok(()),
                Ok(false) => Err(BackendError::NotFound(MSG_NO_ITEM)),
                Err(_) => Err(BackendError::internal("that item could not be deleted")),
            }
        })
        .await
    }

    async fn clear(&self) -> Result<u64> {
        self.blocking(move |inner| {
            inner
                .store
                .delete_all()
                .map_err(|_| BackendError::internal("history could not be cleared"))
        })
        .await
    }

    async fn set_pinned(&self, id: &str, pinned: bool) -> Result<Item> {
        let id = id.to_string();
        self.blocking(move |inner| {
            match inner.store.set_pinned(&id, pinned) {
                Ok(true) => {}
                Ok(false) => return Err(BackendError::NotFound(MSG_NO_ITEM)),
                Err(_) => return Err(BackendError::internal("that item could not be changed")),
            }
            // Reply with the updated row so the caller need not re-list.
            inner.fetch(&id)
        })
        .await
    }

    /// Needs `Store::reorder_pinned`, which does not exist.
    ///
    /// The `pin_order` column is there and `set_pinned` maintains it, but
    /// rewriting the order is a transaction over the whole pinned section and
    /// belongs beside the other `pin_order` writes in `copypaste-core`, not
    /// re-derived here. Same refusal as the daemon backend, same reason
    /// (parity finding 19) — so both platforms are missing exactly one thing
    /// rather than one platform quietly growing a second implementation.
    async fn reorder_pinned(&self, _ids: &[String]) -> Result<()> {
        Err(BackendError::Unsupported(MSG_NO_REORDER))
    }

    async fn status(&self) -> Result<StatusData> {
        self.blocking(move |inner| {
            // `status` never fails: an unreadable count is reported as zero
            // rather than an error, because a caller may be probing precisely
            // because storage is unhappy.
            let item_count = inner.store.count().unwrap_or(0);
            Ok(StatusData {
                version: env!("CARGO_PKG_VERSION").to_string(),
                protocol_version: copypaste_ipc::PROTOCOL_VERSION,
                item_count,
                // There is no capture loop in this build: Android has no
                // background daemon and no clipboard polling. Reporting `true`
                // would tell the status line that history is growing when it
                // is not.
                capture_running: false,
                clipboard_backend: BACKEND_NAME.to_string(),
            })
        })
        .await
    }

    async fn pair_create(&self, _name: &str) -> Result<PairingData> {
        Err(BackendError::Unsupported(MSG_NO_PAIRING))
    }

    async fn pair_accept(&self, _code: &str, _addr: &str) -> Result<Vec<PeerInfo>> {
        Err(BackendError::Unsupported(MSG_NO_PAIRING))
    }

    /// A pure `PeerStore` read, so it is real.
    ///
    /// `online` is always `false`: liveness comes from mDNS discovery, and
    /// there is no discovery service running in this build. The wire type
    /// documents `false` as "not seen", never "unreachable", so this is the
    /// honest value rather than a placeholder.
    async fn peers(&self) -> Result<Vec<PeerInfo>> {
        self.blocking(move |inner| {
            Ok(inner
                .peers
                .list()
                .iter()
                .map(|peer| peer_info(peer, false))
                .collect())
        })
        .await
    }

    /// Also a pure `PeerStore` operation. Local and one-sided: the other device
    /// keeps its half until it also unpairs.
    async fn unpair(&self, pairing_id: &str) -> Result<()> {
        let pairing_id = pairing_id.to_string();
        self.blocking(move |inner| match inner.peers.remove(&pairing_id) {
            Ok(true) => Ok(()),
            Ok(false) => Err(BackendError::NotFound(MSG_NO_PEER)),
            Err(_) => Err(BackendError::internal("that device could not be removed")),
        })
        .await
    }

    async fn sync(&self, _pairing_id: Option<&str>) -> Result<Vec<SyncResult>> {
        Err(BackendError::Unsupported(MSG_NO_SYNC))
    }

    /// Needs the running p2p node, same as pairing does (ADR-0003).
    ///
    /// `copypaste_p2p::discovery` is importable, but browsing without also
    /// advertising would show this device other devices while leaving it
    /// invisible to them — half a feature that looks like a whole one.
    async fn discovered(&self) -> Result<Vec<DiscoveredDevice>> {
        Err(BackendError::Unsupported(MSG_NO_DISCOVERY))
    }

    async fn rescan(&self) -> Result<Vec<DiscoveredDevice>> {
        Err(BackendError::Unsupported(MSG_NO_DISCOVERY))
    }

    /// There is nothing to subscribe to.
    ///
    /// Push exists because on the desktop a *separate process* changes history
    /// under the app. Here the app is the only writer, so every change is one
    /// this process just made and React Query has already invalidated. The
    /// frontend falls back to its poll, which costs nothing on a platform where
    /// history only changes when the user is looking at it.
    async fn watch(&self) -> Result<tokio::sync::mpsc::Receiver<EventData>> {
        Err(BackendError::Unsupported(MSG_NO_WATCH))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Records what was written, so `copy` can be asserted without a system
    /// clipboard.
    #[derive(Default)]
    struct FakeClipboard(Mutex<Vec<String>>);

    impl Clipboard for Arc<FakeClipboard> {
        fn set_text(&self, text: &str) -> std::result::Result<(), &'static str> {
            self.0.lock().unwrap().push(text.to_string());
            Ok(())
        }
    }

    fn backend() -> (EmbeddedBackend, Arc<FakeClipboard>, tempfile::TempDir) {
        let dir = tempfile::TempDir::new().unwrap();
        // Keeps the test off the real keystore and off the developer's own
        // history file.
        std::env::set_var("COPYPASTE_EPHEMERAL_KEY", "1");
        let clipboard = Arc::new(FakeClipboard::default());
        let backend = EmbeddedBackend::open(dir.path(), Box::new(Arc::clone(&clipboard)))
            .expect("the embedded backend should open under a temp dir");
        (backend, clipboard, dir)
    }

    #[test]
    fn page_sizes_are_clamped_exactly_as_the_daemon_clamps_them() {
        assert_eq!(clamp_page(0, DEFAULT_LIST_PAGE), DEFAULT_LIST_PAGE);
        assert_eq!(clamp_page(10, DEFAULT_LIST_PAGE), 10);
        assert_eq!(clamp_page(u32::MAX, DEFAULT_LIST_PAGE), MAX_PAGE);
    }

    #[tokio::test]
    async fn it_opens_and_reports_an_honest_status() {
        let (backend, _clip, _dir) = backend();
        let status = backend.status().await.unwrap();
        assert_eq!(status.item_count, 0);
        assert!(
            !status.capture_running,
            "there is no capture loop in this build"
        );
        assert_eq!(status.clipboard_backend, BACKEND_NAME);
    }

    #[tokio::test]
    async fn an_empty_history_lists_and_searches_without_failing() {
        let (backend, _clip, _dir) = backend();
        assert!(backend.list(50, 0).await.unwrap().items.is_empty());
        assert!(backend
            .search("anything", 20)
            .await
            .unwrap()
            .items
            .is_empty());
    }

    #[tokio::test]
    async fn an_unknown_id_is_not_found_rather_than_a_silent_success() {
        let (backend, _clip, _dir) = backend();
        assert!(matches!(
            backend.delete("nope").await.unwrap_err(),
            BackendError::NotFound(_)
        ));
        assert!(matches!(
            backend.set_pinned("nope", true).await.unwrap_err(),
            BackendError::NotFound(_)
        ));
        assert!(matches!(
            backend.get("nope").await.unwrap_err(),
            BackendError::NotFound(_)
        ));
    }

    #[tokio::test]
    async fn an_unknown_peer_is_not_found() {
        let (backend, _clip, _dir) = backend();
        assert!(backend.peers().await.unwrap().is_empty());
        assert!(matches!(
            backend.unpair("nope").await.unwrap_err(),
            BackendError::NotFound(_)
        ));
    }

    /// The refusals must read as structural, not transient: a user must not be
    /// told to try again at something that will never work in this build.
    #[tokio::test]
    async fn the_unimplemented_operations_say_so_plainly() {
        let (backend, _clip, _dir) = backend();
        for err in [
            backend.add("x").await.unwrap_err(),
            backend.pair_create("phone").await.unwrap_err(),
            backend.pair_accept("code", "1.2.3.4:1").await.unwrap_err(),
            backend.sync(None).await.unwrap_err(),
        ] {
            assert!(
                matches!(err, BackendError::Unsupported(_)),
                "expected a structural refusal, got {err:?}"
            );
            let shown = err.to_string();
            assert!(!shown.contains("try again"), "reads as transient: {shown}");
            assert!(!shown.contains('/'), "a path reached the user: {shown}");
        }
    }

    #[tokio::test]
    async fn clearing_an_empty_history_is_a_success_reporting_zero() {
        let (backend, _clip, _dir) = backend();
        assert_eq!(backend.clear().await.unwrap(), 0);
    }
}

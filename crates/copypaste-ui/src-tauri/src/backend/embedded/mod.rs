//! The Android backend: `copypaste-core` and `copypaste-p2p` in the app
//!  process.
//!
//! Android will not host a long-lived background daemon, so there is no socket
//! and no second process (ADR-0002). The store, the keyring and the peer file
//! are opened inside the app and the same operations run inline.
//!
//! # What this file will not do, and why that matters
//!
//! Four operations — `set_config`, `backup`, `restore` and `reorder_pinned` —
//! return [`BackendError::Unsupported`] rather than an implementation. Each is a
//! deliberate refusal with its reason on the method. `backup` and `restore`
//! still name the same fix: the validate-then-swap lives inside the
//! `copypaste-daemon` binary, which has no `[lib]` target, so approximating it
//! here would be the second implementation CLAUDE.md rule 1 exists to stop —
//! and a file copy that looks like a restore and is not is data loss.
//!
//! Pairing, syncing, discovery, export and import used to be on that list. They
//! are not any more: `copypaste_p2p::Node` owns the listener and the four peer
//! operations, `copypaste_core::StoreSource` is the history behind them, and
//! `copypaste_core::transfer` owns the skip accounting and the
//! detector-re-runs-on-import rule — the same node, the same source and the same
//! two transfer functions the daemon runs (ADR-0003, [`peers`]).
//!
//! # What it does do
//!
//! Everything reachable through the library API as it stands: the read paths,
//! ingest, the clipboard write, delete/clear/pin, status, export and import, and
//! the whole of peer pairing, sync and LAN discovery.
//!
//! # Unverified
//!
//! Nothing in this file has been run. This host has no Android SDK and no NDK,
//! so it is compiled — under `--features embedded-backend`, on Linux — and
//! never executed. Note that the backend `copypaste_core::Keyring` selects here
//! is *not* the one an Android build selects: on this host it is the `0600`
//! file, and on Android it is the Keystore-wrapped secret, which no machine in
//! this project can compile.

mod peers;
mod rows;
mod state;
mod transfer;

use std::path::Path;
use std::sync::Arc;

use copypaste_core::{ingest, IngestError, ItemCursor, StoredItem};
use copypaste_ipc::{
    BackupData, ConfigApplied, ConfigPatch, DiscoveredDevice, EventData, ExportData, ExportItem,
    ImportData, Item, PairingData, PeerInfo, StatusData, SyncResult,
};
use tokio::sync::OnceCell;

use super::{Backend, BackendError, Page, Result};
use peers::PeerNode;
use rows::{clamp_page, DEFAULT_LIST_PAGE, DEFAULT_SEARCH_PAGE};
use state::BackendState;

/// Reported in [`StatusData::clipboard_backend`] so a build that is not polling
/// anything cannot be mistaken for one that is.
const BACKEND_NAME: &str = "android-inprocess";

const MSG_EMPTY: &str = "There was nothing to save.";
const MSG_TOO_LARGE: &str = "That item is larger than the size limit you set.";
const MSG_NOT_STORED: &str = "That item could not be saved.";
const MSG_NO_ITEM: &str = "That item is no longer there.";
/// Refused rather than restarted: a load-more that silently began again would
/// replay the whole history and the caller could not tell.
const MSG_BAD_CURSOR: &str = "That page marker isn't one this app issued.";
const MSG_NO_PEER: &str = "That device isn't paired.";
const MSG_NO_WATCH: &str = "Live updates aren't available in this build.";
const MSG_NO_SETTINGS: &str =
    "This build can't change the service's settings — it has no background service to \
     change them on, and runs on the defaults shown here.";
const MSG_NO_BACKUP: &str = "Backup and restore aren't available in this build yet.";
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
    state: BackendState,
    /// The peer node, brought up on the first operation that needs it — see
    /// [`peers`] for why it is not built in [`EmbeddedBackend::open`].
    node: OnceCell<PeerNode>,
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
        Ok(Self {
            inner: Arc::new(Inner {
                state: BackendState::open(data_dir)?,
                node: OnceCell::new(),
                clipboard,
            }),
        })
    }

    /// The peer node, started on first use.
    ///
    /// `get_or_try_init` rather than `get_or_init`: opening the paired-device
    /// file can fail, and a failure that were cached would make every later
    /// peer operation fail for a reason that may have gone away.
    async fn node(&self) -> Result<&PeerNode> {
        self.inner
            .node
            .get_or_try_init(|| PeerNode::start(&self.inner))
            .await
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

impl Backend for EmbeddedBackend {
    async fn list(&self, limit: u32, cursor: Option<&str>) -> Result<Page> {
        let limit = clamp_page(limit, DEFAULT_LIST_PAGE);
        let after = match cursor.map(ItemCursor::parse).transpose() {
            Ok(after) => after,
            // Never "start from the top": a load-more that silently restarted
            // would replay the whole history, and the caller cannot tell that
            // from a list that really does begin again.
            Err(_) => return Err(BackendError::Invalid(MSG_BAD_CURSOR)),
        };
        self.blocking(move |inner| {
            let page = inner
                .state
                .store
                .list_from(after.as_ref(), limit)
                .map_err(|_| BackendError::internal("history could not be read"))?;
            let next = page.next.map(|cursor| cursor.token());
            let mut wire = inner.to_wire_page(page.items);
            // From the store's page, never from what survived decryption: rows
            // that would not open still occupy the window.
            wire.next_cursor = next;
            Ok(wire)
        })
        .await
    }

    async fn search(&self, query: &str, limit: u32) -> Result<Page> {
        let limit = clamp_page(limit, DEFAULT_SEARCH_PAGE);
        let query = query.to_string();
        self.blocking(move |inner| {
            let rows = inner
                .state
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

    /// Every capture on this platform ends here — the share sheet, the
    /// text-selection action, the Quick Settings tile and rung 2's background
    /// listener alike (`crate::capture::intake`).
    ///
    /// `copypaste_core::ingest` is the daemon's ingest path, moved down into
    /// the core rather than re-typed here: dedup, the write-time secret rule,
    /// the id-before-seal ordering and eviction are one implementation with
    /// two callers, not two that will drift (CLAUDE.md rule 1, ADR-0003).
    async fn add(&self, content: &str) -> Result<Item> {
        let content = content.to_string();
        self.blocking(move |inner| {
            let outcome = ingest(
                &inner.state.store,
                &inner.state.detector,
                &inner.state.keyring,
                &content,
                copypaste_ipc::content_type::TEXT,
                &inner.state.settings,
            );
            match outcome {
                Ok(ingested) => inner.to_wire(ingested.into_item()),
                // Both refusals are the user's own configuration answering, so
                // they are reported as invalid input rather than as a fault —
                // and neither is retryable with the same content.
                Err(IngestError::Empty) => Err(BackendError::Invalid(MSG_EMPTY)),
                Err(IngestError::TooLarge) => Err(BackendError::Invalid(MSG_TOO_LARGE)),
                Err(e) => {
                    tracing::warn!(error = ?e, "a capture could not be stored");
                    Err(BackendError::internal(MSG_NOT_STORED))
                }
            }
        })
        .await
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

    async fn copy_as_plain_text(&self, id: &str) -> Result<Item> {
        let id = id.to_string();
        self.blocking(move |inner| {
            let item = inner.fetch(&id)?;
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
            match inner.state.store.delete(&id) {
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
                .state
                .store
                .delete_all()
                .map_err(|_| BackendError::internal("history could not be cleared"))
        })
        .await
    }

    async fn set_pinned(&self, id: &str, pinned: bool) -> Result<Item> {
        let id = id.to_string();
        self.blocking(move |inner| {
            match inner.state.store.set_pinned(&id, pinned) {
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
        let status = self.blocking(rows::status_of).await?;
        // The supervisor probes `status` at launch, and this is what makes that
        // probe start the peer listener: without it a paired device could only
        // reach this one while a peer screen happened to be open. A node that
        // will not come up is logged and stepped over — history still works.
        if let Err(e) = self.node().await {
            tracing::warn!(error = %e, "the peer node did not start");
        }
        Ok(status)
    }

    /// Mint a pairing and hand back the code to read out to the other device.
    ///
    /// The peer is stored before the other device has ever been heard from,
    /// because the pre-shared key has to be in the listener's candidate list
    /// before the other half dials in — which is also why this is the operation
    /// that must not run without a listener.
    async fn pair_create(&self, name: &str) -> Result<PairingData> {
        self.node().await?.pair_create(name)
    }

    /// Consume a code from the other device and prove the pairing works.
    ///
    /// The peer is persisted only after a complete session — the node's rule,
    /// and the reason a failed pairing leaves the paired-device list untouched.
    async fn pair_accept(&self, code: &str, addr: &str) -> Result<Vec<PeerInfo>> {
        self.node()
            .await?
            .pair_accept(&self.inner, code, addr)
            .await
    }

    /// Known peers, with a best-effort liveness flag from discovery.
    ///
    /// `online: false` means "not seen on the network", never "unreachable" — a
    /// device on a network without multicast is reachable by address and still
    /// reads as offline.
    async fn peers(&self) -> Result<Vec<PeerInfo>> {
        Ok(self.node().await?.peers())
    }

    /// Local and one-sided: the other device keeps its half until it also
    /// unpairs, which is why this cannot fail on an unreachable peer. What it
    /// does do is remove the pre-shared key from the listener's candidates.
    async fn unpair(&self, pairing_id: &str) -> Result<()> {
        match self.node().await?.unpair(pairing_id)? {
            true => Ok(()),
            false => Err(BackendError::NotFound(MSG_NO_PEER)),
        }
    }

    async fn revoke(&self, pairing_id: &str) -> Result<()> {
        self.node().await?.revoke(pairing_id)
    }

    async fn sync(&self, pairing_id: Option<&str>) -> Result<Vec<SyncResult>> {
        self.node().await?.sync(&self.inner, pairing_id).await
    }

    async fn discovered(&self) -> Result<Vec<DiscoveredDevice>> {
        Ok(self.node().await?.discovered())
    }

    /// Re-advertise and answer as [`Backend::discovered`] does.
    ///
    /// Best-effort, like every other discovery call: a republish that fails is
    /// logged and the current table is still returned, because what the user
    /// asked for was "show me what is out there".
    async fn rescan(&self) -> Result<Vec<DiscoveredDevice>> {
        let node = self.node().await?;
        node.republish();
        Ok(node.discovered())
    }

    /// Real, and always the defaults.
    ///
    /// The settings are still worth reading: `max_item_bytes`,
    /// `dedup_window_secs` and the sensitive TTL govern `copypaste_core::ingest`
    /// here exactly as they do in the daemon, so a screen that shows them is
    /// showing what this build actually does. `restart_required` is empty
    /// because nothing here is read once at start.
    async fn get_config(&self) -> Result<ConfigApplied> {
        self.blocking(move |inner| {
            Ok(ConfigApplied {
                config: inner.state.settings.clone(),
                restart_required: Vec::new(),
            })
        })
        .await
    }

    /// Refused, because there is nowhere to put the answer.
    ///
    /// A setting has to outlive the process to be a setting, and the file the
    /// daemon persists to is the daemon's. Accepting the write and keeping it
    /// in memory would be worse than refusing: the control would appear to
    /// work, and the value would be gone at the next launch.
    async fn set_config(&self, _patch: ConfigPatch) -> Result<ConfigApplied> {
        Err(BackendError::Unsupported(MSG_NO_SETTINGS))
    }

    async fn export(&self, limit: u32, include_sensitive: bool) -> Result<ExportData> {
        self.blocking(move |inner| transfer::export(inner, limit, include_sensitive))
            .await
    }

    async fn import(&self, items: Vec<ExportItem>) -> Result<ImportData> {
        self.blocking(move |inner| transfer::import(inner, items))
            .await
    }

    /// Needs the daemon's validate-then-swap, and must not be approximated.
    ///
    /// A backup is `VACUUM INTO` against the live pager, and a restore is a
    /// staged copy opened with this device's real key, integrity-checked and
    /// swapped inside one transaction (`server::dbadmin`). A plain file copy
    /// would look like both and be neither: it can capture a torn page, and it
    /// can replace a working history with a file that turns out not to open.
    /// Data loss is the worst outcome (CLAUDE.md rule 4), so this refuses
    /// rather than approximates.
    async fn backup(&self, _dest: &Path) -> Result<BackupData> {
        Err(BackendError::Unsupported(MSG_NO_BACKUP))
    }

    async fn restore(&self, _src: &Path) -> Result<()> {
        Err(BackendError::Unsupported(MSG_NO_BACKUP))
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
    pub(super) struct FakeClipboard(Mutex<Vec<String>>);

    impl Clipboard for Arc<FakeClipboard> {
        fn set_text(&self, text: &str) -> std::result::Result<(), &'static str> {
            self.0.lock().unwrap().push(text.to_string());
            Ok(())
        }
    }

    /// `pub(super)` so a submodule's tests build the same backend rather than a
    /// second fixture that could drift from this one.
    pub(super) fn backend() -> (EmbeddedBackend, Arc<FakeClipboard>, tempfile::TempDir) {
        let dir = tempfile::TempDir::new().unwrap();
        // Keeps the test off the real keystore and off the developer's own
        // history file.
        std::env::set_var("COPYPASTE_EPHEMERAL_KEY", "1");
        let clipboard = Arc::new(FakeClipboard::default());
        let backend = EmbeddedBackend::open(dir.path(), Box::new(Arc::clone(&clipboard)))
            .expect("the embedded backend should open under a temp dir");
        (backend, clipboard, dir)
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
        assert!(backend.list(50, None).await.unwrap().items.is_empty());
        assert!(backend
            .search("anything", 20)
            .await
            .unwrap()
            .items
            .is_empty());
    }

    /// B-1 / `CopyPaste-8ebg.57` on the platform with no daemon behind it: an
    /// item captured between two pages must not make the second page repeat a
    /// row or skip one. Both backends answer the same command, so both have to
    /// hold the same property.
    #[tokio::test]
    async fn a_capture_between_two_pages_neither_repeats_nor_skips_a_row() {
        let (backend, _clip, _dir) = backend();
        let mut original = Vec::new();
        for n in 0..6 {
            original.push(backend.add(&format!("clip {n}")).await.unwrap().id);
        }

        let first = backend.list(2, None).await.unwrap();
        let mut next = first.next_cursor.clone();
        assert!(next.is_some(), "a full page has to say where it stopped");

        backend.add("arrived mid-scroll").await.unwrap();

        let mut seen: Vec<String> = first.items.iter().map(|i| i.id.clone()).collect();
        while let Some(cursor) = next {
            let page = backend.list(2, Some(&cursor)).await.unwrap();
            seen.extend(page.items.iter().map(|i| i.id.clone()));
            next = page.next_cursor;
        }

        for id in &original {
            assert_eq!(
                seen.iter().filter(|s| *s == id).count(),
                1,
                "an item was skipped or repeated across the pages"
            );
        }
    }

    #[tokio::test]
    async fn a_forged_page_marker_is_refused_rather_than_restarting_the_list() {
        let (backend, _clip, _dir) = backend();
        backend.add("one").await.unwrap();
        for bad in ["", "not-hex!", "abcdef"] {
            assert!(
                matches!(
                    backend.list(10, Some(bad)).await.unwrap_err(),
                    BackendError::Invalid(_)
                ),
                "accepted {bad:?}"
            );
        }
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
        assert!(matches!(
            backend.sync(Some("nope")).await.unwrap_err(),
            BackendError::NotFound(_)
        ));
    }

    /// The whole point of the move: this build can mint a pairing, and the code
    /// it hands back reconstructs the key the listener will accept.
    #[tokio::test]
    async fn a_pairing_is_minted_and_stored_before_the_other_device_dials_in() {
        let (backend, _clip, _dir) = backend();
        let pairing = backend.pair_create("laptop").await.unwrap();
        assert!(!pairing.code.is_empty());
        assert_ne!(pairing.code, pairing.pairing_id);

        let token =
            copypaste_p2p::transport::PairingToken::parse(&pairing.code).expect("a valid code");
        assert_eq!(token.pairing_id(), pairing.pairing_id);

        let listed = backend.peers().await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "laptop");

        // ...and forgetting it removes the key from the listener's candidates.
        backend.unpair(&pairing.pairing_id).await.unwrap();
        assert!(backend.peers().await.unwrap().is_empty());
    }

    /// Revoking is the same gesture as unpairing until you try to use the code
    /// again, which is the only place the two differ.
    #[tokio::test]
    async fn a_revoked_pairing_cannot_be_minted_back_onto_this_device() {
        let (backend, _clip, _dir) = backend();
        let pairing = backend.pair_create("stolen phone").await.unwrap();

        backend.revoke(&pairing.pairing_id).await.unwrap();
        assert!(backend.peers().await.unwrap().is_empty());

        let store = copypaste_p2p::peers::PeerStore::open(&backend.inner.state.peers_path).unwrap();
        let token =
            copypaste_p2p::transport::PairingToken::parse(&pairing.code).expect("a valid code");
        assert!(
            store
                .upsert(copypaste_p2p::peers::Peer {
                    pairing_id: token.pairing_id(),
                    name: "stolen phone".into(),
                    psk: token.psk(),
                    last_addr: None,
                    last_seen_ms: 0,
                })
                .is_err(),
            "the code someone kept still enrols the pairing"
        );
    }

    /// A well-formed code for a device that is not listening must not leave a
    /// half-made pairing behind.
    #[tokio::test]
    async fn a_failed_pairing_stores_nothing() {
        let (backend, _clip, _dir) = backend();
        let code = copypaste_p2p::transport::PairingToken::generate().to_code();
        // Port 1 on loopback: nothing is listening, so the connect fails fast.
        assert!(backend.pair_accept(&code, "127.0.0.1:1").await.is_err());
        assert!(backend.peers().await.unwrap().is_empty());

        // A code that is not a code is refused without dialling anything.
        assert!(backend
            .pair_accept("not-a-code", "127.0.0.1:1")
            .await
            .is_err());
        assert!(backend.peers().await.unwrap().is_empty());
    }

    /// Discovery and sync answer rather than refuse, and an empty answer is a
    /// normal one — a container has no multicast and no peers.
    #[tokio::test]
    async fn discovery_and_sync_answer_with_nothing_rather_than_refusing() {
        let (backend, _clip, _dir) = backend();
        assert!(backend.sync(None).await.unwrap().is_empty());
        assert!(backend.discovered().await.unwrap().is_empty());
        assert!(backend.rescan().await.unwrap().is_empty());
    }

    /// A locally captured row stores no origin, and the wire item still has to
    /// name this device — the field is documented as never empty, and the merge
    /// tie-break reads the same substitution.
    #[tokio::test]
    async fn a_captured_item_is_attributed_to_this_device() {
        let (backend, _clip, _dir) = backend();
        let item = backend.add("mine").await.unwrap();
        assert!(!item.origin_device_id.is_empty());
        assert_eq!(
            item.origin_device_name.as_deref(),
            Some("CopyPaste phone"),
            "this device is in its own name registry"
        );

        let listed = backend.list(50, None).await.unwrap();
        assert_eq!(listed.items[0].origin_device_id, item.origin_device_id);
    }

    /// The refusals must read as structural, not transient: a user must not be
    /// told to try again at something that will never work in this build.
    #[tokio::test]
    async fn the_unimplemented_operations_say_so_plainly() {
        let (backend, _clip, _dir) = backend();
        for err in [
            backend
                .set_config(ConfigPatch::default())
                .await
                .unwrap_err(),
            backend.backup(Path::new("anywhere")).await.unwrap_err(),
            backend.restore(Path::new("anywhere")).await.unwrap_err(),
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

    /// Rung 0 has no value at all unless this works: the share sheet, the
    /// text-selection action and the tile all end here.
    #[tokio::test]
    async fn a_captured_clip_is_stored_searchable_and_readable_again() {
        let (backend, _clip, _dir) = backend();
        let item = backend.add("a shared note").await.unwrap();
        assert_eq!(item.content, "a shared note");

        assert_eq!(backend.list(50, None).await.unwrap().items.len(), 1);
        assert_eq!(backend.search("shared", 20).await.unwrap().items.len(), 1);
        assert_eq!(
            backend.get(&item.id).await.unwrap().content,
            "a shared note"
        );
    }

    /// The same text twice is one row. Not because this file says so — the
    /// dedup probe inside `copypaste_core::ingest` says so, and that is the
    /// point of calling it rather than re-deriving it.
    #[tokio::test]
    async fn the_same_clip_captured_twice_does_not_double() {
        let (backend, _clip, _dir) = backend();
        backend.add("same").await.unwrap();
        backend.add("same").await.unwrap();
        assert_eq!(backend.list(50, None).await.unwrap().items.len(), 1);
    }

    /// CLAUDE.md rule 4, the write-time layer: a detected secret is stored but
    /// never indexed, on this platform as on the other.
    #[tokio::test]
    async fn a_captured_secret_is_stored_and_stays_out_of_the_index() {
        let (backend, _clip, _dir) = backend();
        let item = backend.add("AKIAIOSFODNN7EXAMPLE").await.unwrap();
        assert!(item.is_sensitive, "the detector did not flag a known key");
        assert!(
            backend
                .search("AKIAIOSFODNN7EXAMPLE", 20)
                .await
                .unwrap()
                .items
                .is_empty(),
            "a sensitive item reached the search index"
        );
        // …and it is still the user's data: reachable by id, which is how the
        // reveal gesture gets to it.
        assert!(backend.get(&item.id).await.unwrap().is_sensitive);
    }

    #[tokio::test]
    async fn an_empty_capture_is_refused_without_storing_anything() {
        let (backend, _clip, _dir) = backend();
        assert!(matches!(
            backend.add("   ").await.unwrap_err(),
            BackendError::Invalid(_)
        ));
        assert!(backend.list(50, None).await.unwrap().items.is_empty());
    }

    /// A settings screen here shows what this build actually runs on, and says
    /// nothing needs a restart — there is no start to be read at.
    #[tokio::test]
    async fn the_settings_are_readable_even_though_they_are_not_writable() {
        let (backend, _clip, _dir) = backend();
        let applied = backend.get_config().await.unwrap();
        assert_eq!(applied.config, copypaste_ipc::ConfigData::default());
        assert!(applied.restart_required.is_empty());
    }

    #[tokio::test]
    async fn clearing_an_empty_history_is_a_success_reporting_zero() {
        let (backend, _clip, _dir) = backend();
        assert_eq!(backend.clear().await.unwrap(), 0);
    }
}

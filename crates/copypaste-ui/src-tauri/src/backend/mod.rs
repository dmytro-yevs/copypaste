//! One command surface, two backends.
//!
//! ADR-0002: macOS runs a background daemon and the app talks to it over a
//! `0600` Unix socket; Android hosts no daemon at all, so the same operations
//! have to run inside the app process against `copypaste-core` and
//! `copypaste-p2p` directly.
//!
//! # What this trait is protecting
//!
//! The identity of the *command surface*, not the implementations. If the two
//! platforms drift — a different name, a different argument, an extra field on
//! one side — the cost lands on the React code as a platform branch, and
//! platform branches in the view layer are how one app becomes two. ADR-0002
//! calls that identity "a hard requirement" and this is where it is enforced:
//!
//! * every operation is declared once, here, in terms of [`copypaste_ipc`]
//!   types — the single model of the contract that the daemon and the CLI also
//!   use, so there is no second set of DTOs (CLAUDE.md rule 1);
//! * `crate::commands` is written against the trait and contains **no `cfg`
//!   at all**, so the `#[tauri::command]` signatures are literally the same
//!   text on both platforms;
//! * the choice of implementation is one type alias, [`SelectedBackend`],
//!   resolved at compile time. No dynamic dispatch, no runtime probe, and no
//!   way for a command to ask which platform it is on.
//!
//! # Why a trait and not just two `cfg` modules
//!
//! Two `cfg` modules with matching function names compile fine while
//! disagreeing about argument order, optionality or return type — the compiler
//! only ever sees one of them. The trait makes both sides answer to one
//! declaration, so a change to the surface breaks the build of whichever
//! backend was not updated, including the one this host cannot run.
//!
//! # Errors
//!
//! Every method returns [`BackendError`], whose `Display` is the exact sentence
//! shown to the user. Two properties hold by construction:
//!
//! * **No filesystem path ever reaches one.** The socket path discloses the
//!   local username (CLAUDE.md rule 4). Anything that originates outside this
//!   crate goes through [`copypaste_ipc::redact::scrub_paths`] — the shared
//!   module, not a second copy of it.
//! * **No error carries content.** A failure to decrypt or store an item names
//!   the operation, never the item's text.

use std::path::Path;

use copypaste_ipc::{
    BackupData, ConfigApplied, ConfigPatch, DiscoveredDevice, EventData, ExportData, ExportItem,
    ImportData, Item, PairingData, PeerInfo, StatusData, SyncResult,
};
use tokio::sync::mpsc::Receiver;

pub mod error;

#[cfg(test)]
pub mod testing;

pub use error::BackendError;

/// Shorthand for what every backend method returns.
pub type Result<T> = std::result::Result<T, BackendError>;

/// A page of history, and what did not make it into one.
///
/// `skipped_undecryptable` counts rows that were read and would not open. v1
/// returned it and v2 dropped it, so a user with three unreadable rows saw
/// three fewer items and no explanation (parity finding 17, `CopyPaste-00zz`,
/// manifest 03 Q10). A shorter page is not an error — the rows that did open
/// are still the user's data — but it is not nothing either, and the difference
/// is exactly what a count carries.
///
/// It is a count and not a list of ids on purpose: nothing can be done with an
/// item that will not open, so an id would only invite a UI that offers an
/// action it cannot perform.
#[derive(Debug, Clone, Default)]
pub struct Page {
    pub items: Vec<Item>,
    pub skipped_undecryptable: u32,
}

impl From<copypaste_ipc::ItemPage> for Page {
    fn from(page: copypaste_ipc::ItemPage) -> Self {
        Self {
            items: page.items,
            skipped_undecryptable: page.skipped_undecryptable,
        }
    }
}

/// Everything the product can do, in one declaration.
///
/// The method set is the CLI's verb set, which is deliberate: the CLI is the
/// scripting and test surface for the same daemon, so any operation it can
/// reach and the app cannot is a feature with no UI (CLAUDE.md rule 6).
///
/// `async fn` in a trait rather than `#[async_trait]`: the backend is chosen at
/// compile time by [`SelectedBackend`], so there is never a trait object and
/// never a boxed future to pay for.
#[allow(async_fn_in_trait)]
pub trait Backend: Send + Sync + 'static {
    // ---- history ---------------------------------------------------------

    /// Most recent items, newest first; pinned ahead of unpinned.
    async fn list(&self, limit: u32, offset: u32) -> Result<Page>;

    /// Full-text search. Sensitive items are never indexed and never returned.
    async fn search(&self, query: &str, limit: u32) -> Result<Page>;

    /// Add an item directly, bypassing clipboard capture.
    async fn add(&self, content: &str) -> Result<Item>;

    /// Fetch one item by id, including a sensitive one's plaintext.
    ///
    /// The only route back to a secret, and it exists for the explicit reveal
    /// gesture. See `crate::model` for why nothing else needs one.
    async fn get(&self, id: &str) -> Result<Item>;

    /// Put an item's content on the system clipboard.
    ///
    /// Takes an id, not content, so a sensitive item can be copied without its
    /// plaintext ever entering the WebView.
    async fn copy(&self, id: &str) -> Result<Item>;

    async fn delete(&self, id: &str) -> Result<()>;

    /// Delete everything. Returns how many rows went.
    async fn clear(&self) -> Result<u64>;

    /// Pin or unpin. Returns the updated item so the caller need not re-list.
    async fn set_pinned(&self, id: &str, pinned: bool) -> Result<Item>;

    /// Rewrite the order of the pinned section.
    ///
    /// `ids` is the complete pinned list in the order the user wants it. The
    /// whole list rather than a move-one-item operation, because a reorder
    /// races a concurrent pin: "put B after A" is ambiguous once a third
    /// device has pinned C between them, while a full ordering either applies
    /// or is refused.
    ///
    /// Storage can already express this — `pin_order` is a `REAL` column that
    /// `set_pinned` appends to — so this is the only missing link in parity
    /// finding 19.
    async fn reorder_pinned(&self, ids: &[String]) -> Result<()>;

    // ---- state -----------------------------------------------------------

    async fn status(&self) -> Result<StatusData>;

    // ---- peers -----------------------------------------------------------

    /// Mint a pairing code on this device. The code is a secret and is
    /// returned exactly once.
    async fn pair_create(&self, name: &str) -> Result<PairingData>;

    /// Consume a code minted on another device and complete the pairing.
    async fn pair_accept(&self, code: &str, addr: &str) -> Result<Vec<PeerInfo>>;

    /// Known peers and when each was last reachable.
    async fn peers(&self) -> Result<Vec<PeerInfo>>;

    /// Forget a peer. Local and one-sided.
    async fn unpair(&self, pairing_id: &str) -> Result<()>;

    /// Sync with one peer, or with every known peer when `pairing_id` is
    /// `None`.
    async fn sync(&self, pairing_id: Option<&str>) -> Result<Vec<SyncResult>>;

    /// LAN devices discovery has heard from, paired or not.
    ///
    /// What turns pairing from "read out an address and a 52-character code"
    /// into "pick the device you can see". Everything in a
    /// [`DiscoveredDevice`] is unauthenticated mDNS chatter until the Noise
    /// handshake proves it, which is why the UI labels it as such (INV-15).
    async fn discovered(&self) -> Result<Vec<DiscoveredDevice>>;

    /// Re-advertise and re-browse, then answer as [`Backend::discovered`] does.
    async fn rescan(&self) -> Result<Vec<DiscoveredDevice>>;

    // ---- settings --------------------------------------------------------

    /// The backend's effective settings.
    async fn get_config(&self) -> Result<ConfigApplied>;

    /// Change some settings.
    ///
    /// A patch rather than a whole [`copypaste_ipc::ConfigData`], and the
    /// reason is a lost update: two Settings screens — or a screen and the CLI
    /// — each send the field they changed, so neither carries a stale copy of
    /// the other's. Rejected whole if any value is out of range, and a
    /// rejection leaves the backend on the configuration it already had.
    ///
    /// The reply names the fields that will not take effect until a restart, at
    /// the moment of the change rather than afterwards
    /// ([`copypaste_ipc::ConfigData::field_liveness`]).
    async fn set_config(&self, patch: ConfigPatch) -> Result<ConfigApplied>;

    // ---- transfer and database administration ----------------------------

    /// Read history out. `limit` of 0 means everything.
    ///
    /// [`ExportData`] carries three skip counts and they are the reason this
    /// returns the whole struct rather than a `Vec`: an export that quietly
    /// withheld a flagged item is one the user believes is complete.
    async fn export(&self, limit: u32, include_sensitive: bool) -> Result<ExportData>;

    /// Put items back, through the same ingest path a capture takes.
    async fn import(&self, items: Vec<ExportItem>) -> Result<ImportData>;

    /// Copy the encrypted database to `dest`, which must not already exist.
    ///
    /// A `&Path` rather than a `String` so a caller cannot hand this something
    /// that merely looks like one, and so the only way to obtain it is from the
    /// platform's own save panel.
    async fn backup(&self, dest: &Path) -> Result<BackupData>;

    /// Replace history with the contents of the backup at `src`.
    ///
    /// The most destructive operation in the product. The confirmation is the
    /// caller's to obtain — reaching here *is* the confirmation, so nothing
    /// below this point asks again.
    async fn restore(&self, src: &Path) -> Result<()>;

    // ---- change stream ---------------------------------------------------

    /// Subscribe to change events, replacing the 3-second poll.
    ///
    /// A channel rather than a callback so the subscription's lifetime is the
    /// receiver's: dropping it hangs up, and there is no way to leak a task
    /// that keeps a socket open for a screen that is gone.
    ///
    /// Returning [`BackendError::Unsupported`] is a supported answer, not a
    /// failure — the caller falls back to polling (parity finding 15). Push is
    /// an accelerator; the poll is the backstop, exactly as
    /// `copypaste_cloud::sync::cadence` treats Realtime.
    async fn watch(&self) -> Result<Receiver<EventData>>;
}

// ---------------------------------------------------------------------------
// selection
// ---------------------------------------------------------------------------

// The predicate names the feature as well as the target so the Android path is
// type-checkable on a Linux host (`--features embedded-backend`). Without that
// the entire in-process backend would be dead text on every machine anyone
// actually builds on, which is how the deleted Compose app got to ~2,500
// uncompiled lines.
#[cfg(any(target_os = "android", feature = "embedded-backend"))]
pub mod embedded;

#[cfg(not(target_os = "android"))]
pub mod daemon;

/// The backend this build talks to.
///
/// One alias, resolved at compile time. `crate::commands` names this type and
/// nothing else, which is what keeps `cfg` out of the command layer.
#[cfg(not(target_os = "android"))]
pub type SelectedBackend = daemon::DaemonBackend;

/// The backend this build talks to. Android links the core into the process.
#[cfg(target_os = "android")]
pub type SelectedBackend = embedded::EmbeddedBackend;

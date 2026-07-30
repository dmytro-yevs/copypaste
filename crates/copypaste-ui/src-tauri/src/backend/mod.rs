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

use copypaste_ipc::{Item, PairingData, PeerInfo, StatusData, SyncResult};

pub mod error;

pub use error::BackendError;

/// Shorthand for what every backend method returns.
pub type Result<T> = std::result::Result<T, BackendError>;

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
    async fn list(&self, limit: u32, offset: u32) -> Result<Vec<Item>>;

    /// Full-text search. Sensitive items are never indexed and never returned.
    async fn search(&self, query: &str, limit: u32) -> Result<Vec<Item>>;

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

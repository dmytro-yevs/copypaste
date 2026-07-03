//! # copypaste-ipc
//!
//! Shared IPC wire types for the CopyPaste daemon, UI, and CLI.
//!
//! This crate centralises the JSON-over-Unix-socket protocol so every
//! consumer (`copypaste-daemon`, `copypaste-ui`, `copypaste-cli`) speaks the
//! same shape. Pulling the types into a dedicated crate (arch-2 in the beta
//! plan) lets consumers depend on the wire contract without pulling in the
//! daemon binary.
//!
//! ## Scope
//!
//! * [`Request`] — incoming method call (`id`, `method`, `params`).
//! * [`Response`] — outgoing reply (`id`, `ok`, optional `data`/`error`/`error_code`,
//!   `protocol_version`).
//! * Stable [error code constants](#constants) for machine-readable branching.
//!
//! ## Consumers
//!
//! [`ErrorCode`], the `METHOD_*` constants, and the [`Request`]/[`Response`]
//! structs are consumed by the daemon, CLI, and UI. The typed structs use
//! `id: String` to match the actual daemon wire format (CopyPaste-crol: the
//! previous `id: u64` was an arch-2 planning artefact that was never aligned
//! with the live wire — fixed by migrating to `String` here).

#![deny(missing_docs)]
#![deny(rust_2018_idioms)]

// CopyPaste-crh3.53: BackoffScheduler lives here (a pure std::time::Duration
// state machine with no I/O) rather than in copypaste-sync, so copypaste-supabase
// can reuse it without transitively pulling in copypaste-core (SQLCipher +
// crypto + storage, 40+ crates). copypaste-ipc deps are serde/serde_json/home.
pub mod backoff;
pub mod error;
pub mod methods;
/// Canonical socket-path resolver — single source of truth for where the daemon socket lives.
pub mod paths;
pub mod request;
pub mod response;

pub use backoff::{
    BackoffScheduler, DEFAULT_BASE_DELAY, DEFAULT_MAX_DELAY, DEFAULT_SUCCESS_HOLD_THRESHOLD,
};
pub use error::ErrorCode;
pub use methods::{
    compute_sync_badge_state, compute_sync_badge_state_with_inflight, map_content_type_to_uti,
    AppConfig, AppConfigResponse, DbBackupRequest, DbBackupResponse, DbRestoreRequest,
    DbRestoreResponse, DbStatsResponse, GetSyncStatusResponse, PeerTransport, ResetDatabaseRequest,
    ResetDatabaseResponse, StatsResponse, StoreCloudPasswordRequest, StoreCloudPasswordResponse,
    SyncBadgeState, VacuumRequest, VacuumResponse, METHOD_ADD_FILE_ITEM,
    METHOD_CLOUD_TEST_CONNECTION, METHOD_COPY_ITEM, METHOD_COUNT, METHOD_DB_BACKUP,
    METHOD_DB_RESTORE, METHOD_DB_STATS, METHOD_DELETE_ALL, METHOD_DELETE_ITEM, METHOD_EXPORT,
    METHOD_GET_APP_ICON, METHOD_GET_CONFIG, METHOD_GET_ITEM_FILE, METHOD_GET_ITEM_IMAGE,
    METHOD_GET_ITEM_THUMBNAIL, METHOD_GET_OWN_DEVICE_INFO, METHOD_GET_OWN_FINGERPRINT,
    METHOD_GET_PRIVATE_MODE, METHOD_GET_SYNC_STATUS, METHOD_HISTORY_PAGE, METHOD_IMPORT,
    METHOD_LIST_DISCOVERED, METHOD_LIST_PEERS, METHOD_PAIR_ABORT, METHOD_PAIR_CONFIRM_SAS,
    METHOD_PAIR_GENERATE_QR, METHOD_PAIR_GET_SAS, METHOD_PAIR_PEER_WITH_PASSWORD,
    METHOD_PAIR_WITH_DISCOVERED, METHOD_PIN_ITEM, METHOD_POLL_PEER_EVENTS, METHOD_REORDER_PINNED,
    METHOD_RESCAN_DISCOVERED, METHOD_RESET_DATABASE, METHOD_REVOKE_ALL_PEERS,
    METHOD_REVOKE_AND_ROTATE, METHOD_REVOKE_PEER, METHOD_ROTATE_SYNC_KEY, METHOD_SEARCH,
    METHOD_SET_CONFIG, METHOD_SET_PRIVATE_MODE, METHOD_SET_SYNC_PASSPHRASE, METHOD_STATS,
    METHOD_STATUS, METHOD_STORE_CLOUD_PASSWORD, METHOD_UNPAIR_PEER, METHOD_VACUUM,
    METHOD_WATCH_SUBSCRIBE, SYNC_BADGE_RECENT_MS,
};
pub use request::Request;
pub use response::{
    Response, ERR_CODE_AUTH_FAILED, ERR_CODE_DAEMON_OFFLINE, ERR_CODE_INTERNAL_ERROR,
    ERR_CODE_INVALID_ARGUMENT, ERR_CODE_IPC_NOT_READY, ERR_CODE_MIGRATION_IN_PROGRESS,
    ERR_CODE_NOT_FOUND, ERR_CODE_NOT_IMPLEMENTED, ERR_CODE_RATE_LIMITED,
    ERR_CODE_REQUEST_TOO_LARGE, ERR_CODE_VERSION_MISMATCH,
};

/// Current IPC protocol version. Bump on breaking wire changes.
///
/// Consumers should set [`Request::protocol_version`] and
/// [`Response::protocol_version`] to this value; peers that receive a higher
/// value should reject the message with `error_code = "invalid_argument"`.
pub const PROTOCOL_VERSION: u32 = 1;

/// Lifetime of a QR-pairing token, in seconds — the single source of truth for
/// the pairing window.
///
/// The daemon's `generate_pairing_qr` handler stamps the QR code's
/// `expires_at = now + QR_PAIRING_TTL_SECS`, and the P2P bootstrap responder's
/// accept timeout (`copypaste_p2p::bootstrap::BOOTSTRAP_ACCEPT_TIMEOUT`) must
/// match it: the user scans the QR, confirms, and the initiator connects all
/// within this window. Both values previously hard-coded `120`; this constant
/// exists so they cannot drift independently.
///
/// Note (updated CopyPaste-8ebg.65): `copypaste-p2p` *does* now depend on
/// `copypaste-ipc` (see `crates/copypaste-p2p/Cargo.toml`) — the coupling was
/// judged acceptable because `copypaste-ipc` is a tiny types-only crate
/// (serde + serde_json, no `copypaste-core`). `BOOTSTRAP_ACCEPT_TIMEOUT`
/// (`crates/copypaste-p2p/src/bootstrap/mod.rs`) derives directly from
/// [`QR_PAIRING_TTL_SECS`] rather than carrying its own literal or a
/// `TODO(shared-const)`. This doc previously claimed the dependency did not
/// (and should not) exist; that was stale as of the P2P/IPC coupling change.
pub const QR_PAIRING_TTL_SECS: u64 = 120;

/// Maximum size, in bytes, of a single line-delimited-JSON IPC request/frame.
///
/// This is the single source of truth for the daemon's IPC request-size cap.
/// Both the Unix-socket server (`copypaste-daemon/src/ipc/consts.rs`) and the
/// frozen Windows named-pipe skeleton (`copypaste-daemon/src/ipc_win.rs`,
/// see `docs/adr/ADR-012-windows-frozen-homebrew-only.md`) enforce this same
/// 16 MiB ceiling, and the CLI's `import` command (`copypaste-cli/src/commands/import.rs`)
/// derives its own tighter pre-flight cap from it to fail fast before ever
/// opening the IPC connection. CopyPaste-8ebg.59/.65: previously each site
/// carried its own `16 * 1024 * 1024` literal kept in sync only by comments.
pub const MAX_IPC_REQUEST_BYTES: usize = 16 * 1024 * 1024;

/// Maximum length-delimited data-plane frame size, in bytes (16 MiB).
///
/// Single source of truth for `copypaste_sync::engine::MAX_FRAME_BYTES` and
/// `copypaste_p2p::transport::MAX_FRAME_BYTES` — the P2P sync protocol's
/// frame codec and the P2P transport's `LengthDelimitedCodec` ceiling must
/// stay in lockstep or a peer sending a maximally-sized image item would be
/// accepted by one layer and truncated/rejected by the other.
///
/// CopyPaste-1d5l.59: previously each crate carried its own
/// `16 * 1024 * 1024` literal, kept in sync only by a doc comment plus a
/// compile-time assertion in `copypaste-daemon/tests/frame_consts.rs`
/// (CopyPaste-w47w #1, still kept green). `copypaste-sync` and
/// `copypaste-p2p` do not depend on each other, but `copypaste-p2p` already
/// depends on `copypaste-ipc` (see its `Cargo.toml`) and `copypaste-sync`
/// gains the same tiny (serde + serde_json + home) dependency here, mirroring
/// the precedent set by [`QR_PAIRING_TTL_SECS`].
pub const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

/// Maximum plaintext blob size, in bytes, re-keyed onto the sync wire (8 MiB).
///
/// Single source of truth shared by:
///   * `copypaste-daemon`'s outbound re-key ceiling
///     (`sync_orch::rekey::outbound::SYNC_MAX_BLOB_BYTES`) — items above this
///     size are kept locally but skipped for sync (warned), never forwarded.
///   * `copypaste-relay`'s per-tier text-item quota
///     (`copypaste_relay::quota::Tier::max_item_bytes("text")`) — sized to
///     match so a 1-8 MiB text item that clears the sync ceiling is not
///     separately rejected 413 by the relay.
///
/// CopyPaste-1d5l.58: previously each site carried its own
/// `8 * 1024 * 1024` literal kept in sync only by cross-referencing comments.
pub const SYNC_MAX_BLOB_BYTES: usize = 8 * 1024 * 1024;

/// Default maximum decoded ciphertext size, in bytes, for a single relay item
/// (10 MiB) — the relay's own configurable request-body / per-item-quota
/// ceiling.
///
/// Single source of truth shared by:
///   * `copypaste-relay`'s `RelayConfig::default().max_item_bytes` (overridable
///     at runtime via `RELAY_MAX_ITEM_BYTES`).
///   * `copypaste-relay`'s per-tier image/file quota
///     (`copypaste_relay::quota::Tier::max_item_bytes("image" | "file")`),
///     which must match the operator body cap — otherwise file payloads
///     between 1-10 MiB are wrongly rejected 413.
///
/// CopyPaste-1d5l.58: previously each site carried its own
/// `10 * 1024 * 1024` literal kept in sync only by comments. Distinct from
/// [`SYNC_MAX_BLOB_BYTES`] (8 MiB) and the P2P transport's 16 MiB
/// [`MAX_FRAME_BYTES`] — those are different ceilings for different purposes
/// (sync-eligibility vs. transport framing) and are intentionally NOT
/// collapsed into this constant.
pub const RELAY_MAX_ITEM_BYTES: usize = 10 * 1024 * 1024;

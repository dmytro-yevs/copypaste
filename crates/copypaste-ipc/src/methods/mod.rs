//! Method-name constants and typed request/response payloads for individual
//! IPC methods.
//!
//! The daemon dispatches on the bare method-name string (see
//! `copypaste-daemon::ipc`), so these constants are the single shared source of
//! truth for the wire-level method name. Clients (UI, CLI) reference the
//! constant instead of re-typing the string literal, so a rename is a
//! compile-time break rather than a silent runtime mismatch.
//!
//! ## Module layout
//!
//! | Sub-module | Contents |
//! |---|---|
//! | [`badge`]   | `SyncBadgeState`, `SYNC_BADGE_RECENT_MS`, badge-derivation fns |
//! | [`clipboard`] | Clipboard-item METHOD_* constants + `StatsResponse` |
//! | [`sync`]    | Sync-key + cloud-sync METHOD_* constants + their DTOs |
//! | [`db`]      | DB-maintenance METHOD_* constants + their DTOs |
//! | [`pairing`] | Pairing + LAN/SAS + peer-management METHOD_* constants |
//! | [`config`]  | `AppConfig` IPC wire type |

pub mod badge;
pub mod clipboard;
pub mod config;
pub mod db;
pub mod pairing;
pub mod sync;

// Re-export everything so that `copypaste_ipc::methods::X` and the lib.rs
// `pub use methods::{...}` continue to resolve unchanged.

pub use badge::{
    compute_sync_badge_state, compute_sync_badge_state_with_inflight, SyncBadgeState,
    SYNC_BADGE_RECENT_MS,
};

pub use clipboard::{
    map_content_type_to_uti, StatsResponse, METHOD_ADD_FILE_ITEM, METHOD_COPY, METHOD_COPY_ITEM,
    METHOD_COUNT, METHOD_DELETE, METHOD_DELETE_ALL, METHOD_DELETE_ITEM, METHOD_EXPORT,
    METHOD_GET_APP_ICON, METHOD_GET_ITEM_FILE, METHOD_GET_ITEM_IMAGE, METHOD_GET_ITEM_THUMBNAIL,
    METHOD_GET_OWN_DEVICE_INFO, METHOD_GET_OWN_FINGERPRINT, METHOD_GET_PRIVATE_MODE,
    METHOD_HISTORY_PAGE, METHOD_IMPORT, METHOD_LIST, METHOD_PIN_ITEM, METHOD_SEARCH,
    METHOD_SET_PRIVATE_MODE, METHOD_STATS, METHOD_STATUS,
};

pub use config::{AppConfig, AppConfigResponse};

pub use db::{
    DbBackupRequest, DbBackupResponse, DbRestoreRequest, DbRestoreResponse, DbStatsResponse,
    ResetDatabaseRequest, ResetDatabaseResponse, VacuumRequest, VacuumResponse, METHOD_DB_BACKUP,
    METHOD_DB_RESTORE, METHOD_DB_STATS, METHOD_RESET_DATABASE, METHOD_VACUUM,
};

pub use pairing::{
    PeerTransport, METHOD_LIST_DISCOVERED, METHOD_LIST_PEERS, METHOD_PAIR_ABORT,
    METHOD_PAIR_CONFIRM_SAS, METHOD_PAIR_GENERATE_QR, METHOD_PAIR_GET_SAS,
    METHOD_PAIR_PEER_WITH_PASSWORD, METHOD_PAIR_WITH_DISCOVERED, METHOD_POLL_PEER_EVENTS,
    METHOD_REORDER_PINNED, METHOD_RESCAN_DISCOVERED, METHOD_REVOKE_ALL_PEERS, METHOD_REVOKE_PEER,
    METHOD_UNPAIR_PEER,
};

pub use sync::{
    GetSyncStatusResponse, StoreCloudPasswordRequest, StoreCloudPasswordResponse,
    METHOD_CLOUD_TEST_CONNECTION, METHOD_GET_CONFIG, METHOD_GET_SYNC_STATUS,
    METHOD_REVOKE_AND_ROTATE, METHOD_ROTATE_SYNC_KEY, METHOD_SET_CONFIG,
    METHOD_SET_SYNC_PASSPHRASE, METHOD_STORE_CLOUD_PASSWORD,
};

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
//! Today only [`ErrorCode`] and the `METHOD_*` constants are actively consumed
//! by the daemon/UI/CLI. The [`Request`] and [`Response`] structs define the
//! *proposed* arch-2 wire shape but are **not yet live on the daemon wire**:
//! the daemon still uses `id: String` whereas this crate uses `id: u64`.
//!
//! **Do NOT change `id: u64` to `String` here without a coordinated daemon
//! migration** — the mismatch is intentional (arch-2 plan) and tracked as a
//! TODO. Consumers that need the live daemon wire today should use the daemon's
//! own `protocol` module directly until the migration is complete.

#![deny(missing_docs)]
#![deny(rust_2018_idioms)]

pub mod error;
pub mod methods;
pub mod request;
pub mod response;

pub use error::ErrorCode;
pub use methods::{
    ResetDatabaseRequest, ResetDatabaseResponse, METHOD_CLOUD_TEST_CONNECTION, METHOD_COPY,
    METHOD_COUNT, METHOD_DELETE, METHOD_DELETE_ALL, METHOD_EXPORT, METHOD_GET_CONFIG,
    METHOD_GET_PRIVATE_MODE, METHOD_GET_SYNC_STATUS, METHOD_IMPORT, METHOD_LIST,
    METHOD_PAIR_GENERATE_QR, METHOD_PIN_ITEM, METHOD_RESET_DATABASE, METHOD_SEARCH,
    METHOD_SET_CONFIG, METHOD_SET_PRIVATE_MODE, METHOD_STATS, METHOD_STATUS,
};
pub use request::Request;
pub use response::{
    Response, ERR_CODE_AUTH_FAILED, ERR_CODE_DAEMON_OFFLINE, ERR_CODE_INTERNAL_ERROR,
    ERR_CODE_INVALID_ARGUMENT, ERR_CODE_IPC_NOT_READY, ERR_CODE_MIGRATION_IN_PROGRESS,
    ERR_CODE_NOT_FOUND, ERR_CODE_NOT_IMPLEMENTED, ERR_CODE_RATE_LIMITED, ERR_CODE_VERSION_MISMATCH,
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
/// Note: `copypaste-p2p` does not (and should not) depend on `copypaste-ipc`,
/// so its `BOOTSTRAP_ACCEPT_TIMEOUT` carries a `TODO(shared-const)` pointing
/// here rather than referencing this directly. Any consumer that *does* depend
/// on this crate (the daemon) should derive the QR TTL from this constant.
pub const QR_PAIRING_TTL_SECS: u64 = 120;

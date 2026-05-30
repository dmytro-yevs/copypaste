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
pub use methods::{ResetDatabaseRequest, ResetDatabaseResponse, METHOD_RESET_DATABASE};
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

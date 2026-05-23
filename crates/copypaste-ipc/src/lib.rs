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
//! ## Beta migration note
//!
//! This crate currently ships the *new* wire shape proposed in arch-2:
//! numeric `id: u64` and an explicit `protocol_version: u32` field. The
//! existing daemon/ui/cli code still uses the legacy `id: String` shape from
//! `copypaste-daemon::protocol`. Consumer migration happens in Wave 2 / 3;
//! this wave only lands the crate skeleton.

#![deny(missing_docs)]
#![deny(rust_2018_idioms)]

pub mod error;
pub mod request;
pub mod response;
pub mod types;

pub use error::ErrorCode;
pub use request::Request;
pub use response::{
    Response, ERR_CODE_AUTH_FAILED, ERR_CODE_INTERNAL_ERROR, ERR_CODE_INVALID_ARGUMENT,
    ERR_CODE_IPC_NOT_READY, ERR_CODE_NOT_FOUND, ERR_CODE_NOT_IMPLEMENTED,
};

/// Current IPC protocol version. Bump on breaking wire changes.
///
/// Consumers should set [`Request::protocol_version`] and
/// [`Response::protocol_version`] to this value; peers that receive a higher
/// value should reject the message with `error_code = "invalid_argument"`.
pub const PROTOCOL_VERSION: u32 = 1;

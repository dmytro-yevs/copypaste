//! Generated Flutter boundary over the existing CopyPaste runtime contracts.
//!
//! Desktop calls retain the daemon's private IPC transport. Android deliberately
//! has no IPC or clipboard fallback: its in-process runtime is wired separately
//! before any product call is enabled.

mod api;
mod client;
mod native_pairing_abi;
mod protected;
mod runtime;
#[cfg(target_os = "android")]
mod runtime_android;

pub use api::*;

mod frb_generated;

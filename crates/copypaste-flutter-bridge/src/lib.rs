//! Generated Flutter boundary over the existing CopyPaste runtime contracts.
//!
//! Desktop calls retain the daemon's private IPC transport. Android owns an
//! in-process history runtime and a separate bounded inference service.

mod api;
mod client;
#[cfg(target_os = "android")]
mod inference_android;
mod native_pairing_abi;
mod protected;
mod runtime;
#[cfg(target_os = "android")]
mod runtime_android;

pub use api::*;

mod frb_generated;

//! IPC request wire type.

use serde::{Deserialize, Serialize};

/// A single JSON-RPC-style request sent from a client (UI / CLI) to the
/// daemon over the Unix domain socket.
///
/// # Fields
///
/// * `id` — client-chosen correlation identifier serialised as a JSON
///   **string** (e.g. `"1"`, `"req-42"`). The daemon echoes the same string
///   back in the matching [`crate::Response`] so clients can correlate replies
///   on a multiplexed connection. Using `String` (not `u64`) keeps the typed
///   schema consistent with the actual wire format produced by both the daemon
///   (`protocol.rs`) and the CLI (`ipc.rs`), which have always used string ids.
/// * `method` — RPC method name (e.g. `"list"`, `"pin"`, `"push"`).
/// * `params` — opaque JSON payload; shape depends on `method`.
/// * `protocol_version` — wire version, see [`crate::PROTOCOL_VERSION`].
///   Defaults to `0` for older peers that pre-date this field.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Request {
    /// Client-chosen correlation id, serialised as a JSON string and echoed
    /// in the matching response. Callers typically use a monotonic decimal
    /// counter converted to string (e.g. `"1"`, `"2"`, …).
    pub id: String,
    /// RPC method name (e.g. `"list"`, `"pin"`, `"push"`).
    pub method: String,
    /// Opaque method-specific JSON payload.
    #[serde(default)]
    pub params: serde_json::Value,
    /// Wire protocol version. See [`crate::PROTOCOL_VERSION`].
    #[serde(default)]
    pub protocol_version: u32,
}

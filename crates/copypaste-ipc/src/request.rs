//! IPC request wire type.

use serde::{Deserialize, Serialize};

/// A single JSON-RPC-style request sent from a client (UI / CLI) to the
/// daemon over the Unix domain socket.
///
/// # Fields
///
/// * `id` — monotonically increasing client-chosen identifier; the daemon
///   echoes it back on the matching [`crate::Response`] so clients can
///   correlate replies on a multiplexed connection.
/// * `method` — RPC method name (e.g. `"list"`, `"pin"`, `"push"`).
/// * `params` — opaque JSON payload; shape depends on `method`.
/// * `protocol_version` — wire version, see [`crate::PROTOCOL_VERSION`].
///   Defaults to `0` for older peers that pre-date this field.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Request {
    /// Client-chosen correlation id, echoed in the matching response.
    pub id: u64,
    /// RPC method name (e.g. `"list"`, `"pin"`, `"push"`).
    pub method: String,
    /// Opaque method-specific JSON payload.
    #[serde(default)]
    pub params: serde_json::Value,
    /// Wire protocol version. See [`crate::PROTOCOL_VERSION`].
    #[serde(default)]
    pub protocol_version: u32,
}

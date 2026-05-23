//! Shared enum / struct types referenced by both [`crate::Request`] params and
//! [`crate::Response`] payloads.
//!
//! Currently empty — added in this wave as a placeholder so consumer crates
//! (daemon, ui, cli) have a single module to extend when Wave 2/3 migrates
//! payload structs (e.g. `ItemSummary`, `PeerInfo`) off `serde_json::Value`.

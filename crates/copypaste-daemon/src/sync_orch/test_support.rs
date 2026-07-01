//! Shared test fixtures for `sync_orch`'s submodule test suites.
//!
//! Extracted out of the former single flat `sync_orch/mod.rs` test module
//! (ADR-017, CopyPaste-vp63.3) so `catchup.rs`, `merge_tests.rs`, `rekey/`'s
//! test modules, and `mod.rs`'s own `run()` tests can all share the same
//! `make_db`/`make_wire` helpers without duplicating them.

use std::sync::Arc;
use tokio::sync::Mutex;

use copypaste_sync::protocol::WireItem;

pub(crate) fn make_db() -> Arc<Mutex<copypaste_core::Database>> {
    Arc::new(Mutex::new(
        copypaste_core::Database::open_in_memory().expect("in-memory DB must open"),
    ))
}

pub(crate) fn make_wire(id: &str, lamport: i64, content: u8) -> WireItem {
    WireItem {
        deleted: false,
        pinned: false,
        pin_order: None,
        id: id.to_string(),
        item_id: format!("{id}-iid"),
        content_type: "text".to_string(),
        content: Some(vec![content]),
        content_nonce: Some(vec![0u8; 24]),
        blob_ref: None,
        is_sensitive: false,
        lamport_ts: lamport,
        wall_time: 1_700_000_000_000 + lamport,
        expires_at: None,
        app_bundle_id: None,
        origin_device_id: "remote-device".to_string(),
        key_version: 2,
        file_name: None,
        mime: None,
    }
}

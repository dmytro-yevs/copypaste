//! Persistent storage for paired P2P devices.
//!
//! Paired device records are stored as a JSON file alongside the database.
//! The file is read at daemon startup and written whenever the pairing list
//! changes.
//!
//! Split (ADR-017, CopyPaste-vp63.4) into:
//! - [`model`] — the `PairedDevice` record + canonical-fingerprint lookup helpers.
//! - [`store`] — `load_peers`/`save_peers` + the shared atomic-0600 JSON writer.
//! - [`updates`] — per-peer live-metadata updaters (`update_peer_meta`, etc.).
//! - [`pending_unpair`] — the durable `pending_unpair.json` queue (Gap A).
//!
//! This file is a thin facade: it re-exports the exact same public surface the
//! flat `peers.rs` file exposed so every `crate::peers::X` call site (ipc,
//! p2p, sync_orch) keeps compiling unchanged.

mod model;
mod pending_unpair;
mod store;
mod updates;

pub use model::PairedDevice;
// CopyPaste-vp63.52: re-export the canonical-fingerprint find/retain helpers
// so sibling modules (`p2p::unpair`, `ipc::pairing_ops_persist`) can reuse the
// SAME implementation instead of hand-rolling the identical
// `canonical_fingerprint(&p.fingerprint) == target` predicate.
pub(crate) use model::{find_mut_by_fingerprint, retain_not_fingerprint};
pub use pending_unpair::{
    load_pending_unpairs, pending_unpair_path_for, queue_pending_unpair, remove_pending_unpair,
    save_pending_unpairs, PendingUnpair,
};
pub use store::{load_peers, save_peers};
pub use updates::{
    touch_sync_times, update_peer_address, update_peer_device_info, update_peer_meta,
};

// Use the canonical fingerprint normaliser from the IPC module — single
// implementation, zero drift risk. The local alias keeps call-site churn
// minimal; any future rename only touches this one line.
use crate::ipc::canonical_fingerprint as canonical_fp;

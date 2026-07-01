//! Sync-item inbox: types, SSE notifications, push/pull/delete.
//!
//! [`SyncItem`] is the encrypted envelope stored per-device.  The per-device
//! [`tokio::sync::broadcast`] channel carries wake ticks for open SSE streams.
//! Push/pull/delete and SSE management are implemented on [`super::RelayStore`]
//! across the submodules below, each owning one cohesive cluster:
//!
//! - [`types`]  — `SyncItem`, `SYNC_NOTIFY_CHANNEL_CAP`
//! - [`sse`]    — subscribe/count/notify (SSE wake channel)
//! - [`push`]   — `push_item` (test helper) / `push_item_decoded` (prod hot path)
//! - [`pull`]   — `pull_items` (keyset pagination)
//! - [`delete`] — `delete_item`

mod delete;
mod pull;
mod push;
mod sse;
mod types;

pub use types::SyncItem;

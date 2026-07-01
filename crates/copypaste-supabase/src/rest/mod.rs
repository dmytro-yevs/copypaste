//! Supabase PostgREST client for clipboard item CRUD operations.
//!
//! This module provides the [`RestClient`] for interacting with the Supabase
//! `clipboard_items` table via the PostgREST HTTP API. It is separate from the
//! GoTrue auth client ([`crate::auth::AuthClient`]) and the Realtime WebSocket
//! client ([`crate::realtime::RealtimeClient`]).
//!
//! # Design notes
//!
//! - All mutations use `?on_conflict=item_id&resolution=merge-duplicates` (upsert)
//!   so that concurrent writes from multiple devices converge via LWW (last-write-
//!   wins using `lamport_ts`).
//! - The `deleted` flag is **always** included in upserts (CopyPaste-kgs7). A
//!   missing `deleted` column in an INSERT would allow a previously-deleted
//!   tombstone row to be resurrected with `deleted = false` (the Postgres column
//!   default). By explicitly sending `deleted: true` for tombstones and
//!   `deleted: false` for live items, the upsert always propagates the correct
//!   soft-delete state.
//! - `pinned` and `pin_order` are also always included (CopyPaste-vqm0) so that
//!   pin state propagates to all devices through the cloud.
//! - Re-encryption of cloud items on passphrase change (CopyPaste-vvsf): the
//!   [`RestClient::reencrypt_all_cloud_items`] method fetches all rows for the
//!   current user, re-encrypts each payload under the new key, and upserts the
//!   updated rows back. This is a best-effort bulk operation — the caller is
//!   responsible for ensuring that no other sync operation runs concurrently.
//!
//! # Module layout
//!
//! | Sub-module | Responsibility |
//! |---|---|
//! | [`client`] | [`RestClient`] construction, shared state, redacting `Debug` impl |
//! | [`http`] | Table URL / auth-header helpers, PostgREST error-body decoding |
//! | [`read`] | [`RestClient::list_cloud_items`] |
//! | [`write`] | [`RestClient::replace_cloud_item_by_item_id`], [`RestClient::delete_cloud_item_by_item_id`] |
//! | [`reencrypt`] | [`RestClient::reencrypt_all_cloud_items`] (CopyPaste-vvsf) |

mod client;
mod error;
mod http;
mod read;
mod reencrypt;
mod write;

#[cfg(test)]
mod test_support;

pub use client::RestClient;
pub use error::{RestError, RestResult};

//! Cloud sync over Supabase.
//!
//! # Why Supabase and not our own relay
//!
//! v1 shipped `copypaste-relay`: ~12,000 lines providing device registration,
//! an encrypted-blob inbox with quota and TTL, cursor pagination, SSE push,
//! bearer auth and rate limiting. An audit of it found the request handling and
//! the rate limiting were fine, and that the bulk of the complexity — a
//! hand-rolled write-behind cache with its own durable retry queue and
//! out-of-order counter reconciliation — existed to work around holding one
//! mutex across a slow SQLite write.
//!
//! All of it is a service that already exists. Postgres gives the inbox, RLS
//! gives the authorisation, Realtime gives the push, GoTrue gives the accounts.
//!
//! # The server never sees plaintext
//!
//! Rows are sealed on the client under a key derived from the user's passphrase
//! with Argon2id, and the passphrase never leaves the device. Supabase holds
//! ciphertext and metadata only. This is what makes hosting the data somewhere
//! we do not control an acceptable trade rather than a regression from v1,
//! where the relay likewise only ever held opaque blobs.
//!
//! RLS is the second layer, not the first: a misconfigured policy exposes rows
//! that are still unreadable.
//!
//! # The metadata is signed, because encryption cannot order anything
//!
//! The fields the merge orders on travel in the clear — the backend pages on
//! them. Whoever can write to the table could therefore decide which version of
//! an item every device keeps, without ever decrypting anything. Every row
//! carries an HMAC over those fields and the ciphertext, under a key derived
//! from the same passphrase, and a row that does not verify is refused before it
//! reaches the merge. See [`crypto::sign`].

#![forbid(unsafe_code)]

pub mod auth;
pub mod crypto;
pub mod realtime;
pub mod rest;
pub mod sync;

pub use auth::{AuthError, Session, SupabaseAuth};
pub use crypto::{decrypt_row, derive_sync_key, encrypt_row, CloudCrypto, SyncKey};
pub use realtime::{RealtimeError, RealtimeEvent, RealtimeSubscription};
pub use rest::{CloudItem, RestError, SupabaseRest};
pub use sync::{CloudSync, SyncError, SyncStats};

/// Where the deployment lives and which anon key to present.
#[derive(Debug, Clone)]
pub struct CloudConfig {
    /// e.g. `https://<project>.supabase.co`
    pub url: String,
    /// The publishable anon key. Not a secret in the usual sense — RLS is what
    /// restricts access — so it may be shipped in the binary.
    pub anon_key: String,
}

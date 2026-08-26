//! What the cloud account surface reports.
//!
//! Split from its siblings because it is the one payload group that grows with
//! the sync work rather than with the IPC protocol, and because everything in
//! it is answerable without a URL, a token or a path.

use serde::{Deserialize, Serialize};

/// What `copypaste cloud status` reports.
///
/// No URL, no email domain guessing, no token, and no path: everything here is
/// either a flag or something the user typed themselves.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
pub struct CloudStatusData {
    /// A deployment URL and anon key are configured on the daemon.
    pub configured: bool,
    /// A session is held for an account.
    pub signed_in: bool,
    /// The sync key is unlocked, so rows can actually be sealed and opened.
    /// Distinct from `signed_in`: a session can exist without a passphrase and
    /// silently synced nothing.
    pub key_ready: bool,
    /// The signed-in account, as the user typed it.
    pub email: Option<String>,
    /// When the last round completed, in Unix milliseconds.
    pub last_sync_ms: Option<i64>,
    /// Why the last round failed. A fixed sentence — never a path or a token.
    pub last_error: Option<String>,
    /// The current adaptive idle interval, in seconds.
    pub poll_interval_secs: u64,
    /// Local rows this device could not open, so they are not going to the
    /// account. A count and nothing else: the id, the path and the content of
    /// such a row never reach a client (`AGENTS.md` rule 4).
    ///
    /// Not an error — the rest of sync is working — but it is the only thing
    /// that separates "everything is uploaded" from "everything but these".
    #[serde(default)]
    pub unreadable_uploads: u32,
}

/// What one cloud round did. Mirrors `copypaste_cloud::SyncStats`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
pub struct CloudSyncData {
    pub uploaded: u32,
    pub tombstoned: u32,
    pub downloaded: u32,
    pub applied: u32,
    /// Withheld from upload because the detector flagged them. Never zero by
    /// accident: this is the count the user can check the rule against.
    pub skipped_sensitive: u32,
    pub skipped_undecryptable: u32,
    /// Remote rows whose metadata signature did not verify, refused before the
    /// merge saw them.
    ///
    /// **Not a routine number.** A non-zero value means something wrote a row
    /// into the account that does not hold the sync passphrase, which is the
    /// one count here that distinguishes an attack from a quiet day. It is the
    /// only reason a client can give for a round that downloaded rows and
    /// applied none of them.
    ///
    /// Additive within v2: a daemon built before this field omits it, so a newer
    /// client must supply the safe zero value while decoding.
    #[serde(default)]
    pub skipped_forged: u32,
    pub skipped_future: u32,
    /// Local items withheld because they are over the per-item upload cap
    /// (8 MiB for text, 10 MiB otherwise).
    ///
    /// Withheld, never deleted: the item stays on this device in full, it
    /// simply does not reach the account. It is counted for the same reason
    /// `skipped_sensitive` is — a round that uploaded fewer items than the user
    /// expected has to be able to say why — and because without it an item that
    /// will *never* reach the other device is indistinguishable from one that
    /// is merely still in the queue. [`Item::too_large_to_sync`] is the
    /// per-item half of the same answer.
    pub skipped_too_large: u32,
}

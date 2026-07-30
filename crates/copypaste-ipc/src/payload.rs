//! The typed payloads a reply can carry.
//!
//! Split from the envelope in `lib.rs` only for size; the decode ordering rule
//! that governs them lives on [`crate::ResponseData`], which is where a reader
//! deciding where to add a variant will be looking.

use serde::{Deserialize, Serialize};

/// One page of history, and how much of it could not be shown.
///
/// `skipped_undecryptable` is on the wire because the alternative is what v1
/// shipped: a page silently one item shorter, with the reason only in the
/// daemon's log (`CopyPaste-00zz`). A client that reads it can say "3 items
/// could not be read"; a client that ignores it is no worse off than before.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemPage {
    pub items: Vec<Item>,
    pub skipped_undecryptable: u32,
}

/// One item in an export, and the unit an import consumes.
///
/// Deliberately not [`Item`]: an item's id is bound into its ciphertext as
/// associated data, so an imported item is necessarily a *new* item with a new
/// id. Carrying the old one would invite a client to expect it back.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportItem {
    pub content: String,
    pub content_type: String,
    /// Milliseconds since the Unix epoch. Preserved across an import, so a
    /// restored history keeps its order and its ages.
    pub created_at: i64,
    pub pinned: bool,
    /// On import this is a **floor**, never a ceiling: the daemon runs the
    /// detector over the content again and ORs the two, so an edited export
    /// cannot smuggle a credential back in marked clean (manifest 04, PG-26).
    pub is_sensitive: bool,
}

/// What an export contains, and everything it left out.
///
/// The three skip counts are always present, including when they are zero. A
/// count that only appears when it is non-zero is one nobody knows to look for,
/// and a silent export that dropped items is worse than one that says so.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportData {
    pub items: Vec<ExportItem>,
    /// Items whose `content_type` is not text. v2 captures text only, but a
    /// peer or the cloud can deliver something else.
    pub skipped_non_text: u32,
    /// Items the detector flagged, withheld because `include_sensitive` was
    /// false.
    pub skipped_sensitive: u32,
    pub skipped_undecryptable: u32,
}

/// What an import did. `skipped` counts items the store deduplicated against
/// something it already held — not failures: a malformed batch is refused
/// whole, before anything is written.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ImportData {
    pub inserted: u32,
    pub skipped: u32,
}

/// A completed backup. No path: the client supplied it and already knows it,
/// and echoing one back is how it ends up in a log (CLAUDE.md rule 4).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BackupData {
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredData {
    pub devices: Vec<DiscoveredDevice>,
}

/// A device seen on the LAN.
///
/// Presence is not trust: everything here is what an unauthenticated mDNS
/// record claimed, and only the Noise handshake proves any of it. `paired` is
/// resolved locally by looking the pairing id up in this device's own list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredDevice {
    pub pairing_id: String,
    pub name: String,
    /// `host:port` to hand to `pair accept`.
    pub addr: String,
    pub last_seen_ms: i64,
    pub paired: bool,
}

/// What `copypaste cloud status` reports.
///
/// No URL, no email domain guessing, no token, and no path: everything here is
/// either a flag or something the user typed themselves.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudStatusData {
    /// A deployment URL and anon key are configured on the daemon.
    pub configured: bool,
    /// A session is held for an account.
    pub signed_in: bool,
    /// The sync key is unlocked, so rows can actually be sealed and opened.
    /// Distinct from `signed_in`: v1 could be signed in with no passphrase and
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
}

/// What one cloud round did. Mirrors `copypaste_cloud::SyncStats`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CloudSyncData {
    pub uploaded: u32,
    pub tombstoned: u32,
    pub downloaded: u32,
    pub applied: u32,
    /// Withheld from upload because the detector flagged them. Never zero by
    /// accident: this is the count the user can check the rule against.
    pub skipped_sensitive: u32,
    pub skipped_undecryptable: u32,
    pub skipped_future: u32,
}

/// A freshly minted pairing, returned once and never retrievable again.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingData {
    /// The transferable form of the pre-shared key. Read this to the other
    /// device. It is secret: anyone holding it can pair, so it must not be
    /// logged, and the UI should treat it like a password.
    pub code: String,
    /// Non-secret identifier for the pairing. Safe to log and to display.
    pub pairing_id: String,
    /// Where the other device should connect, when it can be determined.
    pub listen_addr: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub pairing_id: String,
    pub name: String,
    pub last_addr: Option<String>,
    pub last_seen_ms: i64,
    /// True when the peer is currently visible on the network. Discovery is a
    /// convenience, so `false` means "not seen", never "unreachable".
    pub online: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub pairing_id: String,
    pub name: String,
    pub sent: u32,
    pub received: u32,
    /// Present when this peer failed; the rest of the run still reports.
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusData {
    pub version: String,
    pub protocol_version: u32,
    pub item_count: u64,
    pub capture_running: bool,
    /// Which clipboard backend is live — the real pasteboard or the fake used
    /// on non-macOS hosts and in tests. Surfaced so a demo cannot be mistaken
    /// for the real thing.
    pub clipboard_backend: String,
}

/// An item as seen by clients. Content is plaintext here: it is decrypted by
/// the daemon on the way out, and the socket is `0600`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub content: String,
    pub content_type: String,
    /// Milliseconds since the Unix epoch.
    pub created_at: i64,
    pub pinned: bool,
    /// True when the detector matched. Sensitive items are excluded from the
    /// search index at write time, at read time, and by a purge pass.
    pub is_sensitive: bool,
}

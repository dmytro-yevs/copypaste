//! The typed payloads a reply can carry.
//!
//! Split from the envelope in `lib.rs` only for size. Their distinct wire
//! wrappers live on [`crate::ResponseData`].

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
    /// Where to resume, or `None` at the end of the list.
    ///
    /// Pass it back as [`crate::Method::List`]'s `cursor`. **The only correct
    /// end-of-list test**: a short page is not one, because
    /// `skipped_undecryptable` rows were read and dropped, and stopping on a
    /// short page would hide the rest of the history behind a handful of
    /// corrupt rows.
    ///
    /// Always `None` from `Search`, which is not paged — see
    /// [`crate::Method::Search`].
    #[serde(default)]
    pub next_cursor: Option<String>,
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
    #[serde(default)]
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
    /// Items whose `content_type` is not text.
    pub skipped_non_text: u32,
    /// Items the detector flagged, withheld because `include_sensitive` was
    /// false.
    pub skipped_sensitive: u32,
    pub skipped_undecryptable: u32,
}

/// What an import did.
///
/// `skipped` is the compatibility total and is always the sum of the three
/// reason counts in replies from a current daemon. The reason fields are
/// additive and default to zero so a current client can still decode a reply
/// from an earlier v2 daemon. In that case `skipped` can be non-zero while the
/// reason counts are zero, and the client must describe the reason as
/// unavailable rather than assuming those rows were duplicates.
///
/// A malformed batch is refused whole, before anything is written. These
/// counts cover only individually recoverable rows inside an accepted batch.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
pub struct ImportData {
    pub inserted: u32,
    pub skipped: u32,
    /// Items deduplicated against something the store already held.
    #[serde(default)]
    pub skipped_duplicate: u32,
    /// Empty or whitespace-only items, which the ingest path cannot store.
    #[serde(default)]
    pub skipped_empty: u32,
    /// Items larger than the user's configured per-item size limit.
    #[serde(default)]
    pub skipped_too_large: u32,
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
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
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
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
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
    /// Machine-readable counterpart to `error`.
    ///
    /// Additive within protocol v1: older daemons omit it, so a current client
    /// treats absence as an unknown failure without reconstructing a code from
    /// the English sentence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<crate::ErrorCode>,
}

/// What the daemon has refused, missed or deleted since it started.
///
/// Every one of these was already counted somewhere and readable nowhere, which
/// is the whole reason the struct exists: an oversized copy that was dropped and
/// a daemon that has stopped capturing look identical to a user, and the
/// difference was only ever in the daemon's own stderr.
///
/// **Counts, and nothing else.** No ids, no timestamps of individual events, no
/// text. A diagnostics report built from this cannot carry clipboard content
/// because there is none here to carry (CLAUDE.md rule 4).
///
/// Cumulative since the process started and never reset, so `uptime_secs` is
/// the only thing they can honestly be read against.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
pub struct DiagnosticCounters {
    /// Clipboard changes dropped for exceeding the read gate.
    pub rejected_too_large: u64,
    /// Clipboard values that were overwritten before the poll could observe
    /// them. `changeCount` is lossy; a delta above 1 is irrecoverable.
    pub lost_intermediates: u64,
    /// Detected secrets the auto-wipe sweep deleted. The only deletions the
    /// user did not ask for.
    pub sensitive_swept: u64,
    /// Search-index rows the startup purge removed because the current ruleset
    /// calls them sensitive.
    pub index_purged: u64,
    pub uptime_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
pub struct StatusData {
    pub version: String,
    pub protocol_version: u32,
    pub item_count: u64,
    pub capture_running: bool,
    /// Which clipboard backend is live — the real pasteboard or the fake used
    /// on non-macOS hosts and in tests. Surfaced so a demo cannot be mistaken
    /// for the real thing.
    pub clipboard_backend: String,
    /// Persisted capture gate. Present on status so a lightweight poller and
    /// the tray converge without a second settings round-trip.
    #[serde(default)]
    pub private_mode: bool,

    /// A CopyPaste 0.4 history is on this device, unread and unaltered.
    ///
    /// The whole of CLAUDE.md rule 3's second obligation depends on this
    /// crossing the wire. The daemon *works* — v2 starts a new history and the
    /// old one is untouched — so this is not a failure and has no error code;
    /// what it is, is the difference between "your clipboard history vanished"
    /// and "your clipboard history is still there and this version cannot read
    /// it". Without it a client has no way to tell an empty history from an
    /// unreadable one, and the upgrade looks like data loss.
    ///
    /// Decided once, at startup, by a read-only probe: re-probing on every
    /// status poll would open a SQLite connection every few seconds against a
    /// file this build has promised not to touch. It therefore goes stale if
    /// the old history is removed while the daemon runs, which costs a restart
    /// and no data.
    ///
    /// `#[serde(default)]` so a client built before this field decodes a reply
    /// that has it — an unknown field is ignored, a missing one is not.
    #[serde(default)]
    pub legacy_history_present: bool,

    /// What has been refused, missed or deleted since the daemon started.
    ///
    /// On `status` rather than behind a method of its own because every one of
    /// these numbers is only meaningful beside the flags above it: "3 copies
    /// refused" reads differently against a running capture loop than against a
    /// stopped one, and a client that had to make two calls could show them
    /// disagreeing.
    ///
    /// `#[serde(default)]` for the same reason as the field above.
    #[serde(default)]
    pub counters: DiagnosticCounters,
}

/// The private capture gate's authoritative value.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PrivateModeData {
    pub private_mode: bool,
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

    /// Which device first captured this item.
    ///
    /// Never empty: an item with no recorded origin was captured here, and the
    /// daemon substitutes its own device id rather than sending a blank —
    /// "unknown" and "this one" are different answers and a client should not
    /// have to guess which it got.
    ///
    /// An opaque UUID, not a name. It is stable across renames and it is what
    /// the merge tie-break orders on, so it is the right thing to *group* and
    /// *filter* by; [`Item::origin_device_name`] is the right thing to show.
    #[serde(default)]
    pub origin_device_id: String,

    /// What that device calls itself, when this device has ever been told.
    ///
    /// `None` for a device that has never completed a sync session here — the
    /// cloud path carries an origin id but no name, so an item that arrived
    /// through an account from a third device has an id and no name until that
    /// device is paired directly. A client should fall back to a short form of
    /// the id rather than claiming the item is local.
    ///
    /// Peer-supplied and cosmetic. It is not trusted for anything, and two
    /// devices may well report the same name.
    #[serde(default)]
    pub origin_device_name: Option<String>,

    /// This item is larger than cloud sync will carry, so it will never reach
    /// another device through an account.
    ///
    /// A property of the item and the cap, not of any particular round: it is
    /// true before the first sync attempt and stays true, which is the point —
    /// v1 drew a warning on the row (`CopyPaste-f72f`) precisely so that an
    /// item that will silently never arrive does not look like one that is
    /// still on its way.
    ///
    /// Local history is unaffected. The item is stored in full, it is
    /// searchable, it copies, and the peer transport is not governed by this
    /// cap. [`CloudSyncData::skipped_too_large`] is the per-round half.
    #[serde(default)]
    pub too_large_to_sync: bool,
}

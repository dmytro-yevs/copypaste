//! The typed payloads a reply can carry.
//!
//! Split from the envelope in `lib.rs` only for size. Their distinct wire
//! wrappers live on [`crate::ResponseData`].

use serde::{Deserialize, Serialize};

mod cloud;
mod device;

pub use cloud::{CloudStatusData, CloudSyncData};
pub use device::{
    DeviceClass, DeviceDetails, DeviceEndpointObservation, DeviceLatencyObservation,
    DeviceObservationProvenance, DeviceObservationTrust, DevicePlatform, DevicePresenceObservation,
    DeviceProfileObservation, ExternalNetworkObservation,
};

use crate::health::SettingsHealth;

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

/// A bounded PNG thumbnail generated from a stored image on demand.
///
/// The original encrypted clipboard payload never crosses the UI boundary.
/// `png_base64` is only a small, decoded preview suitable for an `<img>`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImagePreview {
    pub png_base64: String,
    pub width: u32,
    pub height: u32,
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
    /// Items that landed but whose pinned state could not be written.
    ///
    /// Reported rather than logged: an import that answered success with a pin
    /// missing lost user state silently (DMY-156). Non-zero means the rows are
    /// there and unpinned; re-running the same import is the repair, because a
    /// duplicate still carries the pin the file named.
    #[serde(default)]
    pub pins_failed: u32,
}

/// A completed backup. No path: the client supplied it and already knows it,
/// and echoing one back is how it ends up in a log (AGENTS.md rule 4).
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
/// resolved locally by comparing advertised pairing ids with this device's
/// own pairing list.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
pub struct DiscoveredDevice {
    /// Opaque mDNS candidate id. This is not a pairing credential or a device
    /// identity; it only de-duplicates announcements in the client.
    pub discovery_id: String,
    pub name: String,
    /// `host:port` to hand to `pair join`.
    pub addr: String,
    pub last_seen_ms: i64,
    pub paired: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(optional))]
    pub details: Option<DeviceDetails>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
#[serde(rename_all = "snake_case")]
pub enum PairingRole {
    Initiator,
    Responder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
#[serde(rename_all = "snake_case")]
pub enum PairingState {
    Idle,
    WaitingForPeer,
    Handshaking,
    AwaitingConfirmation,
    Confirmed,
    Rejected,
    Cancelled,
    TimedOut,
    Failed,
}

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
pub struct PairingInviteData {
    /// Secret QR payload input. Product adapters must render it without
    /// returning the text to the WebView (manifest 06, INV-13).
    pub code: String,
    pub pairing_id: String,
    pub listen_addr: Option<String>,
    pub expires_in_secs: u64,
}

impl std::fmt::Debug for PairingInviteData {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PairingInviteData")
            .field("pairing_id", &self.pairing_id)
            .field("code", &"<redacted>")
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
pub struct PairingProgressData {
    pub pairing_id: Option<String>,
    pub role: Option<PairingRole>,
    pub state: PairingState,
    /// Monotonic time remaining in the current Rust-owned pairing phase.
    /// Native watchdogs may request a fresh progress read when this reaches
    /// zero, but must not derive a terminal state themselves.
    pub expires_in_ms: Option<u64>,
    /// Present only after the authenticated handshake and before a terminal
    /// state. Six ASCII digits, display-only.
    pub sas: Option<String>,
    pub peer_device_id: Option<String>,
    pub peer_name: Option<String>,
    pub peer_addr: Option<String>,
    /// Present only after bilateral confirmation and durable persistence.
    pub known_device: Option<PeerInfo>,
    pub error_code: Option<crate::ErrorCode>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(optional))]
    pub details: Option<DeviceDetails>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub pairing_id: String,
    pub name: String,
    pub sent: u32,
    pub received: u32,
    /// End-to-end duration of this sync session. This is not an ICMP ping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
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
/// because there is none here to carry (AGENTS.md rule 4).
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
    #[serde(default)]
    pub device_name: String,
    pub version: String,
    pub protocol_version: u32,
    /// Address of the peer listener that is actually running, when routable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listen_addr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(optional))]
    pub device_details: Option<DeviceDetails>,
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
    /// CopyPaste-48k0: lets pollers detect intervening private-mode writes.
    pub private_mode_epoch: u64,

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

    /// Set when the persisted settings did not read back cleanly.
    ///
    /// `None` is the healthy case and the only one a client may render
    /// silently. Anything else means the daemon is running on values the user
    /// did not choose, and the failure it exists for is a private-mode user
    /// never told that capture stopped following their setting.
    #[serde(default)]
    pub settings_health: Option<SettingsHealth>,
}

/// The private capture gate's authoritative value.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
pub struct PrivateModeData {
    pub private_mode: bool,
    pub private_mode_epoch: u64,
}

/// One validated match in text that remains eligible for history and sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
pub struct SensitiveSpan {
    /// UTF-8 byte offset into the detector's NFKC-normalised input.
    pub start: u32,
    /// Exclusive UTF-8 byte offset into the detector's NFKC-normalised input.
    pub end: u32,
}

/// A detected but non-destructive match that a client may surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
pub struct SensitiveFinding {
    /// Stable detector rule id, such as `email` or `iban`.
    pub label: String,
    /// Validated matches in normalised UTF-8 byte order.
    pub spans: Vec<SensitiveSpan>,
    /// True when more matches existed than the wire contract permits.
    pub spans_truncated: bool,
    /// A bounded NFKC-normalised preview with every validated match replaced.
    pub redacted_preview: String,
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
    /// True when the high-confidence whole-item gate matched or capture came
    /// from a password manager. Such items are excluded from the search index
    /// at write time, at read time, and by a purge pass.
    pub is_sensitive: bool,

    /// Low-confidence detector metadata. Absent for unmatched, binary, and
    /// whole-item sensitive rows; the latter remain hidden rather than partly
    /// represented by a redacted preview.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensitive_finding: Option<SensitiveFinding>,

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

    /// Application bundle that owned the foreground clipboard when this item
    /// was captured. This is local, cosmetic attribution, so synced items may
    /// not have one and clients must use a neutral fallback in that case.
    #[serde(default)]
    pub source_app_bundle_id: Option<String>,

    /// Platform-resolved display name for [`Self::source_app_bundle_id`].
    /// Cosmetic only: application identity and exclusion rules use the stable
    /// bundle/package id, never this mutable label.
    #[serde(default)]
    pub source_app_name: Option<String>,

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

    #[serde(default)]
    pub truncated: bool,
}

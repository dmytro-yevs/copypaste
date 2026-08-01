//! The single model of the daemon wire contract.
//!
//! v1 modelled this three times — typed DTOs in a shared crate that the CLI
//! never imported, a near-duplicate inside the daemon, and untyped
//! `serde_json::Value` poking in the CLI across 128 `.as_*()` calls that
//! silently defaulted on a missing field. Both the daemon and the CLI depend on
//! this crate and on nothing else for wire types, so a change here breaks
//! compilation on both sides rather than drifting.
//!
//! Framing is newline-delimited JSON over a Unix socket. That much v1 got
//! right; what it got wrong was hand-rolling the frame codec, so the daemon
//! uses `tokio_util::codec::LinesCodec` instead of a byte-scanning read loop.

#![forbid(unsafe_code)]

pub mod config;
pub mod content_type;
pub mod error;
pub mod limits;
pub mod paths;
pub mod payload;
pub mod redact;

pub use paths::{data_dir, database_path, socket_path, v1_data_dir};

pub use config::{
    ConfigData, ConfigError, ConfigPatch, Liveness, DEFAULT_STORAGE_QUOTA_BYTES,
    MIN_STORAGE_QUOTA_BYTES,
};
pub use error::ErrorCode;
pub use limits::{
    clamp_page, DEFAULT_LIST_PAGE, DEFAULT_SEARCH_PAGE, MAX_CONTENT_BYTES, MAX_FRAME_BYTES,
    MAX_PAGE, MAX_PAGE_CONTENT_BYTES,
};
pub use payload::{
    BackupData, CloudStatusData, CloudSyncData, DiagnosticCounters, DiscoveredData,
    DiscoveredDevice, ExportData, ExportItem, ImportData, Item, ItemPage, PairingData, PeerInfo,
    PrivateModeData, StatusData, SyncResult,
};

use serde::{Deserialize, Serialize};

/// Bumped on any breaking change to the request or response shape.
pub const PROTOCOL_VERSION: u32 = 1;

/// One request. `id` is echoed back so a client can match replies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    #[serde(default = "default_protocol_version")]
    pub protocol_version: u32,
    #[serde(flatten)]
    pub method: Method,
}

fn default_protocol_version() -> u32 {
    PROTOCOL_VERSION
}

/// Every operation the daemon supports.
///
/// An enum rather than a method-name string plus untyped params: v1 dispatched
/// 61 stringly-typed methods through a chain of `match` arms spread over 21
/// files, and extracted params by hand. Here the compiler enumerates the cases.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "snake_case")]
pub enum Method {
    /// Liveness plus daemon state.
    Status,
    /// Most recent items, newest first. Pinned items sort ahead of unpinned.
    ///
    /// Paged by cursor, never by offset. A clipboard manager inserts above the
    /// window every time the user copies anything, so an offset taken for page
    /// 1 no longer names the same boundary by the time page 2 is asked for: the
    /// second page repeats a row or skips one, and a row the user never saw is
    /// indistinguishable from one that was never captured
    /// (`CopyPaste-8ebg.57`, CLAUDE.md rule 4).
    ///
    /// `cursor` is [`ItemPage::next_cursor`] from the previous page, and
    /// `None`/absent asks for the first. It is opaque — a position in the list
    /// order, not an id — and a client must pass it back unread. One this
    /// daemon did not write is refused with [`ErrorCode::InvalidRequest`]
    /// rather than being treated as "start from the top", which would make a
    /// load-more silently repeat the whole history.
    List {
        limit: u32,
        #[serde(default)]
        cursor: Option<String>,
    },
    /// Full-text search. Sensitive items are never indexed and never returned.
    ///
    /// **Not paged, and deliberately so.** It runs against the whole database
    /// and returns the best `limit` matches, so a hit at row 800 is found
    /// without reading 800 rows first (AT-73 / `CopyPaste-crh3.106`). Cursors
    /// need a total order to seek on and FTS5 `rank` is not one — it is a
    /// score that every write can change — so a cursor over it would promise a
    /// stability nothing upholds. [`ItemPage::next_cursor`] is therefore always
    /// `None` here.
    Search {
        query: String,
        limit: u32,
    },
    /// Put an item's content back on the system clipboard.
    Copy {
        id: String,
    },
    /// Put an item's textual representation on the system clipboard.
    ///
    /// This is intentionally distinct from [`Method::Copy`]. `Copy` preserves
    /// the item's native representation as capture grows beyond text, whereas
    /// this verb is the explicit Quick Paste ⌥Enter request for plain text.
    CopyPlainText {
        id: String,
    },
    /// One item by id, with its content, and **no side effects**.
    ///
    /// The read-only twin of [`Method::Copy`]. It exists because a UI that
    /// wants to *show* an item — a reveal gesture on a sensitive one, a detail
    /// pane — otherwise has to call `Copy`, which publishes the content to the
    /// system pasteboard, where every other application can read it. Looking at
    /// something must not be indistinguishable from copying it.
    ///
    /// **It does return the plaintext of a sensitive item, deliberately.** That
    /// is not a hole in the sensitive-content rules: those are about the item
    /// never reaching the *search index* and never leaving the *device*, and
    /// this crosses neither boundary — the socket is `0600`, `List` already
    /// returns the same plaintext, and the alternative is a client that cannot
    /// implement reveal at all. Deciding whether to render it is the client's,
    /// and a client should require an explicit gesture.
    Get {
        id: String,
    },
    /// Add an item directly, bypassing clipboard capture. Used by tests, by
    /// `copypaste add`, and by the fake clipboard source.
    Add {
        content: String,
    },
    Delete {
        id: String,
    },
    DeleteAll,
    Pin {
        id: String,
        pinned: bool,
    },
    /// Rewrite the order of the pinned section.
    ///
    /// `ids` is the **complete** pinned ordering, first to last — not a
    /// move-one-item instruction. A partial move ("put b after d") is ambiguous
    /// the moment a peer pins something between the two, because the two
    /// devices no longer agree on what is in between; a full ordering is the
    /// only shape that survives a concurrent pin, and it is what the drag
    /// gesture produces anyway.
    ///
    /// Ids that are not pinned, or no longer exist, are ignored rather than
    /// refused: the list a client is holding was read a moment ago, and a peer
    /// may have deleted one since. Pinned items the list does not name keep
    /// their relative order and sort after the ones it does.
    ///
    /// The order is local. Nothing about a pin travels on either transport
    /// (`Store::upsert` preserves the local `pinned` and `pin_order` across an
    /// incoming version), so reordering here does not reorder anywhere else.
    ReorderPinned {
        ids: Vec<String>,
    },

    // ---- peer-to-peer sync -------------------------------------------------
    /// Mint a pairing token on this device and return the code to read out.
    /// The token is the Noise pre-shared key; the code is its transferable form.
    PairCreate {
        name: String,
    },
    /// Consume a code produced by `PairCreate` on another device, connect to
    /// `addr`, and complete the pairing.
    PairAccept {
        code: String,
        addr: String,
    },
    /// Forget a peer. Its half of the pairing keeps working until it also
    /// unpairs — this is a local decision, not a negotiated one.
    Unpair {
        pairing_id: String,
    },
    /// Cut a device off for good: drop its key **and** bar the pairing id.
    ///
    /// Distinct from [`Method::Unpair`] in exactly one respect, and it is the
    /// one that matters after a device is lost. The code that made a pairing is
    /// the long-term Noise pre-shared key, so it keeps working: after `Unpair`,
    /// re-entering a code someone kept restores the pairing. `Revoke` records
    /// the pairing id as refused — permanently, across restarts and across a
    /// stale copy of the peer file — so nothing can enrol it again
    /// (`CopyPaste-gbo`, `PeerStore::revoke`).
    ///
    /// Irreversible from this side and not negotiated: the other device is told
    /// nothing and keeps its own half. Pairing the two devices again means a
    /// new code, by hand, on both.
    ///
    /// Answers `Empty` whether or not a peer was there to remove. Revoking an
    /// id this device has not seen is meaningful — it refuses one that has not
    /// dialled in yet — so a `not_found` would deny work that was done.
    Revoke {
        pairing_id: String,
    },
    /// Known peers and when each was last reachable.
    Peers,
    /// Sync now with one peer, or with every known peer when `pairing_id` is
    /// absent. Returns per-peer counts.
    SyncNow {
        pairing_id: Option<String>,
    },

    /// LAN devices discovery has heard from, paired or not.
    Discovered,
    /// Re-advertise and re-browse, then answer as [`Method::Discovered`] does.
    Rescan,

    // ---- transfer and database administration -------------------------------
    /// Read history out of the daemon. `limit` of 0 means everything.
    ///
    /// `include_sensitive` defaults to false on the wire *and* in every client:
    /// an export is a plaintext file that leaves the app's control, so a
    /// detected credential is only ever in one because the user asked twice.
    Export {
        limit: u32,
        include_sensitive: bool,
    },
    /// Put items back. Each one goes through the same ingest path a capture
    /// does, so the detector runs again and dedup still applies.
    Import {
        items: Vec<ExportItem>,
    },
    /// Copy the encrypted database to `dest_path`, which must not exist.
    Backup {
        dest_path: String,
    },
    /// Replace history with the contents of the backup at `src_path`.
    ///
    /// `confirm` must be true. The backup is opened and checked with this
    /// device's real key before anything is replaced — see the daemon's
    /// `server::dbadmin` for what "validate then swap" means here.
    Restore {
        src_path: String,
        confirm: bool,
    },

    // ---- cloud sync --------------------------------------------------------
    /// Sign into the sync account and unlock the sync key.
    ///
    /// Three secrets in one call because they are one gesture: the account
    /// credentials authenticate to the backend, and the passphrase derives the
    /// key the backend must never be able to derive. All three cross a `0600`
    /// socket and none is logged or echoed back.
    CloudSignIn {
        email: String,
        password: String,
        passphrase: String,
    },
    /// Forget the account, the tokens and the sync key on this device.
    ///
    /// Persistent, and it keeps the deployment's URL and anon key — those are
    /// configuration, not credentials (manifest 04, `CopyPaste-crh3.100`).
    CloudSignOut,
    /// Whether cloud sync is configured, signed in, and when it last ran.
    CloudStatus,
    /// Run one push-then-pull round now instead of waiting for the poll.
    CloudSyncNow,

    // ---- settings ----------------------------------------------------------
    /// The daemon's effective settings.
    GetConfig,
    /// Change some settings. Rejected whole if any value is out of range, and a
    /// rejection leaves the daemon on the configuration it already had.
    SetConfig {
        patch: ConfigPatch,
    },
    /// Set the persisted capture gate without requiring a client to merge the
    /// whole settings object.
    SetPrivateMode {
        enabled: bool,
    },
    /// Read the persisted capture gate.
    GetPrivateMode,

    /// Turn this connection into a change stream.
    ///
    /// The daemon answers once with [`ResponseData::Empty`] to acknowledge, then
    /// writes an [`ResponseData::Event`] frame — same `id`, same envelope, no
    /// second framing to implement — every time history or the peer list
    /// changes, until the client hangs up.
    ///
    /// Events carry no clipboard content, only what changed and how many items
    /// there now are: a subscriber re-reads through the ordinary methods, which
    /// keeps one set of rules about what a client may see.
    ///
    /// A subscriber is exempt from the daemon's idle read deadline — being
    /// silent is the point — and is counted against a separate, smaller cap so
    /// that watchers cannot consume every connection slot.
    Watch,

    /// Stop the daemon.
    ///
    /// Acknowledged first and acted on second, so a client learns the request
    /// was accepted rather than inferring it from a closed socket.
    ///
    /// **It confers no authority.** The socket is `0600`, so anyone who can
    /// send this could already read every item, add one, or delete the lot —
    /// and could signal the process besides. What it adds is doing it
    /// *portably and politely*: the daemon unwinds through the same path
    /// SIGTERM takes, finishing the capture it is in and removing its socket,
    /// rather than being aborted and leaving a stale file behind.
    ///
    /// It exists because an app cannot stop a daemon it did not start. Without
    /// it, ADR-0004's protocol-mismatch state can explain that the service is
    /// the wrong version and then offer the user nothing: the app can start a
    /// daemon, and it can restart one it launched itself, but the one already
    /// running is somebody else's process.
    ///
    /// Answerable before readiness, deliberately — a daemon whose database will
    /// not open is exactly the one a user needs to be able to stop.
    Shutdown,
}

/// The settings as they now stand, plus what will not take effect yet.
///
/// `restart_required` is empty for every live field and is what lets a Settings
/// screen say so at the moment of the change rather than leaving the user to
/// discover it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigApplied {
    pub config: ConfigData,
    pub restart_required: Vec<String>,
}

/// What changed. Coalesced: a burst of captures may produce one event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// History changed — an item was added, deleted, pinned, imported or
    /// arrived from a peer or the cloud.
    Items,
    /// The paired-device list changed.
    Peers,
}

/// One push frame on a [`Method::Watch`] connection.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EventData {
    pub event: EventKind,
    /// Live item count at the time of the event, so a client can render a badge
    /// without a round trip.
    pub item_count: u64,
    /// The change was a *capture* — something the user copied, as opposed to a
    /// delete, a pin, an import, or a row arriving from a peer.
    ///
    /// This is what a client needs to post the notification and play the sound
    /// that `ConfigData::notify_on_copy` and `ConfigData::sound_on_copy` gate.
    /// A flag rather than a new [`EventKind`] variant so that a client built
    /// against an older build still decodes the frame: an unknown enum variant
    /// fails deserialisation, and a watcher that stops decoding stops updating.
    ///
    /// It carries no content and no id, for the reason [`Method::Watch`] gives:
    /// a subscriber that wants the item re-reads it through the ordinary
    /// methods, which keeps one set of rules about what a client may see.
    ///
    /// The daemon does **not** consult `notify_on_copy` before setting it. The
    /// flag says what happened; the setting says what to do about it, and the
    /// client that owns the notification is the one that should read it (the
    /// daemon has no bundle and therefore cannot post one — see
    /// `daemon/src/notify.rs`).
    #[serde(default)]
    pub captured: bool,

    /// Detected secrets the auto-wipe sweep deleted, in the change this event
    /// reports. Zero on every other change.
    ///
    /// A deletion the user did not ask for is the one history change they have
    /// to be told about, and until this field there was no way to tell them:
    /// `ConfigData::sensitive_ttl_secs` defaults to `0` — the feature off —
    /// *because* the count could not leave the daemon, and the Settings screen
    /// says so in as many words.
    ///
    /// A count and not a list of ids, for the reason [`Method::Watch`] gives:
    /// the rows are gone, and an event carries no content.
    ///
    /// Same `#[serde(default)]` reasoning as [`EventData::captured`] — a
    /// watcher built against an older daemon must keep decoding.
    #[serde(default)]
    pub swept: u32,
}

/// One reply. `ok` distinguishes success from failure without inspecting the
/// payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub id: u64,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<ResponseData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<ErrorCode>,
}

impl Response {
    pub fn ok(id: u64, data: ResponseData) -> Self {
        Self {
            id,
            ok: true,
            data: Some(data),
            error: None,
            error_code: None,
        }
    }

    /// Build a failure reply.
    ///
    /// `message` must never contain a filesystem path: the daemon socket path
    /// discloses the local username (CLAUDE.md rule 4). Callers map internal
    /// errors to a plain sentence before they get here.
    pub fn err(id: u64, code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            id,
            ok: false,
            data: None,
            error: Some(message.into()),
            error_code: Some(code),
        }
    }
}

/// The payload of a successful reply.
///
/// Untagged, so the decoder tries the variants **in declaration order** and
/// takes the first that fits. Two rules follow from that and neither is
/// cosmetic:
///
/// * A variant whose required fields are a subset of another's must come
///   *after* it, or it will swallow the richer payload. [`ResponseData::Export`]
///   is declared before [`ResponseData::Page`] for exactly this reason.
/// * `Empty {}` matches any JSON object at all, so it stays last.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponseData {
    Status(StatusData),
    Export(ExportData),
    Import(ImportData),
    Backup(BackupData),
    Discovered(DiscoveredData),
    Config(ConfigApplied),
    Event(EventData),
    Page(ItemPage),
    Item(Item),
    Count(u64),
    Pairing(PairingData),
    Peers(Vec<PeerInfo>),
    Sync(Vec<SyncResult>),
    CloudStatus(CloudStatusData),
    CloudSync(CloudSyncData),
    PrivateMode(PrivateModeData),
    /// Must stay last: an empty struct variant matches any JSON object, so an
    /// arm below it would never be reached by the untagged decoder.
    Empty {},
}

#[cfg(test)]
mod tests {
    use super::*;

    /// v0.4's directory is a *different question* from v2's, which is the whole
    /// reason this function exists rather than a reuse of [`data_dir`].
    #[test]
    fn the_v0_4_directory_is_resolved_where_v0_4_put_it() {
        let v1 = v1_data_dir().expect("a home directory in the test environment");
        assert!(v1.is_absolute(), "{}", v1.display());

        #[cfg(target_os = "macos")]
        {
            assert!(
                v1.ends_with("Library/Application Support/CopyPaste"),
                "{v1:?}"
            );
            // If these ever coincided, CLAUDE.md rule 3's "never touch the old
            // file" would rest on the filename alone.
            assert_ne!(v1, data_dir());
        }
        #[cfg(not(target_os = "macos"))]
        {
            // v0.4.x and `ProjectDirs` agree here; the *filename* is what keeps
            // the two histories apart, and `database_path` is why.
            assert!(v1.ends_with("copypaste"), "{v1:?}");
            assert_eq!(v1, data_dir());
        }
    }

    /// A daemon built before the field omits it, and a client built after must
    /// still decode that reply rather than treating a status as malformed.
    #[test]
    fn a_status_without_the_legacy_flag_decodes_as_absent() {
        let older = r#"{"version":"2.0.0-alpha.1","protocol_version":1,"item_count":3,
                        "capture_running":true,"clipboard_backend":"fake"}"#;
        let status: StatusData = serde_json::from_str(older).unwrap();
        assert!(!status.legacy_history_present);
        assert_eq!(status.counters, DiagnosticCounters::default());
    }
}

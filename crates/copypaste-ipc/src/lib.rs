//! The single model of the daemon wire contract.
//!
//! v1 modelled this three times — typed DTOs in a shared crate that the CLI
//! never imported, a near-duplicate inside the daemon, and untyped
//! `serde_json::Value` poking in the CLI across 128 `.as_*()` calls that
//! silently defaulted on a missing field. Both the daemon and the CLI depend on
//! this crate and on nothing else for wire types, so a change here breaks
//! compilation on both sides rather than drifting.
//!
//! Framing is newline-delimited JSON over the local endpoint [`transport`]
//! names. That much v1 got right; what it got wrong was hand-rolling the frame
//! codec, so the daemon uses `tokio_util::codec::LinesCodec` instead of a
//! byte-scanning read loop.

#![forbid(unsafe_code)]

pub mod config;
pub mod content_type;
pub mod error;
pub mod health;
pub mod limits;
pub mod paths;
pub mod payload;
pub mod redact;
mod response;
pub mod transport;

pub use paths::{data_dir, database_path, socket_path};

pub use config::{
    ConfigData, ConfigError, ConfigPatch, Liveness, DEFAULT_STORAGE_QUOTA_BYTES,
    MAX_DECODED_IMAGE_MB, MAX_FILE_SIZE_BYTES, MAX_IMAGE_SIZE_BYTES, MAX_TEXT_SIZE_BYTES,
    MIN_DECODED_IMAGE_MB, MIN_FILE_SIZE_BYTES, MIN_IMAGE_SIZE_BYTES, MIN_STORAGE_QUOTA_BYTES,
    MIN_TEXT_SIZE_BYTES, POLL_INTERVAL_MAX_MS, POLL_INTERVAL_MIN_MS,
};
pub use error::ErrorCode;
pub use health::SettingsHealth;
pub use limits::{
    clamp_page, DEFAULT_LIST_PAGE, DEFAULT_SEARCH_PAGE, MAX_CONTENT_BYTES, MAX_FRAME_BYTES,
    MAX_PAGE, MAX_PAGE_CONTENT_BYTES,
};
pub use payload::{
    BackupData, CloudStatusData, CloudSyncData, DiagnosticCounters, DiscoveredData,
    DiscoveredDevice, ExportData, ExportItem, ImagePreview, ImportData, Item, ItemPage,
    PairingData, PairingInviteData, PairingProgressData, PairingRole, PairingState, PeerInfo,
    PrivateModeData, SensitiveFinding, SensitiveSpan, StatusData, SyncResult,
};
pub use response::{ConfigApplied, EventData, EventKind, Response, ResponseData};

use serde::{Deserialize, Serialize};

/// Bumped on any breaking change to the request or response shape.
pub const PROTOCOL_VERSION: u32 = 2;

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
    /// Persist this device's peer-visible display name.
    SetDeviceName {
        name: String,
    },
    /// Most recent items, newest first. Pinned items sort ahead of unpinned.
    ///
    /// Paged by cursor, never by offset. A clipboard manager inserts above the
    /// window every time the user copies anything, so an offset taken for page
    /// 1 no longer names the same boundary by the time page 2 is asked for: the
    /// second page repeats a row or skips one, and a row the user never saw is
    /// indistinguishable from one that was never captured
    /// (`CopyPaste-8ebg.57`, AGENTS.md rule 4).
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
    /// A decoded, bounded PNG thumbnail for a non-sensitive image item.
    ///
    /// Kept separate from `List` so a long history never decrypts or transfers
    /// image bytes until a row actually becomes visible.
    ImagePreview {
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
    /// Legacy invite shape. Creates the same memory-only invitation as
    /// [`Method::PairCreateInvite`].
    PairCreate {
        name: String,
    },
    /// Disabled legacy operation. New clients use [`Method::PairJoin`] and
    /// [`Method::PairConfirm`].
    PairAccept {
        code: String,
        addr: String,
    },
    /// Create one in-memory invitation. Nothing is persisted until both peers
    /// explicitly confirm the handshake-bound SAS.
    PairCreateInvite,
    /// Start the initiating half of a pairing ceremony.
    PairJoin {
        code: String,
        addr: String,
    },
    /// Current ceremony state, including SAS or confirmed known-device data.
    PairProgress,
    /// Record the local SAS decision. Persistence still waits for the peer's
    /// matching decision.
    PairConfirm {
        accept: bool,
    },
    /// Idempotently cancel the current ceremony or invitation.
    PairCancel,
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
        #[serde(default)]
        limit: u32,
        #[serde(default)]
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

impl Method {
    /// Whether this verb needs the long client budget rather than the ordinary
    /// one (manifest 04 §3.4, `CopyPaste-8ebg.4`).
    ///
    /// Here rather than in each client because v1 kept three models of the wire
    /// contract and they disagreed (AGENTS.md rule 1). A client that classified
    /// this on its own would eventually cancel an operation the other client
    /// waits for, and the difference would show up as an intermittent failure
    /// on one surface only.
    ///
    /// Three reasons, all of them "the work is genuinely longer than a request":
    /// local I/O across a whole history, a round trip over the network, and a
    /// step that waits on a human comparing a SAS. The daemon writes nothing
    /// until its handler returns, so a budget shorter than the work does not
    /// protect the client — it cancels an operation that was going to succeed.
    #[must_use]
    pub fn is_long_running(&self) -> bool {
        matches!(
            self,
            Self::Import { .. }
                | Self::Backup { .. }
                | Self::Restore { .. }
                | Self::SyncNow { .. }
                | Self::CloudSyncNow
                | Self::CloudSignIn { .. }
                | Self::PairJoin { .. }
                | Self::PairConfirm { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Optional status fields preserve protocol-v1 decoding across releases.
    #[test]
    fn a_status_without_optional_fields_decodes_with_defaults() {
        let older = r#"{"version":"2.0.0-alpha.1","protocol_version":1,"item_count":3,
                        "capture_running":true,"clipboard_backend":"fake",
                        "private_mode_epoch":0}"#;
        let status: StatusData = serde_json::from_str(older).unwrap();
        assert!(status.device_name.is_empty());
        assert!(!status.private_mode);
        assert_eq!(status.private_mode_epoch, 0);
        assert_eq!(status.counters, DiagnosticCounters::default());
        assert!(status.settings_health.is_none());
    }

    /// The classification both clients share. `Status` is the one that matters
    /// most for the ordinary budget: it is what quit and restart poll, so a
    /// long budget there is a hung app.
    #[test]
    fn only_the_verbs_that_outlast_a_request_get_the_long_budget() {
        for long in [
            Method::Import { items: Vec::new() },
            Method::Backup {
                dest_path: "d".into(),
            },
            Method::Restore {
                src_path: "s".into(),
                confirm: true,
            },
            Method::SyncNow { pairing_id: None },
            Method::CloudSyncNow,
            Method::PairJoin {
                code: "c".into(),
                addr: "a".into(),
            },
            Method::PairConfirm { accept: true },
        ] {
            assert!(long.is_long_running(), "{long:?}");
        }
        for ordinary in [
            Method::Status,
            Method::Shutdown,
            Method::List {
                limit: 1,
                cursor: None,
            },
            Method::GetConfig,
            Method::GetPrivateMode,
            Method::Peers,
            Method::Discovered,
            // `rescan` republishes an mDNS record and returns the cache it
            // already holds; it blocks on nothing. The long budget here left
            // the Refresh button — `disabled={rescan.isPending}` — dead for
            // three minutes whenever a reply went missing.
            Method::Rescan,
            Method::CloudStatus,
            Method::Watch,
        ] {
            assert!(!ordinary.is_long_running(), "{ordinary:?}");
        }
    }

    /// Absent means healthy, and it has to stay distinguishable from a health
    /// record that reports nothing wrong — a client renders one silently and
    /// must never render the other that way.
    #[test]
    fn an_absent_settings_health_is_not_the_same_as_an_empty_one() {
        assert!(!SettingsHealth::default().is_degraded());
        assert!(SettingsHealth {
            record_unreadable: true,
            unreadable_fields: Vec::new(),
        }
        .is_degraded());
        assert!(SettingsHealth {
            record_unreadable: false,
            unreadable_fields: vec!["private_mode".into()],
        }
        .is_degraded());
    }

    #[test]
    fn private_mode_epoch_is_required_for_convergence() {
        assert!(serde_json::from_str::<PrivateModeData>(r#"{"private_mode":true}"#).is_err());
    }

    /// The reason fields were added within protocol v1. A client that speaks
    /// the same protocol must continue to accept a reply from a daemon built
    /// before they existed, while retaining the old aggregate for an honest
    /// "reason unavailable" report.
    #[test]
    fn an_import_reply_without_reason_fields_keeps_its_total() {
        let older = r#"{"id":1,"ok":true,"data":{"import":{"inserted":7,"skipped":2}}}"#;
        let response: Response = serde_json::from_str(older).unwrap();
        let Some(ResponseData::Import(imported)) = response.data else {
            panic!("the older import reply decoded as the wrong payload")
        };

        assert_eq!(imported.inserted, 7);
        assert_eq!(imported.skipped, 2);
        assert_eq!(imported.skipped_duplicate, 0);
        assert_eq!(imported.skipped_empty, 0);
        assert_eq!(imported.skipped_too_large, 0);
    }

    #[test]
    fn an_import_reply_serialises_every_skip_reason() {
        let response = Response::ok(
            1,
            ResponseData::Import(ImportData {
                inserted: 4,
                skipped: 6,
                skipped_duplicate: 1,
                skipped_empty: 2,
                skipped_too_large: 3,
                pins_failed: 1,
            }),
        );
        let json = serde_json::to_value(response).unwrap();

        assert_eq!(json["data"]["import"]["inserted"], 4);
        assert_eq!(json["data"]["import"]["skipped"], 6);
        assert_eq!(json["data"]["import"]["skipped_duplicate"], 1);
        assert_eq!(json["data"]["import"]["skipped_empty"], 2);
        assert_eq!(json["data"]["import"]["skipped_too_large"], 3);
        assert_eq!(json["data"]["import"]["pins_failed"], 1);
    }
}

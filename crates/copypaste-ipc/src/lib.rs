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
pub mod payload;
pub mod redact;

pub use config::{ConfigData, ConfigError, ConfigPatch, Liveness};
pub use payload::{
    BackupData, CloudStatusData, CloudSyncData, DiscoveredData, DiscoveredDevice, ExportData,
    ExportItem, ImportData, Item, ItemPage, PairingData, PeerInfo, StatusData, SyncResult,
};

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Bumped on any breaking change to the request or response shape.
pub const PROTOCOL_VERSION: u32 = 1;

/// Frames larger than this are rejected before allocation.
pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

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
    List {
        limit: u32,
        offset: u32,
    },
    /// Full-text search. Sensitive items are never indexed and never returned.
    Search {
        query: String,
        limit: u32,
    },
    /// Put an item's content back on the system clipboard.
    Copy {
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
    /// Must stay last: an empty struct variant matches any JSON object, so an
    /// arm below it would never be reached by the untagged decoder.
    Empty {},
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    NotFound,
    InvalidRequest,
    ProtocolMismatch,
    NotReady,
    /// Credentials were rejected, or the operation needs credentials the daemon
    /// does not hold. Its own code because the recovery is a human action —
    /// sign in, retype the passphrase — and never a retry (manifest 04's
    /// `auth_failed`).
    AuthFailed,
    Internal,
}

/// Where the daemon socket lives.
///
/// One definition, used by the daemon and the CLI. v1 duplicated this logic in
/// three places and the module doc admitted it.
pub fn socket_path() -> PathBuf {
    data_dir().join("daemon.sock")
}

/// v2 database filename. Deliberately distinct from v1's, so an existing v0.4.x
/// database is never opened, modified, or reported as corrupt — see CLAUDE.md
/// rule 3.
pub fn database_path() -> PathBuf {
    data_dir().join("copypaste-v2.db")
}

pub fn data_dir() -> PathBuf {
    directories::ProjectDirs::from("com", "copypaste", "CopyPaste")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".copypaste"))
}

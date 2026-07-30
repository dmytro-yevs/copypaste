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

pub mod redact;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponseData {
    Status(StatusData),
    Items(Vec<Item>),
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

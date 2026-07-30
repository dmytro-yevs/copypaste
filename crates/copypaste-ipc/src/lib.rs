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
    List { limit: u32, offset: u32 },
    /// Full-text search. Sensitive items are never indexed and never returned.
    Search { query: String, limit: u32 },
    /// Put an item's content back on the system clipboard.
    Copy { id: String },
    /// Add an item directly, bypassing clipboard capture. Used by tests, by
    /// `copypaste add`, and by the fake clipboard source.
    Add { content: String },
    Delete { id: String },
    DeleteAll,
    Pin { id: String, pinned: bool },
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
        Self { id, ok: true, data: Some(data), error: None, error_code: None }
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
    Empty {},
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

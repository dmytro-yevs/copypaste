// Permit dead code — many IPC methods are stubs for future UI features.
#![allow(dead_code)]

use anyhow::{anyhow, Context, Result};
use copypaste_ipc::ErrorCode;
use serde::{Deserialize, Serialize};
use serde_json::Value;
/// Synchronous IPC client for the copypaste-daemon Unix socket.
///
/// Protocol: newline-delimited JSON.
///   Request:  {"id":"<req_id>","method":"<method>","params":{...}}
///   Response: {"id":"<req_id>","ok":<bool>,"data":<value>,"error":"<msg>"}
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;

// ---------------------------------------------------------------------------
// IpcError
// ---------------------------------------------------------------------------

/// Structured connection errors. Exposed so callers can detect a missing
/// daemon (vs. a protocol/IO failure) and show a friendly empty-state.
///
/// Beta wave W3.3 extends this enum with typed variants for daemon-side
/// failures carrying a [`copypaste_ipc::ErrorCode`]. Existing variants are
/// untouched (append-only) so prior call sites and tests keep working.
#[derive(Debug)]
pub enum IpcError {
    /// The daemon socket file does not exist or refused the connection —
    /// the daemon is most likely not running. Carries the socket path that
    /// was attempted so the UI can include it in the empty-state text.
    DaemonOffline(std::path::PathBuf),
    /// Any other IO error while connecting.
    Io(std::io::Error),
    /// Daemon responded with `error_code = "not_implemented"`.
    /// Carries the human-readable message for display.
    NotImplemented(String),
    /// Daemon responded with `error_code = "auth_failed"`.
    AuthFailed(String),
    /// Daemon responded with `error_code = "not_found"`.
    NotFound(String),
    /// Daemon responded with `error_code = "invalid_argument"`.
    InvalidArgument(String),
    /// Daemon responded with `error_code = "ipc_not_ready"`.
    IpcNotReady(String),
    /// Daemon responded with `error_code = "rate_limited"`.
    RateLimited(String),
    /// Daemon responded with `error_code = "version_mismatch"`.
    VersionMismatch(String),
    /// Daemon responded with `error_code = "internal_error"` or with no
    /// recognised code at all. Carries the message and the parsed code
    /// when one was supplied.
    Daemon(String, Option<ErrorCode>),
}

impl std::fmt::Display for IpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IpcError::DaemonOffline(path) => write!(
                f,
                "Daemon not running. Start with `copypaste daemon start` (socket: {}).",
                path.display()
            ),
            IpcError::Io(e) => write!(f, "IPC error: {e}"),
            IpcError::NotImplemented(m) => write!(f, "not implemented: {m}"),
            IpcError::AuthFailed(m) => write!(f, "authentication failed: {m}"),
            IpcError::NotFound(m) => write!(f, "not found: {m}"),
            IpcError::InvalidArgument(m) => write!(f, "invalid argument: {m}"),
            IpcError::IpcNotReady(m) => write!(f, "daemon not ready: {m}"),
            IpcError::RateLimited(m) => write!(f, "rate limited: {m}"),
            IpcError::VersionMismatch(m) => write!(f, "protocol version mismatch: {m}"),
            IpcError::Daemon(m, Some(code)) => write!(f, "daemon error [{code}]: {m}"),
            IpcError::Daemon(m, None) => write!(f, "daemon error: {m}"),
        }
    }
}

impl std::error::Error for IpcError {}

impl IpcError {
    /// Map a parsed `error_code` + message pair onto the typed variant.
    /// Unknown / missing codes collapse to [`IpcError::Daemon`].
    ///
    /// Centralised so the same mapping is used everywhere a daemon failure
    /// surfaces, and so adding a new code only requires touching one site.
    pub fn from_code(code: Option<ErrorCode>, message: String) -> Self {
        match code {
            Some(ErrorCode::NotImplemented) => IpcError::NotImplemented(message),
            Some(ErrorCode::AuthFailed) => IpcError::AuthFailed(message),
            Some(ErrorCode::NotFound) => IpcError::NotFound(message),
            Some(ErrorCode::InvalidArgument) => IpcError::InvalidArgument(message),
            Some(ErrorCode::IpcNotReady) => IpcError::IpcNotReady(message),
            Some(ErrorCode::RateLimited) => IpcError::RateLimited(message),
            Some(ErrorCode::VersionMismatch) => IpcError::VersionMismatch(message),
            other => IpcError::Daemon(message, other),
        }
    }
}

// ---------------------------------------------------------------------------
// Wire types
// ---------------------------------------------------------------------------

/// Wire-level response from the daemon.
///
/// W3.3: gained an optional [`ErrorCode`] parsed from the daemon's
/// `error_code` field. Existing fields are unchanged.
#[derive(Debug)]
pub struct IpcResponse {
    pub ok: bool,
    pub data: Option<Value>,
    pub error: Option<String>,
    /// Typed machine-readable error code, when the daemon attached one.
    /// `None` on success and on legacy (untagged) error responses.
    pub error_code: Option<ErrorCode>,
}

/// A history item returned by `history_page`.
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub id: String,
    pub content_type: String,
    pub preview: String,
    pub is_sensitive: bool,
    /// Unix epoch milliseconds
    pub wall_time: i64,
}

/// Paginated history result from the daemon.
#[derive(Debug)]
pub struct HistoryPage {
    pub items: Vec<HistoryEntry>,
    pub total: u64,
}

// ---------------------------------------------------------------------------
// Settings / config types
// ---------------------------------------------------------------------------

/// Application configuration persisted via the daemon.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default)]
    pub p2p_enabled: bool,
    #[serde(default)]
    pub supabase_url: Option<String>,
    #[serde(default)]
    pub supabase_anon_key: Option<String>,
}

// ---------------------------------------------------------------------------
// Peer types
// ---------------------------------------------------------------------------

/// A device paired for P2P clipboard sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedDevice {
    pub fingerprint: String,
    pub name: String,
}

// ---------------------------------------------------------------------------
// IpcClient
// ---------------------------------------------------------------------------

pub struct IpcClient {
    stream: UnixStream,
}

/// Server-side page limit enforced by the daemon for `history_page`.
/// Mirrored here so the UI can clamp `limit` before sending the request.
/// Source: see Wave 2.3 — `daemon` rejects pages larger than `MAX_PAGE`.
pub const MAX_PAGE: u64 = 1000;

impl IpcClient {
    /// Connect to the daemon Unix socket.
    ///
    /// Returns [`IpcError::DaemonOffline`] when the socket file is missing
    /// or the connection is refused (the common case when the daemon has
    /// not been started yet). The UI surface translates this into an
    /// empty-state hint instead of crashing.
    pub fn connect(socket_path: &Path) -> Result<Self> {
        match UnixStream::connect(socket_path) {
            Ok(stream) => Ok(Self { stream }),
            Err(e)
                if e.kind() == std::io::ErrorKind::NotFound
                    || e.kind() == std::io::ErrorKind::ConnectionRefused =>
            {
                Err(IpcError::DaemonOffline(socket_path.to_path_buf()).into())
            }
            Err(e) => Err(IpcError::Io(e).into()),
        }
    }

    fn call(&mut self, method: &str, params: Value) -> Result<IpcResponse> {
        let req = serde_json::json!({
            "id": "ui-1",
            "method": method,
            "params": params,
        });
        let mut line = serde_json::to_string(&req)?;
        line.push('\n');
        self.stream
            .write_all(line.as_bytes())
            .context("write to daemon socket failed")?;

        let mut reader = BufReader::new(&self.stream);
        let mut resp_line = String::new();
        reader
            .read_line(&mut resp_line)
            .context("read from daemon socket failed")?;

        if resp_line.is_empty() {
            return Err(anyhow!("daemon closed connection without response"));
        }

        let v: Value =
            serde_json::from_str(resp_line.trim()).context("invalid JSON from daemon")?;

        Ok(IpcResponse {
            ok: v["ok"].as_bool().unwrap_or(false),
            data: if v["data"].is_null() {
                None
            } else {
                Some(v["data"].clone())
            },
            error: v["error"].as_str().map(str::to_owned),
            // W3.3: parse the machine-readable `error_code` if the daemon
            // attached one. Unknown / missing codes collapse to `None` so
            // older daemons keep working unchanged.
            error_code: v["error_code"].as_str().and_then(ErrorCode::parse),
        })
    }

    // -----------------------------------------------------------------------
    // History methods
    // -----------------------------------------------------------------------

    /// Fetch a page of clipboard history from the daemon.
    ///
    /// `limit` is clamped to [`MAX_PAGE`] before the request is sent —
    /// the server enforces `MAX_PAGE=1000` and would reject anything larger
    /// (see Wave 2.3). Clamping client-side avoids a round-trip error and
    /// keeps the UI responsive when callers ask for an entire 10k+ history.
    pub fn history_page(&mut self, limit: u64, offset: u64) -> Result<HistoryPage> {
        // server enforces MAX_PAGE=1000
        let effective_limit = build_history_limit(limit);
        let resp = self.call(
            "history_page",
            serde_json::json!({
                "limit": effective_limit,
                "offset": offset,
            }),
        )?;

        if !resp.ok {
            return Err(anyhow!(
                "daemon error: {}",
                resp.error.unwrap_or_else(|| "unknown".into())
            ));
        }

        let data = resp.data.unwrap_or(Value::Null);
        let total = data["total"].as_u64().unwrap_or(0);
        let raw_items = data["items"].as_array().cloned().unwrap_or_default();

        let items: Vec<HistoryEntry> = raw_items
            .into_iter()
            .map(|v| HistoryEntry {
                id: v["id"].as_str().unwrap_or("").to_owned(),
                content_type: v["content_type"].as_str().unwrap_or("text").to_owned(),
                preview: v["preview"].as_str().unwrap_or("").to_owned(),
                is_sensitive: v["is_sensitive"].as_bool().unwrap_or(false),
                wall_time: v["wall_time"].as_i64().unwrap_or(0),
            })
            .collect();

        Ok(HistoryPage { items, total })
    }

    /// Ask the daemon to paste a history item back to the system clipboard.
    /// Returns the item ID on success.
    pub fn paste(&mut self, item_id: &str) -> Result<String> {
        let resp = self.call("paste", serde_json::json!({ "id": item_id }))?;
        if resp.ok {
            Ok(item_id.to_owned())
        } else {
            Err(anyhow!(
                "paste failed: {}",
                resp.error.unwrap_or_else(|| "unknown".into())
            ))
        }
    }

    /// Copy a history item back to the system clipboard by id.
    ///
    /// T5.x: uses the daemon `copy_item` verb, which surfaces typed
    /// `invalid_argument` / `not_found` / `auth_failed` error codes. The
    /// daemon decrypts the stored ciphertext and writes plaintext to the
    /// system clipboard. Returns the item id on success.
    pub fn copy_item(&mut self, item_id: &str) -> Result<String> {
        let resp = self.call("copy_item", serde_json::json!({ "id": item_id }))?;
        if resp.ok {
            Ok(item_id.to_owned())
        } else {
            Err(IpcError::from_code(
                resp.error_code,
                resp.error.unwrap_or_else(|| "copy_item failed".into()),
            )
            .into())
        }
    }

    /// Pin or unpin a history item by id.
    ///
    /// T5.x: `pinned = true` removes the item's expiry so it survives TTL
    /// and history-limit prunes; `pinned = false` restores normal behaviour.
    pub fn pin_item(&mut self, item_id: &str, pinned: bool) -> Result<()> {
        let resp = self.call(
            "pin_item",
            serde_json::json!({ "id": item_id, "pinned": pinned }),
        )?;
        if resp.ok {
            Ok(())
        } else {
            Err(IpcError::from_code(
                resp.error_code,
                resp.error.unwrap_or_else(|| "pin_item failed".into()),
            )
            .into())
        }
    }

    /// Delete a single history item by id.
    ///
    /// T5.x: uses the daemon `delete_item` verb (typed error codes). FTS
    /// cleanup is best-effort on the daemon side.
    pub fn delete_item(&mut self, item_id: &str) -> Result<()> {
        let resp = self.call("delete_item", serde_json::json!({ "id": item_id }))?;
        if resp.ok {
            Ok(())
        } else {
            Err(IpcError::from_code(
                resp.error_code,
                resp.error.unwrap_or_else(|| "delete_item failed".into()),
            )
            .into())
        }
    }

    /// Delete every clipboard-history item (Settings → "Clear history").
    ///
    /// Returns the number of items the daemon deleted.
    pub fn delete_all(&mut self) -> Result<u64> {
        let resp = self.call("delete_all", serde_json::json!({}))?;
        if !resp.ok {
            return Err(IpcError::from_code(
                resp.error_code,
                resp.error.unwrap_or_else(|| "delete_all failed".into()),
            )
            .into());
        }
        let data = resp.data.unwrap_or(Value::Null);
        Ok(data["deleted"].as_u64().unwrap_or(0))
    }

    /// Ping the daemon and return true if it responds.
    #[allow(dead_code)]
    pub fn is_running(&mut self) -> bool {
        self.call("status", Value::Null)
            .map(|r| r.ok)
            .unwrap_or(false)
    }

    // -----------------------------------------------------------------------
    // Settings methods
    // -----------------------------------------------------------------------

    /// Read the application configuration from the daemon.
    pub fn get_settings(&mut self) -> Result<AppSettings> {
        let resp = self.call("get_config", Value::Null)?;
        if !resp.ok {
            return Err(anyhow!(
                "get_config failed: {}",
                resp.error.unwrap_or_else(|| "unknown".into())
            ));
        }
        let data = resp.data.unwrap_or(Value::Null);
        let settings: AppSettings =
            serde_json::from_value(data).context("invalid AppSettings JSON from daemon")?;
        Ok(settings)
    }

    /// Persist application configuration via the daemon.
    #[allow(dead_code)]
    pub fn save_settings(&mut self, settings: &AppSettings) -> Result<()> {
        let params = serde_json::to_value(settings).context("failed to serialize AppSettings")?;
        let resp = self.call("set_config", params)?;
        if resp.ok {
            Ok(())
        } else {
            Err(anyhow!(
                "set_config failed: {}",
                resp.error.unwrap_or_else(|| "unknown".into())
            ))
        }
    }

    // -----------------------------------------------------------------------
    // P2P peer methods
    // -----------------------------------------------------------------------

    /// Return this device's X25519 public key fingerprint.
    ///
    /// Returns `Ok(None)` until the daemon implements p2p key management.
    pub fn get_own_fingerprint(&mut self) -> Result<String> {
        // TODO: connect once daemon has X25519 key management
        let resp = self.call("get_own_fingerprint", Value::Null)?;
        if !resp.ok {
            return Err(anyhow!(
                "get_own_fingerprint failed: {}",
                resp.error.unwrap_or_else(|| "unknown".into())
            ));
        }
        let data = resp.data.unwrap_or(Value::Null);
        let fp = data["fingerprint"].as_str().unwrap_or("").to_owned();
        Ok(fp)
    }

    /// List all paired P2P devices.
    ///
    /// Returns an empty list until the daemon implements p2p peer storage.
    pub fn list_peers(&mut self) -> Result<Vec<PairedDevice>> {
        // TODO: connect once daemon has p2p peer storage
        let resp = self.call("list_peers", Value::Null)?;
        if !resp.ok {
            return Err(anyhow!(
                "list_peers failed: {}",
                resp.error.unwrap_or_else(|| "unknown".into())
            ));
        }
        let data = resp.data.unwrap_or(Value::Null);
        let raw = data["peers"].as_array().cloned().unwrap_or_default();
        let peers = raw
            .into_iter()
            .filter_map(|v| match serde_json::from_value::<PairedDevice>(v) {
                Ok(d) => Some(d),
                Err(e) => {
                    tracing::warn!("dropping malformed PairedDevice JSON: {e}");
                    None
                }
            })
            .collect();
        Ok(peers)
    }

    /// Initiate pairing with a peer identified by fingerprint.
    ///
    /// Returns `Ok(())` immediately; actual pairing is async and not yet implemented.
    pub fn pair_peer(&mut self, fingerprint: &str, name: &str) -> Result<()> {
        // TODO: connect once daemon implements PAKE handshake
        let resp = self.call(
            "pair_peer",
            serde_json::json!({
                "fingerprint": fingerprint,
                "name": name,
            }),
        )?;
        if resp.ok {
            Ok(())
        } else {
            Err(anyhow!(
                "pair_peer failed: {}",
                resp.error.unwrap_or_else(|| "unknown".into())
            ))
        }
    }

    /// Remove a paired peer by fingerprint.
    pub fn unpair_peer(&mut self, fingerprint: &str) -> Result<()> {
        // TODO: connect once daemon implements p2p peer storage
        let resp = self.call(
            "unpair_peer",
            serde_json::json!({
                "fingerprint": fingerprint,
            }),
        )?;
        if resp.ok {
            Ok(())
        } else {
            Err(anyhow!(
                "unpair_peer failed: {}",
                resp.error.unwrap_or_else(|| "unknown".into())
            ))
        }
    }

    /// T4 (v0.3) — manually revoke a paired peer.
    ///
    /// Differs from [`unpair_peer`] in that the daemon additionally writes
    /// a row to the `revoked_devices` audit table. The v1.0 cryptographic
    /// revocation protocol will later read that table to publish revocation
    /// markers; for v0.3 the audit row is the only durable record beyond
    /// the local peer-store deletion.
    ///
    /// Returns the unix-seconds `revoked_at` timestamp the daemon recorded,
    /// so the UI can show "Revoked just now" feedback without a re-query.
    pub fn revoke_peer(&mut self, fingerprint: &str) -> Result<u64> {
        let resp = self.call(
            "revoke_peer",
            serde_json::json!({
                "fingerprint": fingerprint,
            }),
        )?;
        if !resp.ok {
            return Err(anyhow!(
                "revoke_peer failed: {}",
                resp.error.unwrap_or_else(|| "unknown".into())
            ));
        }
        let data = resp.data.unwrap_or(Value::Null);
        let revoked_at = data["revoked_at"].as_u64().unwrap_or(0);
        Ok(revoked_at)
    }

    /// Build the wire-level JSON for a `revoke_peer` request. Exposed for
    /// unit tests that want to assert method/param shape without standing
    /// up a real daemon.
    pub fn build_revoke_peer_request(fingerprint: &str) -> Value {
        serde_json::json!({
            "id": "ui-1",
            "method": "revoke_peer",
            "params": {
                "fingerprint": fingerprint,
            },
        })
    }

    /// T5.x — revoke ALL paired peers (Settings → "Reset pairings").
    ///
    /// Clears the daemon's local peer store and writes an audit row per
    /// peer. Returns the number of audit rows the daemon recorded.
    pub fn revoke_all_peers(&mut self) -> Result<u64> {
        let resp = self.call("revoke_all_peers", serde_json::json!({}))?;
        if !resp.ok {
            return Err(IpcError::from_code(
                resp.error_code,
                resp.error
                    .unwrap_or_else(|| "revoke_all_peers failed".into()),
            )
            .into());
        }
        let data = resp.data.unwrap_or(Value::Null);
        Ok(data["revoked"].as_u64().unwrap_or(0))
    }

    // -----------------------------------------------------------------------
    // Private mode methods
    // -----------------------------------------------------------------------

    /// Read the current private-mode state from the daemon.
    ///
    /// Returns `Ok(false)` when the daemon is unreachable so the tray can
    /// fall back gracefully without crashing the UI.
    pub fn get_private_mode(&mut self) -> Result<bool> {
        let resp = self.call("get_private_mode", serde_json::json!({}))?;
        if !resp.ok {
            return Err(anyhow!(
                "get_private_mode failed: {}",
                resp.error.unwrap_or_else(|| "unknown".into())
            ));
        }
        let data = resp.data.unwrap_or(Value::Null);
        let enabled = data["private_mode"].as_bool().unwrap_or(false);
        Ok(enabled)
    }

    /// Persist the private-mode toggle via the daemon.
    pub fn set_private_mode(&mut self, enabled: bool) -> Result<()> {
        let resp = self.call(
            "set_private_mode",
            serde_json::json!({ "enabled": enabled }),
        )?;
        if resp.ok {
            Ok(())
        } else {
            Err(anyhow!(
                "set_private_mode failed: {}",
                resp.error.unwrap_or_else(|| "unknown".into())
            ))
        }
    }

    /// Build the wire-level JSON for a `get_private_mode` request.
    /// Exposed for unit tests that want to assert method/param shape
    /// without standing up a real daemon.
    pub fn build_get_private_mode_request() -> Value {
        serde_json::json!({
            "id": "ui-1",
            "method": "get_private_mode",
            "params": {},
        })
    }

    /// Build the wire-level JSON for a `set_private_mode` request.
    /// Exposed for unit tests that want to assert method/param shape
    /// without standing up a real daemon.
    pub fn build_set_private_mode_request(enabled: bool) -> Value {
        serde_json::json!({
            "id": "ui-1",
            "method": "set_private_mode",
            "params": { "enabled": enabled },
        })
    }

    // -----------------------------------------------------------------------
    // Cloud auth methods
    // -----------------------------------------------------------------------

    /// Sign in to the cloud sync backend with email + password.
    ///
    /// Returns `Ok(())` immediately; actual sign-in is a stub until Supabase is wired.
    pub fn cloud_sign_in(&mut self, email: &str, password: &str) -> Result<()> {
        // TODO: connect once daemon has Supabase auth integration
        let resp = self.call(
            "cloud_sign_in",
            serde_json::json!({
                "email": email,
                "password": password,
            }),
        )?;
        if resp.ok {
            Ok(())
        } else {
            Err(anyhow!(
                "cloud_sign_in failed: {}",
                resp.error.unwrap_or_else(|| "unknown".into())
            ))
        }
    }

    /// Sign out from the cloud sync backend.
    pub fn cloud_sign_out(&mut self) -> Result<()> {
        // TODO: connect once daemon has Supabase auth integration
        let resp = self.call("cloud_sign_out", Value::Null)?;
        if resp.ok {
            Ok(())
        } else {
            Err(anyhow!(
                "cloud_sign_out failed: {}",
                resp.error.unwrap_or_else(|| "unknown".into())
            ))
        }
    }

    // -----------------------------------------------------------------------
    // PAKE pairing (Beta W3.2)
    // -----------------------------------------------------------------------

    /// Initiate PAKE-based pairing with a peer using a shared password.
    ///
    /// Sends the `pair_peer_with_password` IPC method with `peer_fingerprint`
    /// and `password` parameters. The daemon validates both inputs and (in a
    /// full implementation) routes 3 PAKE handshake messages over the p2p
    /// Transport. Until that wiring lands, valid requests return a typed
    /// `not_implemented` error which callers surface in the UI status text.
    pub fn pair_with_password(&mut self, peer_fingerprint: &str, password: &str) -> Result<()> {
        let resp = self.call(
            "pair_peer_with_password",
            serde_json::json!({
                "peer_fingerprint": peer_fingerprint,
                "password": password,
            }),
        )?;
        if resp.ok {
            Ok(())
        } else {
            Err(anyhow!(
                "pair_peer_with_password failed: {}",
                resp.error.unwrap_or_else(|| "unknown".into())
            ))
        }
    }

    /// Build the wire-level JSON for a `pair_peer_with_password` request.
    /// Exposed for unit testing without a running daemon — the request
    /// builder mirrors what [`pair_with_password`] sends so tests can assert
    /// the method name and parameter shape.
    pub fn build_pair_with_password_request(peer_fingerprint: &str, password: &str) -> Value {
        serde_json::json!({
            "id": "ui-1",
            "method": "pair_peer_with_password",
            "params": {
                "peer_fingerprint": peer_fingerprint,
                "password": password,
            },
        })
    }

    /// Minimum length required for a PAKE pairing password. Enforced in the
    /// UI as a quick guard before round-tripping to the daemon (which also
    /// double-checks); kept in one place so both layers agree.
    pub const MIN_PAIR_PASSWORD_LEN: usize = 6;

    /// Validate a password against [`MIN_PAIR_PASSWORD_LEN`] using Unicode
    /// scalar counts so multibyte characters count as one.
    pub fn is_valid_pair_password(password: &str) -> bool {
        password.chars().count() >= Self::MIN_PAIR_PASSWORD_LEN
    }

    // -----------------------------------------------------------------------
    // Request builders for the T5.x history-action verbs (unit-testable)
    // -----------------------------------------------------------------------

    /// Build the wire-level JSON for a `copy_item` request.
    pub fn build_copy_item_request(item_id: &str) -> Value {
        serde_json::json!({
            "id": "ui-1",
            "method": "copy_item",
            "params": { "id": item_id },
        })
    }

    /// Build the wire-level JSON for a `pin_item` request.
    pub fn build_pin_item_request(item_id: &str, pinned: bool) -> Value {
        serde_json::json!({
            "id": "ui-1",
            "method": "pin_item",
            "params": { "id": item_id, "pinned": pinned },
        })
    }

    /// Build the wire-level JSON for a `delete_item` request.
    pub fn build_delete_item_request(item_id: &str) -> Value {
        serde_json::json!({
            "id": "ui-1",
            "method": "delete_item",
            "params": { "id": item_id },
        })
    }

    /// Build the wire-level JSON for a `delete_all` request.
    pub fn build_delete_all_request() -> Value {
        serde_json::json!({
            "id": "ui-1",
            "method": "delete_all",
            "params": {},
        })
    }

    /// Build the wire-level JSON for a `revoke_all_peers` request.
    pub fn build_revoke_all_peers_request() -> Value {
        serde_json::json!({
            "id": "ui-1",
            "method": "revoke_all_peers",
            "params": {},
        })
    }
}

// ---------------------------------------------------------------------------
// Request builders (kept free-standing for unit testing without a daemon)
// ---------------------------------------------------------------------------

/// Clamp a caller-supplied page limit to the server's [`MAX_PAGE`].
///
/// A `limit` of `0` falls through unchanged so the daemon can return its own
/// "missing/invalid limit" error; only oversize values are capped.
#[inline]
pub fn build_history_limit(limit: u64) -> u64 {
    if limit > MAX_PAGE {
        MAX_PAGE
    } else {
        limit
    }
}

// ---------------------------------------------------------------------------
// Time formatting helpers
// ---------------------------------------------------------------------------

/// Format Unix epoch milliseconds as a human-readable string without external deps.
pub fn format_wall_time(ms: i64) -> String {
    if ms <= 0 {
        return "\u{2014}".to_string();
    }
    let secs = (ms / 1000) as u64;
    let (y, mo, d, h, mi, s) = secs_to_ymd_hms(secs);
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, mo, d, h, mi, s)
}

fn secs_to_ymd_hms(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;
    let (y, mo, d) = days_to_ymd(days);
    (y, mo, d, h, m, s)
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970u64;
    loop {
        let dy = if is_leap(year) { 366 } else { 365 };
        if days < dy {
            break;
        }
        days -= dy;
        year += 1;
    }
    let months: [u64; 12] = if is_leap(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1u64;
    let mut remaining = days;
    for md in months.iter() {
        if remaining < *md {
            break;
        }
        remaining -= md;
        month += 1;
    }
    (year, month, remaining + 1)
}

fn is_leap(y: u64) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_wall_time_zero_returns_dash() {
        assert_eq!(format_wall_time(0), "\u{2014}");
    }

    #[test]
    fn format_wall_time_negative_returns_dash() {
        assert_eq!(format_wall_time(-1), "\u{2014}");
    }

    #[test]
    fn format_wall_time_known_date() {
        // 2024-01-01 00:00:00 UTC = 1704067200000 ms
        let s = format_wall_time(1_704_067_200_000);
        assert_eq!(s, "2024-01-01 00:00:00");
    }

    #[test]
    fn format_wall_time_structure() {
        let s = format_wall_time(1_750_000_496_000);
        assert_eq!(s.len(), 19);
        assert_eq!(&s[4..5], "-");
        assert_eq!(&s[7..8], "-");
        assert_eq!(&s[10..11], " ");
        assert_eq!(&s[13..14], ":");
        assert_eq!(&s[16..17], ":");
    }

    #[test]
    fn ipc_client_connect_fails_when_no_socket() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.sock");
        assert!(IpcClient::connect(&path).is_err());
    }

    #[test]
    fn ipc_client_returns_error_on_daemon_offline() {
        // Wave 3.1 fix #25: HistoryWindow opening before daemon socket
        // must surface a typed DaemonOffline error so the UI can show
        // an empty-state hint instead of crashing.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.sock");
        let err = IpcClient::connect(&path)
            .err()
            .expect("expected connect failure");

        // The anyhow::Error must carry our typed IpcError::DaemonOffline.
        let typed = err
            .downcast_ref::<IpcError>()
            .expect("expected IpcError on offline daemon");
        match typed {
            IpcError::DaemonOffline(p) => assert_eq!(p, &path),
            other => panic!("expected DaemonOffline, got {other:?}"),
        }

        // The displayed message must contain the user-actionable hint so
        // the empty-state in the HistoryWindow reads naturally.
        let msg = err.to_string();
        assert!(
            msg.contains("Daemon not running"),
            "missing user hint, got: {msg}"
        );
        assert!(
            msg.contains("copypaste daemon start"),
            "missing recovery command, got: {msg}"
        );
    }

    #[test]
    fn history_request_limit_capped_at_1000() {
        // Wave 3.1 fix #26: even if the UI requests 10k+ items, the
        // builder must cap `limit` at MAX_PAGE=1000 to match the
        // server-side enforcement added in Wave 2.3.
        assert_eq!(build_history_limit(1), 1, "small limit untouched");
        assert_eq!(build_history_limit(50), 50, "default page untouched");
        assert_eq!(
            build_history_limit(1000),
            1000,
            "exactly MAX_PAGE untouched"
        );
        assert_eq!(
            build_history_limit(1001),
            1000,
            "just-over limit clamped to MAX_PAGE"
        );
        assert_eq!(
            build_history_limit(10_000),
            1000,
            "10k items clamped to MAX_PAGE"
        );
        assert_eq!(
            build_history_limit(u64::MAX),
            1000,
            "u64::MAX clamped to MAX_PAGE"
        );
        // Zero falls through so the daemon can issue its own validation error.
        assert_eq!(build_history_limit(0), 0, "zero passes through");
    }

    #[test]
    fn app_settings_default() {
        let s = AppSettings::default();
        assert!(!s.p2p_enabled);
        assert!(s.supabase_url.is_none());
        assert!(s.supabase_anon_key.is_none());
    }

    #[test]
    fn typed_error_variant_from_response_code() {
        // W3.3: `IpcError::from_code` must map every known ErrorCode onto
        // its typed variant, and fall back to `Daemon(_, None)` for
        // missing / unknown codes (forward-compat for older daemons).
        type Case = (Option<ErrorCode>, &'static str, fn(&IpcError) -> bool);
        let cases: &[Case] = &[
            (Some(ErrorCode::NotImplemented), "cloud sync", |e| {
                matches!(e, IpcError::NotImplemented(_))
            }),
            (Some(ErrorCode::AuthFailed), "bad password", |e| {
                matches!(e, IpcError::AuthFailed(_))
            }),
            (Some(ErrorCode::NotFound), "item missing", |e| {
                matches!(e, IpcError::NotFound(_))
            }),
            (Some(ErrorCode::InvalidArgument), "bad param", |e| {
                matches!(e, IpcError::InvalidArgument(_))
            }),
            (Some(ErrorCode::IpcNotReady), "db booting", |e| {
                matches!(e, IpcError::IpcNotReady(_))
            }),
            (Some(ErrorCode::RateLimited), "slow down", |e| {
                matches!(e, IpcError::RateLimited(_))
            }),
            (Some(ErrorCode::VersionMismatch), "bump client", |e| {
                matches!(e, IpcError::VersionMismatch(_))
            }),
            (Some(ErrorCode::InternalError), "panic", |e| {
                matches!(e, IpcError::Daemon(_, Some(ErrorCode::InternalError)))
            }),
            (None, "legacy err", |e| {
                matches!(e, IpcError::Daemon(_, None))
            }),
        ];

        for (code, msg, check) in cases {
            let err = IpcError::from_code(*code, (*msg).to_string());
            assert!(check(&err), "wrong variant for code {code:?}: got {err:?}");
            // Message must be preserved verbatim so the UI shows the
            // daemon's wording rather than a generic placeholder.
            assert!(
                err.to_string().contains(msg),
                "message lost for code {code:?}: {err}"
            );
        }
    }

    #[test]
    fn ipc_response_parses_error_code_field() {
        // Round-trip a JSON-shaped Value through the same parsing logic
        // `call()` uses for the `error_code` field, without needing a
        // live daemon socket.
        let v: Value = serde_json::from_str(
            r#"{"id":1,"ok":false,"error":"nope","error_code":"not_implemented"}"#,
        )
        .unwrap();
        let parsed_code = v["error_code"].as_str().and_then(ErrorCode::parse);
        assert_eq!(parsed_code, Some(ErrorCode::NotImplemented));

        // Unknown code collapses to None (forward-compat).
        let v2: Value =
            serde_json::from_str(r#"{"id":1,"ok":false,"error":"x","error_code":"future_code"}"#)
                .unwrap();
        let parsed_code = v2["error_code"].as_str().and_then(ErrorCode::parse);
        assert_eq!(parsed_code, None);

        // Missing field collapses to None (legacy daemon, no regression).
        let v3: Value = serde_json::from_str(r#"{"id":1,"ok":false,"error":"x"}"#).unwrap();
        let parsed_code = v3["error_code"].as_str().and_then(ErrorCode::parse);
        assert_eq!(parsed_code, None);
    }

    #[test]
    fn app_settings_round_trip_json() {
        let s = AppSettings {
            p2p_enabled: true,
            supabase_url: Some("https://example.supabase.co".into()),
            supabase_anon_key: Some("key123".into()),
        };
        let json = serde_json::to_string(&s).unwrap();
        let s2: AppSettings = serde_json::from_str(&json).unwrap();
        assert!(s2.p2p_enabled);
        assert_eq!(
            s2.supabase_url.as_deref(),
            Some("https://example.supabase.co")
        );
        assert_eq!(s2.supabase_anon_key.as_deref(), Some("key123"));
    }

    #[test]
    fn ipc_client_pair_with_password_sends_correct_method() {
        // beta-W3.2: builder must use exactly `pair_peer_with_password` so
        // the daemon dispatcher matches; both params must be present and
        // verbatim — the daemon rejects missing/renamed fields.
        let req = IpcClient::build_pair_with_password_request("abc123", "hunter22");
        assert_eq!(req["method"], "pair_peer_with_password");
        assert_eq!(req["params"]["peer_fingerprint"], "abc123");
        assert_eq!(req["params"]["password"], "hunter22");
        assert!(
            req["id"].is_string(),
            "every IPC request needs a string id for matching responses"
        );
    }

    #[test]
    fn ipc_client_revoke_peer_sends_correct_method() {
        // T4 (v0.3): the builder must use exactly `revoke_peer` (not
        // `unpair_peer`) so the daemon writes the audit row, and the
        // fingerprint parameter must be passed verbatim — the daemon
        // validates the XX:XX:... shape on its end.
        let fp = "ab:cd:ef:01:23:45:67:89";
        let req = IpcClient::build_revoke_peer_request(fp);
        assert_eq!(req["method"], "revoke_peer");
        assert_eq!(req["params"]["fingerprint"], fp);
        assert!(req["id"].is_string(), "request needs a string id");
    }

    #[test]
    fn ipc_client_get_private_mode_sends_correct_method() {
        // C.H9: builder must use exactly `get_private_mode` so the daemon
        // dispatcher matches; params must be an empty object (not null) per
        // the protocol spec.
        let req = IpcClient::build_get_private_mode_request();
        assert_eq!(req["method"], "get_private_mode");
        assert!(
            req["params"].is_object(),
            "params must be an object (not null)"
        );
        assert!(req["id"].is_string(), "request needs a string id");
    }

    #[test]
    fn ipc_client_set_private_mode_sends_correct_method() {
        // C.H9: builder must use exactly `set_private_mode` and pass the
        // `enabled` bool verbatim — the daemon rejects missing/renamed fields.
        let req_on = IpcClient::build_set_private_mode_request(true);
        assert_eq!(req_on["method"], "set_private_mode");
        assert_eq!(req_on["params"]["enabled"], true);
        assert!(req_on["id"].is_string(), "request needs a string id");

        let req_off = IpcClient::build_set_private_mode_request(false);
        assert_eq!(req_off["method"], "set_private_mode");
        assert_eq!(req_off["params"]["enabled"], false);
    }

    #[test]
    fn pair_window_password_field_validates_min_length() {
        // beta-W3.2: password must be at least MIN_PAIR_PASSWORD_LEN (6)
        // chars. Both the UI and the daemon enforce this — the UI check
        // avoids a useless round-trip; the daemon check is the source of
        // truth so a malicious client cannot bypass it.
        assert!(!IpcClient::is_valid_pair_password(""), "empty rejected");
        assert!(
            !IpcClient::is_valid_pair_password("ab"),
            "too short rejected"
        );
        assert!(
            !IpcClient::is_valid_pair_password("12345"),
            "5 chars rejected"
        );
        assert!(
            IpcClient::is_valid_pair_password("123456"),
            "exactly 6 chars accepted"
        );
        assert!(
            IpcClient::is_valid_pair_password("hunter22"),
            "longer accepted"
        );
        // Multibyte: 6 Unicode scalars regardless of UTF-8 byte length.
        assert!(
            IpcClient::is_valid_pair_password("парол1"),
            "Cyrillic 6-scalar password must be accepted (chars, not bytes)"
        );
        assert!(
            !IpcClient::is_valid_pair_password("ab漢"),
            "3-scalar password must be rejected even if multibyte UTF-8"
        );
        assert_eq!(
            IpcClient::MIN_PAIR_PASSWORD_LEN,
            6,
            "constant must stay in sync with the daemon-side check"
        );
    }

    #[test]
    fn ipc_client_copy_item_sends_correct_method() {
        // T5.x: builder must use exactly `copy_item` and pass the id verbatim.
        let req = IpcClient::build_copy_item_request("abc-123");
        assert_eq!(req["method"], "copy_item");
        assert_eq!(req["params"]["id"], "abc-123");
        assert!(req["id"].is_string(), "request needs a string id");
    }

    #[test]
    fn ipc_client_pin_item_sends_correct_method_and_flag() {
        // T5.x: builder must use `pin_item` and carry the `pinned` bool so
        // the daemon can toggle pin state from a single verb.
        let req_on = IpcClient::build_pin_item_request("abc-123", true);
        assert_eq!(req_on["method"], "pin_item");
        assert_eq!(req_on["params"]["id"], "abc-123");
        assert_eq!(req_on["params"]["pinned"], true);

        let req_off = IpcClient::build_pin_item_request("abc-123", false);
        assert_eq!(req_off["params"]["pinned"], false);
    }

    #[test]
    fn ipc_client_delete_item_sends_correct_method() {
        let req = IpcClient::build_delete_item_request("abc-123");
        assert_eq!(req["method"], "delete_item");
        assert_eq!(req["params"]["id"], "abc-123");
        assert!(req["id"].is_string(), "request needs a string id");
    }

    #[test]
    fn ipc_client_delete_all_sends_correct_method() {
        let req = IpcClient::build_delete_all_request();
        assert_eq!(req["method"], "delete_all");
        assert!(
            req["params"].is_object(),
            "params must be an object (not null)"
        );
        assert!(req["id"].is_string(), "request needs a string id");
    }

    #[test]
    fn ipc_client_revoke_all_peers_sends_correct_method() {
        let req = IpcClient::build_revoke_all_peers_request();
        assert_eq!(req["method"], "revoke_all_peers");
        assert!(
            req["params"].is_object(),
            "params must be an object (not null)"
        );
        assert!(req["id"].is_string(), "request needs a string id");
    }
}

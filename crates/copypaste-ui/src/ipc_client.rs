// Permit dead code — many IPC methods are stubs for future UI features.
#![allow(dead_code)]

/// Synchronous IPC client for the copypaste-daemon Unix socket.
///
/// Protocol: newline-delimited JSON.
///   Request:  {"id":"<req_id>","method":"<method>","params":{...}}
///   Response: {"id":"<req_id>","ok":<bool>,"data":<value>,"error":"<msg>"}
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// IpcError
// ---------------------------------------------------------------------------

/// Structured connection errors. Exposed so callers can detect a missing
/// daemon (vs. a protocol/IO failure) and show a friendly empty-state.
#[derive(Debug)]
pub enum IpcError {
    /// The daemon socket file does not exist or refused the connection —
    /// the daemon is most likely not running. Carries the socket path that
    /// was attempted so the UI can include it in the empty-state text.
    DaemonOffline(std::path::PathBuf),
    /// Any other IO error while connecting.
    Io(std::io::Error),
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
        }
    }
}

impl std::error::Error for IpcError {}

// ---------------------------------------------------------------------------
// Wire types
// ---------------------------------------------------------------------------

/// Wire-level response from the daemon.
#[derive(Debug)]
pub struct IpcResponse {
    pub ok: bool,
    pub data: Option<Value>,
    pub error: Option<String>,
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

        let v: Value = serde_json::from_str(resp_line.trim())
            .context("invalid JSON from daemon")?;

        Ok(IpcResponse {
            ok: v["ok"].as_bool().unwrap_or(false),
            data: if v["data"].is_null() { None } else { Some(v["data"].clone()) },
            error: v["error"].as_str().map(str::to_owned),
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
        let resp = self.call("history_page", serde_json::json!({
            "limit": effective_limit,
            "offset": offset,
        }))?;

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
        let settings: AppSettings = serde_json::from_value(data)
            .context("invalid AppSettings JSON from daemon")?;
        Ok(settings)
    }

    /// Persist application configuration via the daemon.
    #[allow(dead_code)]
    pub fn save_settings(&mut self, settings: &AppSettings) -> Result<()> {
        let params = serde_json::to_value(settings)
            .context("failed to serialize AppSettings")?;
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
            .filter_map(|v| serde_json::from_value::<PairedDevice>(v).ok())
            .collect();
        Ok(peers)
    }

    /// Initiate pairing with a peer identified by fingerprint.
    ///
    /// Returns `Ok(())` immediately; actual pairing is async and not yet implemented.
    pub fn pair_peer(&mut self, fingerprint: &str, name: &str) -> Result<()> {
        // TODO: connect once daemon implements PAKE handshake
        let resp = self.call("pair_peer", serde_json::json!({
            "fingerprint": fingerprint,
            "name": name,
        }))?;
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
        let resp = self.call("unpair_peer", serde_json::json!({
            "fingerprint": fingerprint,
        }))?;
        if resp.ok {
            Ok(())
        } else {
            Err(anyhow!(
                "unpair_peer failed: {}",
                resp.error.unwrap_or_else(|| "unknown".into())
            ))
        }
    }

    // -----------------------------------------------------------------------
    // Cloud auth methods
    // -----------------------------------------------------------------------

    /// Sign in to the cloud sync backend with email + password.
    ///
    /// Returns `Ok(())` immediately; actual sign-in is a stub until Supabase is wired.
    pub fn cloud_sign_in(&mut self, email: &str, password: &str) -> Result<()> {
        // TODO: connect once daemon has Supabase auth integration
        let resp = self.call("cloud_sign_in", serde_json::json!({
            "email": email,
            "password": password,
        }))?;
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
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
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
        let err = IpcClient::connect(&path).err().expect("expected connect failure");

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
        assert_eq!(build_history_limit(1000), 1000, "exactly MAX_PAGE untouched");
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
    fn app_settings_round_trip_json() {
        let s = AppSettings {
            p2p_enabled: true,
            supabase_url: Some("https://example.supabase.co".into()),
            supabase_anon_key: Some("key123".into()),
        };
        let json = serde_json::to_string(&s).unwrap();
        let s2: AppSettings = serde_json::from_str(&json).unwrap();
        assert!(s2.p2p_enabled);
        assert_eq!(s2.supabase_url.as_deref(), Some("https://example.supabase.co"));
        assert_eq!(s2.supabase_anon_key.as_deref(), Some("key123"));
    }
}

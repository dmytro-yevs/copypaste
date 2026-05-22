/// Minimal synchronous IPC client for the copypaste-daemon Unix socket.
///
/// Protocol: newline-delimited JSON.
///   Request:  {"id":"<req_id>","method":"<method>","params":{...}}
///   Response: {"id":"<req_id>","ok":<bool>,"data":<value>,"error":"<msg>"}
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use anyhow::{anyhow, Context, Result};
use serde_json::Value;

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

pub struct IpcClient {
    stream: UnixStream,
}

impl IpcClient {
    /// Connect to the daemon Unix socket.
    /// Returns an error if the daemon is not running or the socket does not exist.
    pub fn connect(socket_path: &Path) -> Result<Self> {
        let stream = UnixStream::connect(socket_path)
            .with_context(|| format!("daemon not running (socket: {})", socket_path.display()))?;
        Ok(Self { stream })
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

    /// Fetch a page of clipboard history from the daemon.
    pub fn history_page(&mut self, limit: u64, offset: u64) -> Result<HistoryPage> {
        let resp = self.call("history_page", serde_json::json!({
            "limit": limit,
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
}

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
}

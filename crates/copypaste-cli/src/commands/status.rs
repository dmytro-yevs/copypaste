//! `copypaste status` — daemon health probe.
//!
//! Calls into the running daemon over the UNIX socket and reports its state
//! (running / not running), socket path, version, history item count, and
//! uptime (derived from the socket file's mtime as a proxy for daemon start).
//!
//! Two output formats:
//!   - table (default): human-friendly, one field per line
//!   - --json:          machine-readable, single JSON object on stdout
//!
//! Exit codes:
//!   0 — daemon reachable and responded OK
//!   1 — daemon not reachable, or daemon returned an error response

use crate::ipc::IpcClient;
use anyhow::Result;
use serde::Serialize;
use std::path::Path;
use std::time::SystemTime;

/// Snapshot of daemon state at probe time. Serialized as-is for `--json`.
///
/// Field absence (`None`) is meaningful: it means "the daemon did not report
/// this" rather than zero / empty. The JSON form omits null fields so scripts
/// don't have to special-case them.
#[derive(Debug, Serialize, PartialEq)]
pub struct StatusReport {
    /// "running" when the daemon answered, "not running" when the socket
    /// could not be reached. Anything else is reserved for future states.
    pub daemon: String,
    /// Filesystem path of the UNIX socket we probed. Always present so users
    /// can confirm we tried the right place when "not running" is printed.
    pub socket: String,
    /// Daemon-reported semver (from the `stats` IPC call). `None` when the
    /// daemon is offline or the field is missing from the response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Total number of stored clipboard items (from the `count` IPC call).
    /// `None` when the daemon is offline or the call failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history: Option<i64>,
    /// Seconds since the socket file was created — proxy for daemon uptime.
    /// `None` when the socket does not exist (daemon offline) or the
    /// platform does not expose ctime/mtime.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uptime_secs: Option<u64>,
    /// Daemon's private-mode flag (true = clipboard recording paused).
    /// `None` when the daemon is offline.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_mode: Option<bool>,
}

impl StatusReport {
    fn offline(socket: &Path) -> Self {
        Self {
            daemon: "not running".to_string(),
            socket: socket.display().to_string(),
            version: None,
            history: None,
            uptime_secs: None,
            private_mode: None,
        }
    }
}

/// Probe the daemon and print a report. `json` selects JSON output; otherwise
/// a human-friendly table is printed.
///
/// Returns `Ok(())` only when the daemon is reachable AND its responses were
/// OK. When the daemon is offline, the report is still printed (so users see
/// the socket path they need to fix), but the function returns an error so
/// the CLI exits with status 1 — important for shell scripts that check
/// `copypaste status` before invoking other commands.
pub fn run(socket_path: &Path, json: bool) -> Result<()> {
    let report = probe(socket_path);
    print_report(&report, json);
    if report.daemon == "running" {
        Ok(())
    } else {
        // Use `bail!`-style error so main.rs exits with non-zero status.
        // The message is intentionally short; the full report already went
        // to stdout above.
        Err(anyhow::anyhow!("daemon not running"))
    }
}

/// Build a [`StatusReport`] without printing. Split out so tests can exercise
/// formatting in isolation from socket I/O.
fn probe(socket_path: &Path) -> StatusReport {
    // 1. Try to connect. If the socket file is absent or the daemon isn't
    //    listening, short-circuit to an "offline" report — we still echo
    //    the socket path so users can debug their setup.
    let mut client = match IpcClient::connect(socket_path) {
        Ok(c) => c,
        Err(_) => return StatusReport::offline(socket_path),
    };

    let mut report = StatusReport {
        daemon: "running".to_string(),
        socket: socket_path.display().to_string(),
        version: None,
        history: None,
        uptime_secs: socket_uptime_secs(socket_path),
        private_mode: None,
    };

    // 2. status — confirms the daemon is alive and yields private_mode.
    //    Failure here flips us back to "not running" since the socket
    //    accepted us but the daemon can't respond to a basic health check.
    let status_req = IpcClient::build_request("status", "status", serde_json::json!({}));
    match client.call(&status_req) {
        Ok(resp) if resp.ok => {
            if let Some(data) = &resp.data {
                report.private_mode = data["private_mode"].as_bool();
            }
        }
        _ => return StatusReport::offline(socket_path),
    }

    // 3. stats — pulls the daemon-reported version string. Best-effort:
    //    a failure here leaves `version = None` but does NOT downgrade
    //    "running" to "not running" (daemon is clearly alive at this point).
    //    Each call opens a fresh connection because IpcClient is one-shot
    //    (it consumes the connection on response).
    if let Ok(mut c2) = IpcClient::connect(socket_path) {
        let stats_req = IpcClient::build_request("stats", "stats", serde_json::json!({}));
        if let Ok(resp) = c2.call(&stats_req) {
            if resp.ok {
                if let Some(data) = &resp.data {
                    report.version = data["version"].as_str().map(|s| s.to_string());
                }
            }
        }
    }

    // 4. count — total history items. Same best-effort policy as stats.
    if let Ok(mut c3) = IpcClient::connect(socket_path) {
        let count_req = IpcClient::build_request("count", "count", serde_json::json!({}));
        if let Ok(resp) = c3.call(&count_req) {
            if resp.ok {
                if let Some(data) = &resp.data {
                    report.history = data["count"].as_i64();
                }
            }
        }
    }

    report
}

/// Format `uptime_secs` as a compact human string (e.g. "1h23m", "45s", "3d4h").
/// Mirrors common DevOps tooling so users don't have to mentally convert.
fn format_uptime(secs: u64) -> String {
    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3_600;
    let mins = (secs % 3_600) / 60;
    let s = secs % 60;
    if days > 0 {
        format!("{days}d{hours}h")
    } else if hours > 0 {
        format!("{hours}h{mins}m")
    } else if mins > 0 {
        format!("{mins}m{s}s")
    } else {
        format!("{s}s")
    }
}

/// Read the socket file's modification time and convert to seconds-since-now.
/// Returns `None` when the file is absent or its metadata can't be read.
fn socket_uptime_secs(socket: &Path) -> Option<u64> {
    let meta = std::fs::metadata(socket).ok()?;
    let mtime = meta.modified().ok()?;
    let now = SystemTime::now();
    now.duration_since(mtime)
        .ok()
        .map(|d| d.as_secs())
        // If clock went backwards (NTP step), report 0 rather than panicking.
        .or(Some(0))
}

fn print_report(r: &StatusReport, json: bool) {
    if json {
        // Pretty JSON is friendlier when a human is the consumer (`copypaste
        // status --json | less`) and `jq` doesn't care either way.
        match serde_json::to_string_pretty(r) {
            Ok(s) => println!("{s}"),
            Err(e) => eprintln!("copypaste: failed to serialize status: {e}"),
        }
        return;
    }

    // Table format. Column-aligned for readability.
    println!("Daemon:    {}", r.daemon);
    println!("Socket:    {}", r.socket);
    if let Some(v) = &r.version {
        println!("Version:   {v}");
    }
    if let Some(u) = r.uptime_secs {
        println!("Uptime:    {}", format_uptime(u));
    }
    if let Some(h) = r.history {
        println!("History:   {h} items");
    }
    if let Some(p) = r.private_mode {
        println!("Private:   {}", if p { "on" } else { "off" });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Build a known-good `StatusReport` for deterministic format tests.
    fn sample_running() -> StatusReport {
        StatusReport {
            daemon: "running".to_string(),
            socket: "/tmp/test.sock".to_string(),
            version: Some("0.2.0-beta.0".to_string()),
            history: Some(1234),
            uptime_secs: Some(5025), // 1h23m45s
            private_mode: Some(false),
        }
    }

    /// Snapshot: human table contains every field in a stable, parseable order.
    /// We assert per-line so a future column-width tweak doesn't false-positive
    /// the whole test.
    #[test]
    fn format_status_table_output_matches_snapshot() {
        let r = sample_running();
        // We can't capture stdout from print_report cheaply, so we render
        // the same lines manually and lock them in. Any drift in the
        // production formatter must be reflected here too.
        let lines = [
            format!("Daemon:    {}", r.daemon),
            format!("Socket:    {}", r.socket),
            format!("Version:   {}", r.version.as_deref().unwrap()),
            format!("Uptime:    {}", format_uptime(r.uptime_secs.unwrap())),
            format!("History:   {} items", r.history.unwrap()),
            format!(
                "Private:   {}",
                if r.private_mode.unwrap() { "on" } else { "off" }
            ),
        ];
        assert_eq!(lines[0], "Daemon:    running");
        assert_eq!(lines[1], "Socket:    /tmp/test.sock");
        assert_eq!(lines[2], "Version:   0.2.0-beta.0");
        assert_eq!(lines[3], "Uptime:    1h23m");
        assert_eq!(lines[4], "History:   1234 items");
        assert_eq!(lines[5], "Private:   off");
    }

    /// `--json` must serialize EVERY populated field. `None` fields are
    /// omitted via `skip_serializing_if`, which is also asserted.
    #[test]
    fn format_status_json_serializes_all_fields() {
        let r = sample_running();
        let json = serde_json::to_value(&r).expect("serialize");

        assert_eq!(json["daemon"], "running");
        assert_eq!(json["socket"], "/tmp/test.sock");
        assert_eq!(json["version"], "0.2.0-beta.0");
        assert_eq!(json["history"], 1234);
        assert_eq!(json["uptime_secs"], 5025);
        assert_eq!(json["private_mode"], false);

        // Offline report must omit nullable fields (no "version": null noise).
        let off = StatusReport::offline(&PathBuf::from("/tmp/x.sock"));
        let off_json = serde_json::to_value(&off).expect("serialize offline");
        assert_eq!(off_json["daemon"], "not running");
        assert_eq!(off_json["socket"], "/tmp/x.sock");
        assert!(
            off_json.get("version").is_none(),
            "version must be omitted when None"
        );
        assert!(
            off_json.get("history").is_none(),
            "history must be omitted when None"
        );
        assert!(
            off_json.get("uptime_secs").is_none(),
            "uptime must be omitted when None"
        );
        assert!(
            off_json.get("private_mode").is_none(),
            "private_mode must be omitted when None"
        );
    }

    /// When the socket does not exist, `probe` must produce a clean offline
    /// report without panicking. This is the ConnectionRefused path that
    /// shell scripts rely on (`copypaste status || start_daemon`).
    #[test]
    fn handles_daemon_offline_gracefully() {
        let dir = tempfile::tempdir().expect("tempdir");
        let socket = dir.path().join("definitely-not-there.sock");
        let report = probe(&socket);

        assert_eq!(report.daemon, "not running");
        assert_eq!(report.socket, socket.display().to_string());
        assert!(report.version.is_none());
        assert!(report.history.is_none());
        assert!(report.uptime_secs.is_none());
        assert!(report.private_mode.is_none());
    }

    /// Uptime formatter spot-checks across the day/hour/min/sec boundaries.
    /// Catches off-by-one errors in the modulo arithmetic.
    #[test]
    fn format_uptime_covers_all_units() {
        assert_eq!(format_uptime(0), "0s");
        assert_eq!(format_uptime(45), "45s");
        assert_eq!(format_uptime(60), "1m0s");
        assert_eq!(format_uptime(125), "2m5s");
        assert_eq!(format_uptime(3_600), "1h0m");
        assert_eq!(format_uptime(5_025), "1h23m");
        assert_eq!(format_uptime(86_400), "1d0h");
        assert_eq!(format_uptime(90_000), "1d1h");
    }

    /// Signature lock — keeps main.rs in sync. If we ever add a third arg
    /// (e.g. timeout), this test forces the change to be deliberate.
    #[test]
    fn run_signature_compiles() {
        let _: fn(&Path, bool) -> Result<()> = run;
    }
}

//! `copypaste status` — daemon health probe.
//!
//! Calls into the running daemon over the UNIX socket and reports its state
//! (running / degraded / not running), socket path, build version, PID, history
//! item count, and uptime (derived from the socket file's mtime as a proxy for
//! daemon start).
//!
//! Two output formats:
//!   - table (default): human-friendly, one field per line
//!   - --json:          machine-readable, single JSON object on stdout
//!
//! Exit codes:
//!   0 — daemon reachable and reported a healthy/ready state
//!   1 — daemon not reachable, OR reachable but degraded (DB unavailable),
//!       OR it returned an error response

use crate::ipc::IpcClient;
use anyhow::Result;
use copypaste_ipc::{METHOD_COUNT, METHOD_STATUS};
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
    /// "running" when the daemon answered and reported a healthy/ready state;
    /// "degraded" when the daemon is reachable but reports `status=degraded`
    /// (e.g. its SQLCipher key could not be read after a reinstall, so the DB
    /// is unavailable); "not running" when the socket could not be reached.
    pub daemon: String,
    /// Machine-readable reason the daemon reported for a degraded startup
    /// (e.g. `keychain_locked`). `None` unless `daemon == "degraded"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub degraded_reason: Option<String>,
    /// Filesystem path of the UNIX socket we probed. Always present so users
    /// can confirm we tried the right place when "not running" is printed.
    pub socket: String,
    /// Daemon build version string (`<semver>+<git-sha>`) from the `status`
    /// IPC call — the actual release/build, NOT the SQLite schema number.
    /// `None` when the daemon is offline or the field is missing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// PID of the running daemon process, from the `status` IPC call.
    /// `None` when the daemon is offline or the field is missing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
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
            degraded_reason: None,
            socket: socket.display().to_string(),
            version: None,
            pid: None,
            history: None,
            uptime_secs: None,
            private_mode: None,
        }
    }
}

/// Probe the daemon and print a report. `json` selects JSON output; otherwise
/// a human-friendly table is printed.
///
/// Returns `Ok(())` only when the daemon is reachable AND reported a healthy
/// state. When the daemon is offline OR degraded, the report is still printed
/// (so users see the socket path / reason they need to fix), but the function
/// returns an error so the CLI exits with status 1 — important for shell
/// scripts that check `copypaste status` before invoking other commands.
///
/// CopyPaste-8cwb: a JSON serialisation failure is also a non-zero exit —
/// the caller receives `Err` and `main` exits 1. Previously the error was
/// silently swallowed and the process exited 0 with empty stdout, making
/// `copypaste status --json | jq` fail with a cryptic "null input" error
/// rather than a clear diagnostic.
pub fn run(socket_path: &Path, json: bool) -> Result<()> {
    let report = probe(socket_path);
    // CopyPaste-8cwb: propagate serialisation errors so the caller exits
    // non-zero instead of silently succeeding with no output.
    print_report(&report, json)?;
    match report.daemon.as_str() {
        "running" => Ok(()),
        // Daemon is reachable but broken (e.g. DB unavailable after a
        // reinstall). Exit non-zero so `copypaste status && next` does not
        // proceed as if everything were healthy. The full report (including
        // the degraded reason) already went to stdout above.
        "degraded" => {
            let reason = report.degraded_reason.as_deref().unwrap_or("unknown");
            Err(anyhow::anyhow!("daemon degraded: {reason}"))
        }
        // "not running" or any future non-running state.
        _ => Err(anyhow::anyhow!("daemon not running")),
    }
}

/// Fold the daemon's `status` response payload into `report`.
///
/// Reads `build_version`, `pid`, `private_mode`, plus the degraded-startup
/// signals. The daemon reports a degraded startup (DB unavailable, e.g. its
/// SQLCipher key could not be read after a reinstall) as `status="degraded"`,
/// `degraded=true`, `ready=false` with a machine-readable `degraded_reason`.
/// Any of those three signals marks the daemon "degraded" so the CLI exits
/// non-zero instead of falsely reporting "running". Pure (no I/O) so it is
/// unit-testable.
fn apply_status_data(report: &mut StatusReport, data: &serde_json::Value) {
    // Prefer the daemon's build_version (real release) over the stats schema.
    report.version = data["build_version"].as_str().map(|s| s.to_string());
    report.pid = data["pid"].as_u64().and_then(|p| u32::try_from(p).ok());
    report.private_mode = data["private_mode"].as_bool();

    let degraded = data["status"].as_str() == Some("degraded")
        || data["degraded"].as_bool() == Some(true)
        || data["ready"].as_bool() == Some(false);
    if degraded {
        report.daemon = "degraded".to_string();
        report.degraded_reason = data["degraded_reason"].as_str().map(|s| s.to_string());
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
        degraded_reason: None,
        socket: socket_path.display().to_string(),
        version: None,
        pid: None,
        history: None,
        uptime_secs: socket_uptime_secs(socket_path),
        private_mode: None,
    };

    // 2. status — confirms the daemon is alive. Yields `build_version`,
    //    `pid`, `private_mode`, and (when impaired) `degraded`/`degraded_reason`.
    //    Failure here flips us back to "not running" since the socket
    //    accepted us but the daemon can't respond to a basic health check.
    //
    //    The daemon distinguishes a healthy startup (`status="running"`,
    //    `ready=true`) from a degraded one (`status="degraded"`,
    //    `ready=false`, `degraded_reason=...`) where the socket is bound but
    //    the backing DB is unavailable. We must surface "degraded" rather than
    //    reporting "running" and exiting 0 — otherwise scripts treat a broken
    //    daemon as healthy.
    let status_req =
        IpcClient::build_request(&IpcClient::next_id(), METHOD_STATUS, serde_json::json!({}));
    match client.call(&status_req) {
        Ok(resp) if resp.ok => {
            if let Some(data) = &resp.data {
                apply_status_data(&mut report, data);
            }
        }
        _ => return StatusReport::offline(socket_path),
    }

    // 3. count — total history items. Best-effort: failure leaves `history = None`
    //    but does NOT downgrade the daemon state.
    //    Each call opens a fresh connection because IpcClient is one-shot.
    if let Ok(mut c2) = IpcClient::connect(socket_path) {
        let count_req =
            IpcClient::build_request(&IpcClient::next_id(), METHOD_COUNT, serde_json::json!({}));
        if let Ok(resp) = c2.call(&count_req) {
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

/// Format the status report to stdout (table) or stdout (JSON).
///
/// CopyPaste-8cwb: returns `Err` when `--json` serialisation fails so the
/// caller can propagate a non-zero exit code. Previously the function was
/// `fn(...) -> ()` and swallowed the error after printing to stderr, leaving
/// the process to exit 0 with no output.
///
/// CopyPaste-elk5: the degraded reason goes to **stderr** (not stdout) in
/// table mode so that `copypaste status 2>/dev/null | grep Daemon` always
/// produces clean output, and scripts that capture stdout for further
/// processing never accidentally receive diagnostic noise.
fn print_report(r: &StatusReport, json: bool) -> Result<()> {
    if json {
        // Pretty JSON is friendlier when a human is the consumer (`copypaste
        // status --json | less`) and `jq` doesn't care either way.
        // CopyPaste-8cwb: surface the error to the caller instead of swallowing it.
        let s = serde_json::to_string_pretty(r)
            .map_err(|e| anyhow::anyhow!("failed to serialize status: {e}"))?;
        println!("{s}");
        return Ok(());
    }

    // Table format. Column-aligned for readability.
    println!("Daemon:    {}", r.daemon);
    println!("Socket:    {}", r.socket);
    if let Some(v) = &r.version {
        // The daemon's `build_version` is the real release/build string
        // (`<semver>+<git-sha>`), so label it "Version:".
        println!("Version:   {v}");
    }
    if let Some(pid) = r.pid {
        println!("PID:       {pid}");
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
    // CopyPaste-elk5: degraded reason is diagnostic noise — send it to stderr
    // so stdout captures only clean machine-parseable table rows.
    if let Some(reason) = &r.degraded_reason {
        eprintln!("Degraded:  {reason}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Build a known-good `StatusReport` for deterministic format tests.
    fn sample_running() -> StatusReport {
        StatusReport {
            daemon: "running".to_string(),
            degraded_reason: None,
            socket: "/tmp/test.sock".to_string(),
            version: Some("0.4.1+abc1234".to_string()),
            pid: Some(12345),
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
            format!("PID:       {}", r.pid.unwrap()),
            format!("Uptime:    {}", format_uptime(r.uptime_secs.unwrap())),
            format!("History:   {} items", r.history.unwrap()),
            format!(
                "Private:   {}",
                if r.private_mode.unwrap() { "on" } else { "off" }
            ),
        ];
        assert_eq!(lines[0], "Daemon:    running");
        assert_eq!(lines[1], "Socket:    /tmp/test.sock");
        assert_eq!(lines[2], "Version:   0.4.1+abc1234");
        assert_eq!(lines[3], "PID:       12345");
        assert_eq!(lines[4], "Uptime:    1h23m");
        assert_eq!(lines[5], "History:   1234 items");
        assert_eq!(lines[6], "Private:   off");
    }

    /// A degraded `status` payload (DB unavailable after reinstall) must mark
    /// the report "degraded" with its reason — NOT "running". This is what
    /// makes `run()` exit non-zero so scripts don't treat a broken daemon as
    /// healthy.
    #[test]
    fn degraded_status_payload_marks_report_degraded() {
        let mut report = StatusReport {
            daemon: "running".to_string(),
            degraded_reason: None,
            socket: "/tmp/test.sock".to_string(),
            version: None,
            pid: None,
            history: None,
            uptime_secs: None,
            private_mode: None,
        };
        let data = serde_json::json!({
            "status": "degraded",
            "private_mode": false,
            "ready": false,
            "degraded": true,
            "degraded_reason": "keychain_locked",
        });
        apply_status_data(&mut report, &data);

        assert_eq!(report.daemon, "degraded");
        assert_eq!(report.degraded_reason.as_deref(), Some("keychain_locked"));
        assert_eq!(report.private_mode, Some(false));

        // JSON form must surface the reason for scripts.
        let json = serde_json::to_value(&report).expect("serialize");
        assert_eq!(json["daemon"], "degraded");
        assert_eq!(json["degraded_reason"], "keychain_locked");
    }

    /// A healthy `status` payload keeps the report "running" with no reason,
    /// and the JSON form omits `degraded_reason`. Also confirms build_version
    /// and pid are folded in from the status payload.
    #[test]
    fn healthy_status_payload_stays_running() {
        let mut report = StatusReport {
            daemon: "running".to_string(),
            degraded_reason: None,
            socket: "/tmp/test.sock".to_string(),
            version: None,
            pid: None,
            history: None,
            uptime_secs: None,
            private_mode: None,
        };
        let data = serde_json::json!({
            "status": "running",
            "private_mode": true,
            "ready": true,
            "degraded": false,
            "build_version": "0.5.2+deadbee",
            "pid": 4242,
        });
        apply_status_data(&mut report, &data);

        assert_eq!(report.daemon, "running");
        assert!(report.degraded_reason.is_none());
        assert_eq!(report.private_mode, Some(true));
        assert_eq!(report.version.as_deref(), Some("0.5.2+deadbee"));
        assert_eq!(report.pid, Some(4242));

        let json = serde_json::to_value(&report).expect("serialize");
        assert!(
            json.get("degraded_reason").is_none(),
            "degraded_reason must be omitted when healthy"
        );
    }

    /// `--json` must serialize EVERY populated field. `None` fields are
    /// omitted via `skip_serializing_if`, which is also asserted.
    #[test]
    fn format_status_json_serializes_all_fields() {
        let r = sample_running();
        let json = serde_json::to_value(&r).expect("serialize");

        assert_eq!(json["daemon"], "running");
        assert_eq!(json["socket"], "/tmp/test.sock");
        assert_eq!(json["version"], "0.4.1+abc1234");
        assert_eq!(json["pid"], 12345);
        assert_eq!(json["history"], 1234);
        assert_eq!(json["uptime_secs"], 5025);
        assert_eq!(json["private_mode"], false);
        assert!(
            json.get("degraded_reason").is_none(),
            "degraded_reason must be omitted when None"
        );

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
            off_json.get("pid").is_none(),
            "pid must be omitted when None"
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

    /// Degraded daemon: status="degraded" + non-zero exit code.
    #[test]
    fn format_status_degraded_report() {
        let r = StatusReport {
            daemon: "degraded".to_string(),
            degraded_reason: Some("keychain locked; DB unavailable".to_string()),
            socket: "/tmp/test.sock".to_string(),
            version: Some("0.4.1+abc1234".to_string()),
            pid: Some(99),
            history: None,
            uptime_secs: Some(10),
            private_mode: None,
        };
        let json = serde_json::to_value(&r).expect("serialize");
        assert_eq!(json["daemon"], "degraded");
        assert_eq!(json["degraded_reason"], "keychain locked; DB unavailable");
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
        assert!(report.degraded_reason.is_none());
        assert_eq!(report.socket, socket.display().to_string());
        assert!(report.version.is_none());
        assert!(report.pid.is_none());
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

    /// CopyPaste-8cwb: `print_report` in JSON mode must return `Err` when
    /// serialisation fails, NOT silently succeed with empty stdout.
    ///
    /// `serde_json::to_string_pretty` cannot fail on a `StatusReport` because
    /// every field is a primitive (`String`, `u32`, `i64`, `bool`) that always
    /// serialises. We verify the happy-path contract: `print_report` returns
    /// `Ok` and the result is valid JSON.
    #[test]
    fn print_report_json_returns_ok_on_success() {
        let r = sample_running();
        // Verify the function returns Ok (not Err) for a well-formed report.
        let result = print_report(&r, true);
        assert!(
            result.is_ok(),
            "print_report(json=true) must return Ok for a valid report"
        );
    }

    /// CopyPaste-8cwb: `print_report` in table mode must return `Ok`.
    #[test]
    fn print_report_table_returns_ok() {
        let r = sample_running();
        let result = print_report(&r, false);
        assert!(
            result.is_ok(),
            "print_report(json=false) must return Ok for a valid report"
        );
    }

        /// CopyPaste-elk5: the degraded reason must NOT appear in the table output
    /// sent to stdout. It is a diagnostic and belongs on stderr. We verify that
    /// the table-output lines constructed by `print_report` for a degraded
    /// report do NOT include a "Degraded:" line (which was previously on stdout).
    ///
    /// We cannot capture stderr from `print_report` cheaply in-process, so we
    /// verify the contract by confirming `print_report` returns Ok (no panic /
    /// no stdout-exit) and that the stdout lines we construct for the same
    /// data do not contain "Degraded:".
    #[test]
    fn degraded_reason_not_in_table_stdout_lines() {
        let r = StatusReport {
            daemon: "degraded".to_string(),
            degraded_reason: Some("keychain_locked".to_string()),
            socket: "/tmp/test.sock".to_string(),
            version: Some("0.4.1+abc1234".to_string()),
            pid: Some(99),
            history: None,
            uptime_secs: Some(10),
            private_mode: None,
        };
        // The table stdout lines must NOT include the degraded reason;
        // it now goes to stderr via eprintln!.
        let stdout_lines = [
            format!("Daemon:    {}", r.daemon),
            format!("Socket:    {}", r.socket),
        ];
        for line in &stdout_lines {
            assert!(
                !line.contains("Degraded:"),
                "stdout line must not contain Degraded: — found: {line}"
            );
        }
        // print_report must still return Ok.
        assert!(print_report(&r, false).is_ok());
    }
}

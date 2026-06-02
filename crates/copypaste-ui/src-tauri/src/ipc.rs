//! Bridge between the React frontend and the CopyPaste daemon.
//!
//! The daemon speaks newline-delimited JSON over a Unix domain socket. This
//! module exposes a single `ipc_call` Tauri command that opens a short-lived
//! connection, sends one request, and returns the parsed reply. The frontend
//! `src/lib/ipc.ts` wraps each daemon method on top of it.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

use serde_json::Value;

/// Resolve the daemon socket path, matching `copypaste-daemon::paths::socket_path`.
fn socket_path() -> PathBuf {
    if let Ok(p) = std::env::var("COPYPASTE_SOCKET") {
        return PathBuf::from(p);
    }
    // If the home directory cannot be resolved we have no way to locate the
    // daemon socket. Fall back to a path that is guaranteed not to exist (and is
    // not a real system directory) so `UnixStream::connect` fails with NotFound
    // and the frontend surfaces a clean `daemon_offline` rather than silently
    // probing `/Library/...` or `/.local/...`.
    let Some(home) = home::home_dir() else {
        return PathBuf::from("/nonexistent/copypaste/daemon.sock");
    };
    #[cfg(target_os = "macos")]
    {
        home.join("Library/Application Support/CopyPaste/daemon.sock")
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            return PathBuf::from(xdg).join("copypaste/daemon.sock");
        }
        home.join(".local/share/copypaste/daemon.sock")
    }
    #[cfg(not(unix))]
    {
        home.join("daemon.sock")
    }
}

/// JSON-RPC request id sent by all UI→daemon calls.
///
/// The wire value is intentionally fixed (not a correlation counter) because
/// each call opens a fresh short-lived connection and reads exactly one reply,
/// so there is no in-flight multiplexing that would require unique ids. If
/// per-call correlation is needed in the future, replace this with an atomic
/// counter and format it as a string (e.g. `ui-{n}`).
const IPC_REQUEST_ID: &str = "ui-1";

/// Daemon reply, mirroring the wire shape so the frontend can branch on
/// `ok` / `error_code` exactly like the daemon emits.
#[derive(serde::Serialize)]
pub struct IpcReply {
    pub ok: bool,
    pub data: Option<Value>,
    pub error: Option<String>,
    pub error_code: Option<String>,
}

/// Make a synchronous IPC call to the daemon from Rust code (e.g. the tray
/// setup path). Identical wire format to `do_call`; kept separate so the
/// name is clearly scoped to internal use.
pub(crate) fn call(method: &str, params: Value) -> Result<IpcReply, String> {
    do_call(method, params)
}

fn do_call(method: &str, params: Value) -> Result<IpcReply, String> {
    let path = socket_path();
    let stream = UnixStream::connect(&path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused => {
            format!("daemon_offline:{}", path.display())
        }
        _ => format!("io_error:{e}"),
    })?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| format!("io_error:{e}"))?;

    let req = serde_json::json!({ "id": IPC_REQUEST_ID, "method": method, "params": params });
    let mut line = serde_json::to_string(&req).map_err(|e| e.to_string())?;
    line.push('\n');
    (&stream)
        .write_all(line.as_bytes())
        .map_err(|e| format!("io_error:{e}"))?;

    let mut reader = BufReader::new(&stream);
    let mut resp = String::new();
    reader
        .read_line(&mut resp)
        .map_err(|e| format!("io_error:{e}"))?;
    let resp = resp.trim();
    if resp.is_empty() {
        return Err("daemon closed connection without response".into());
    }
    let v: Value = serde_json::from_str(resp).map_err(|e| format!("invalid_json:{e}"))?;
    Ok(IpcReply {
        ok: v["ok"].as_bool().unwrap_or(false),
        data: if v["data"].is_null() {
            None
        } else {
            Some(v["data"].clone())
        },
        error: v["error"].as_str().map(str::to_owned),
        error_code: v["error_code"].as_str().map(str::to_owned),
    })
}

/// Send one JSON-RPC request to the daemon and return the parsed reply.
///
/// The underlying socket IO is blocking, so we offload it to a blocking thread
/// pool via `spawn_blocking` rather than running it inline. An `async` Tauri
/// command is driven on the async runtime; doing blocking `UnixStream` reads
/// directly there would stall the executor (and, with the default scheduler,
/// every other in-flight command) until the daemon replies or the 10s read
/// timeout elapses. Offloading keeps the UI responsive.
#[tauri::command]
pub async fn ipc_call(method: String, params: Option<Value>) -> Result<IpcReply, String> {
    tauri::async_runtime::spawn_blocking(move || call(&method, params.unwrap_or(Value::Null)))
        .await
        .map_err(|e| format!("ipc_call task join error: {e}"))?
}

/// Wipe and recreate the daemon's clipboard database (destructive recovery).
///
/// This is the backend for the desktop UI's "Reset database" button — the
/// explicit escape hatch the user invokes when the daemon is stuck in DEGRADED
/// mode because the existing database cannot be decrypted. It sends the daemon's
/// `reset_database` IPC method with `confirm = true` (the daemon refuses the
/// call without it) and returns the parsed reply. The daemon recovers IN-PLACE
/// on success, so the caller should re-fetch `status` / `history_page`
/// afterwards — no daemon restart is needed.
///
/// Like [`ipc_call`], the underlying socket IO is blocking and is therefore
/// offloaded to a blocking thread to avoid stalling the async runtime.
#[tauri::command]
pub async fn reset_database() -> Result<IpcReply, String> {
    tauri::async_runtime::spawn_blocking(|| {
        call("reset_database", serde_json::json!({ "confirm": true }))
    })
    .await
    .map_err(|e| format!("reset_database task join error: {e}"))?
}

/// Result of [`pairing_qr_svg`]: an inline SVG of the pairing QR plus metadata.
#[derive(serde::Serialize)]
pub struct PairingQr {
    /// Inline SVG markup of the QR code (drop straight into an `<img>` via a
    /// data URI, or `dangerouslySetInnerHTML`).
    pub svg: String,
    /// The raw `CPPAIR1.…` payload string (shown as a fallback / copy target).
    pub payload: String,
    /// Seconds until the embedded pairing token expires.
    pub expires_in_secs: u64,
}

/// Generate a scannable pairing QR for this device.
///
/// Asks the daemon (`pair_generate_qr`) for a fresh pairing payload — this
/// device's fingerprint plus a single-use, short-lived token — and renders it
/// as an inline SVG QR code other devices scan to pair automatically. The QR is
/// purely a transport for the existing PAKE pairing material; no new crypto is
/// introduced (see `copypaste_core::crypto::pairing_qr`).
#[tauri::command]
pub async fn pairing_qr_svg() -> Result<PairingQr, String> {
    // Same rationale as `ipc_call`: the daemon round-trip is blocking IO, so run
    // it off the async runtime to avoid stalling the executor.
    let reply = tauri::async_runtime::spawn_blocking(|| call("pair_generate_qr", Value::Null))
        .await
        .map_err(|e| format!("pairing_qr_svg task join error: {e}"))??;
    if !reply.ok {
        return Err(reply
            .error
            .unwrap_or_else(|| "pair_generate_qr failed".to_string()));
    }
    let data = reply
        .data
        .ok_or_else(|| "daemon returned no data for pair_generate_qr".to_string())?;
    let payload = data["qr"]
        .as_str()
        .ok_or_else(|| "daemon response missing 'qr' field".to_string())?
        .to_string();
    let expires_in_secs = data["expires_in_secs"].as_u64().unwrap_or(0);

    let svg = render_svg(&payload)?;
    Ok(PairingQr {
        svg,
        payload,
        expires_in_secs,
    })
}

/// Read the most recent daemon log file and return up to `max_lines` trailing
/// lines. Returns an empty string if no log files are found.
///
/// **Note:** only the single most-recent log file (by filename, descending sort
/// of `daemon.YYYY-MM-DD.log`) is read. Older rotated files are not included.
/// If the daemon rolled over at midnight the tail of the previous day's log is
/// not returned. This is intentional for simplicity; a future improvement could
/// read across rotation boundaries when `max_lines` is not yet satisfied.
#[tauri::command]
pub async fn read_logs(max_lines: usize) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let log_dir = {
            #[cfg(target_os = "macos")]
            {
                home::home_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
                    .join("Library/Logs/CopyPaste")
            }
            #[cfg(not(target_os = "macos"))]
            {
                std::env::temp_dir().join("copypaste-logs")
            }
        };

        let read_dir = std::fs::read_dir(&log_dir)
            .map_err(|e| format!("cannot read log dir {}: {e}", log_dir.display()))?;

        let mut entries: Vec<std::fs::DirEntry> = read_dir
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name();
                let s = name.to_string_lossy();
                s.starts_with("daemon") && s.ends_with(".log")
            })
            .collect();

        // Sort descending by filename (daily rotation: daemon.YYYY-MM-DD.log).
        entries.sort_by_key(|e| std::cmp::Reverse(e.file_name()));

        let Some(entry) = entries.first() else {
            return Ok(String::new());
        };

        let content = std::fs::read_to_string(entry.path())
            .map_err(|e| format!("cannot read log file: {e}"))?;

        let all_lines: Vec<&str> = content.lines().collect();
        let start = all_lines.len().saturating_sub(max_lines);
        Ok(all_lines[start..].join("\n"))
    })
    .await
    .map_err(|e| format!("read_logs task join error: {e}"))?
}

/// Return the path of the daemon log directory.
#[tauri::command]
pub fn log_dir_path() -> String {
    #[cfg(target_os = "macos")]
    {
        home::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
            .join("Library/Logs/CopyPaste")
            .to_string_lossy()
            .into_owned()
    }
    #[cfg(not(target_os = "macos"))]
    {
        std::env::temp_dir()
            .join("copypaste-logs")
            .to_string_lossy()
            .into_owned()
    }
}

/// Render `payload` as an inline SVG QR code string.
fn render_svg(payload: &str) -> Result<String, String> {
    use qrcode::render::svg;
    use qrcode::{EcLevel, QrCode};

    let code = QrCode::with_error_correction_level(payload, EcLevel::M)
        .map_err(|e| format!("qr_build_failed:{e}"))?;
    let svg = code
        .render::<svg::Color<'_>>()
        .min_dimensions(220, 220)
        .quiet_zone(true)
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .build();
    Ok(svg)
}

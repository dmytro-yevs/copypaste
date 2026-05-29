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
    let home = home::home_dir().unwrap_or_else(|| PathBuf::from("/"));
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

    let req = serde_json::json!({ "id": "ui-1", "method": method, "params": params });
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
#[tauri::command]
pub fn ipc_call(method: String, params: Option<Value>) -> Result<IpcReply, String> {
    call(&method, params.unwrap_or(Value::Null))
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
pub fn pairing_qr_svg() -> Result<PairingQr, String> {
    let reply = call("pair_generate_qr", Value::Null)?;
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

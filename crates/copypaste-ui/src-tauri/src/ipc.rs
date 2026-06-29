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

use copypaste_ipc::{METHOD_GET_ITEM_FILE, METHOD_PAIR_GENERATE_QR, METHOD_RESET_DATABASE};
use serde_json::Value;

/// Resolve the daemon socket path, matching `copypaste-daemon::paths::socket_path`.
pub(crate) fn socket_path() -> PathBuf {
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
    /// Wire protocol version the daemon speaks (ADR-007). Forwarded verbatim
    /// from the daemon's JSON reply so the frontend `protocolMismatchHandler`
    /// in `src/lib/ipc.ts` can fire. `None` when the daemon predates this field.
    pub protocol_version: Option<u32>,
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

    let req = serde_json::json!({
        "id": IPC_REQUEST_ID,
        "method": method,
        "params": params,
        // ADR-007: tell the daemon which wire version the UI was compiled against.
        // The daemon echoes back its own version in the reply; the frontend compares
        // the two and fires `protocolMismatchHandler` when they diverge.
        "protocol_version": 1u32,
    });
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
        // ADR-007: forward the daemon's wire protocol version to the frontend.
        // Cast via u64 first (serde_json stores JSON numbers as u64/i64) then
        // narrow to u32 — any daemon value that overflows u32 is treated as absent.
        protocol_version: v["protocol_version"]
            .as_u64()
            .and_then(|n| u32::try_from(n).ok()),
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
        call(
            METHOD_RESET_DATABASE,
            serde_json::json!({ "confirm": true }),
        )
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
    let reply = tauri::async_runtime::spawn_blocking(|| call(METHOD_PAIR_GENERATE_QR, Value::Null))
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

/// Classify a file extension as executable/script (dangerous) or safe to open directly.
///
/// # Security context
/// File items can arrive from a PAIRED PEER via P2P/relay sync.  The peer
/// controls the filename (and therefore the extension) stored by the daemon.
/// A malicious peer could send a file named `evil.command`, `evil.sh`, or
/// `evil.app` — when the local user clicks "Open", macOS would execute the
/// payload directly without any further prompt.  We therefore block direct
/// `open` for all executable/script/bundle extensions and instead reveal the
/// file in Finder (`open -R`) so the user must consciously decide what to do.
fn is_dangerous_extension(ext: &str) -> bool {
    // Explicit denylist of macOS/Unix/Windows executable and script extensions.
    // Err on the side of caution: any extension not in the SAFE list below
    // should be treated as potentially dangerous.  Add here whenever a new
    // executable type becomes relevant — never remove without security review.
    matches!(
        ext.to_ascii_lowercase().as_str(),
        // macOS-specific execution vectors
        |"app"| "action" | "workflow" | "definition"
        | "scpt" | "scptd" | "applescript"
        | "terminal" | "command" | "tool"
        // Shell scripts
        | "sh" | "bash" | "zsh" | "csh" | "fish" | "ksh"
        // Interpreted languages
        | "py" | "rb" | "pl" | "php" | "lua" | "tcl" | "r"
        // JavaScript (node / browser)
        | "js" | "mjs" | "cjs"
        // JVM
        | "jar" | "class"
        // Windows executables / scripts (not primary target but included for safety)
        | "exe" | "bat" | "cmd" | "com" | "msi" | "ps1"
        | "vb" | "vbs" | "ws" | "wsf" | "wsh" | "scr"
        // Native libraries that can be injected
        | "dylib" | "so" | "dll"
        // CopyPaste-crh3.73: kept in parity with the canonical denylist in
        // copypaste-core/src/filename_security.rs. The architecture boundary
        // (the Tauri shell never links copypaste-core) forces this duplication,
        // so these trailing groups had drifted out — they are restored here.
        // Android package (APK) — dangerous on Android, included for parity
        | "apk"
        // Web/scripting vectors
        | "html" | "htm" | "jse"
        // Registry / shortcut (Windows)
        | "reg" | "lnk"
        // Package installers
        | "dmg" | "pkg"
    )
}

/// Sanitise a peer-supplied filename for safe materialisation on disk.
///
/// Strips everything except alphanumerics, dots, dashes, underscores, and
/// spaces, and removes control characters.  Path separators are already
/// stripped by the `file_name()` call at the call site; this is an additional
/// layer that prevents other shell-special characters from appearing in the
/// temp-file name.
fn sanitize_filename(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '.' | '-' | '_' | ' '))
        .collect();
    if sanitized.is_empty() {
        "clipboard_file".to_string()
    } else {
        sanitized
    }
}

/// How long (seconds) to keep the per-item temp directory alive after the OS
/// open call returns.  30 s is enough for any application to finish reading
/// the file from disk; after that the subdir and its single file are removed.
const OPEN_ITEM_CLEANUP_DELAY_SECS: u64 = 30;

/// Open a file-type clipboard item with the OS default application.
///
/// Fetches the file bytes from the daemon (`get_item_file`), writes them to a
/// **per-item UUID subdirectory** under `$TMPDIR/copypaste_open/<uuid>/`, then
/// opens the file with the OS default application:
///   - macOS: `/usr/bin/open <path>`
///   - Linux: `xdg-open <path>`
///   - Windows: `cmd /c start "" <path>` (not the primary target; included for
///     completeness).
///
/// # Cleanup (n7qv)
/// Each call writes to a fresh `<uuid>/` subdirectory so concurrent opens
/// never collide.  A background thread removes the entire subdirectory after
/// [`OPEN_ITEM_CLEANUP_DELAY_SECS`] seconds — long enough for the OS and the
/// launched application to finish reading the file, but short enough that
/// decrypted content does not linger in `$TMPDIR` indefinitely.
///
/// # Security: dangerous extension blocking
/// Files with executable or script extensions (`.sh`, `.command`, `.app`, etc.)
/// are NOT opened directly. Instead, we reveal them in Finder (`open -R`) so
/// the user must consciously act on the file.  See [`is_dangerous_extension`]
/// and [`sanitize_filename`] for details.
///
/// On error the function returns an `Err(String)` that the frontend surfaces as
/// a toast.
#[tauri::command]
pub async fn open_item_file(id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        // 1. Fetch file data from daemon.
        let reply = call(METHOD_GET_ITEM_FILE, serde_json::json!({ "id": id }))
            .map_err(|e| format!("IPC error: {e}"))?;
        if !reply.ok {
            return Err(reply.error.unwrap_or_else(|| "get_item_file failed".into()));
        }
        let data = reply
            .data
            .ok_or_else(|| "daemon returned no data for get_item_file".to_string())?;

        let filename = data["filename"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or("clipboard_file")
            .to_string();
        let data_b64 = data["data_b64"]
            .as_str()
            .ok_or_else(|| "get_item_file response missing data_b64".to_string())?;

        // 2. Decode base64.
        use base64::Engine as _;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data_b64)
            .map_err(|e| format!("base64 decode error: {e}"))?;

        // 3. Write to a per-item UUID subdirectory so concurrent opens never
        //    collide and each call has an isolated cleanup target (n7qv).
        let item_uuid = uuid::Uuid::new_v4();
        let item_dir = std::env::temp_dir()
            .join("copypaste_open")
            .join(item_uuid.to_string());
        std::fs::create_dir_all(&item_dir).map_err(|e| format!("create temp dir failed: {e}"))?;

        // Sanitise the filename: strip path separators so a malicious filename
        // cannot escape the temp directory (defence-in-depth; daemon should
        // never send such filenames, but we guard at the boundary here too).
        // Additionally sanitize to alphanumerics/.-_ space to prevent
        // shell-special characters in the temp-file path (peer-supplied name).
        let base_name = std::path::Path::new(&filename)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("clipboard_file")
            .to_string();
        let safe_name = sanitize_filename(&base_name);

        // 4. Check extension before writing — avoid materialising executable
        //    content at an OS-openable path if we won't directly open it.
        let extension = std::path::Path::new(&safe_name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();
        let dangerous = is_dangerous_extension(&extension);

        let tmp_path = item_dir.join(&safe_name);
        std::fs::write(&tmp_path, &bytes).map_err(|e| format!("write temp file failed: {e}"))?;

        // 5. Open with OS default app, or reveal in Finder for dangerous types.
        #[cfg(target_os = "macos")]
        {
            if dangerous {
                // Reveal in Finder instead of executing — the user can inspect
                // and decide. This prevents one-click code execution from a
                // peer-supplied file with an executable extension.
                std::process::Command::new("/usr/bin/open")
                    .arg("-R")
                    .arg(&tmp_path)
                    .spawn()
                    .map_err(|e| format!("open -R command failed: {e}"))?;
            } else {
                std::process::Command::new("/usr/bin/open")
                    .arg(&tmp_path)
                    .spawn()
                    .map_err(|e| format!("open command failed: {e}"))?;
            }
        }
        #[cfg(target_os = "linux")]
        {
            if dangerous {
                return Err(format!(
                    "File type '.{extension}' is blocked for direct opening. \
                     Find the file at: {}",
                    tmp_path.display()
                ));
            }
            std::process::Command::new("xdg-open")
                .arg(&tmp_path)
                .spawn()
                .map_err(|e| format!("open command failed: {e}"))?;
        }
        #[cfg(windows)]
        {
            if dangerous {
                return Err(format!(
                    "File type '.{extension}' is blocked for direct opening. \
                     Find the file at: {}",
                    tmp_path.display()
                ));
            }
            std::process::Command::new("cmd")
                .args(["/c", "start", "", &tmp_path.to_string_lossy()])
                .spawn()
                .map_err(|e| format!("open command failed: {e}"))?;
        }

        // 6. Schedule cleanup: remove the per-item subdir after a short delay
        //    so decrypted content does not linger in $TMPDIR (n7qv).  We spawn
        //    a detached OS thread (not a tokio task) so the cleanup runs even
        //    if the async runtime shuts down before the timer fires.  A failure
        //    to remove the dir (e.g. the OS already cleaned it up) is benign.
        let cleanup_dir = item_dir.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(OPEN_ITEM_CLEANUP_DELAY_SECS));
            let _ = std::fs::remove_dir_all(&cleanup_dir);
        });

        Ok(())
    })
    .await
    .map_err(|e| format!("open_item_file task join error: {e}"))?
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

#[cfg(test)]
mod dangerous_extension_tests {
    use super::is_dangerous_extension;

    /// CopyPaste-crh3.73: these 9 extensions had drifted out of the Tauri-shell
    /// denylist while present in the canonical copypaste-core list. They must all
    /// be flagged dangerous. Keep this denylist in parity with
    /// `copypaste-core/src/filename_security.rs::is_dangerous_extension` (the
    /// Tauri shell deliberately does not link copypaste-core, so the list is
    /// duplicated and this test guards against re-drift).
    #[test]
    fn restored_extensions_are_dangerous() {
        for ext in [
            "apk", "dmg", "html", "htm", "jse", "reg", "lnk", "pkg", "wsh",
        ] {
            assert!(
                is_dangerous_extension(ext),
                "{ext} must be treated as a dangerous extension (crh3.73 parity)"
            );
        }
    }

    /// Case-insensitive matching plus a representative sample of the rest of the
    /// denylist, with a few genuinely-safe extensions, so an accidental future
    /// deletion is caught.
    #[test]
    fn denylist_is_case_insensitive_and_excludes_safe_types() {
        for ext in ["APK", "Dmg", "EXE", "sh", "js", "py", "dylib", "scpt"] {
            assert!(is_dangerous_extension(ext), "{ext} must be dangerous");
        }
        for ext in ["txt", "png", "pdf", "md", "json"] {
            assert!(!is_dangerous_extension(ext), "{ext} must be safe");
        }
    }
}

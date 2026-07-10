//! `copypaste import <file>` — bulk-load clipboard items from a JSON export.
//!
//! Input file format:
//! ```json
//! {
//!   "items": [
//!     {
//!       "content_type": "text",
//!       "content_bytes_b64": "...",
//!       "created_at_ms": 1234567890,
//!       "metadata": null
//!     }
//!   ]
//! }
//! ```
//!
//! The daemon's `import` IPC method hashes each item and deduplicates
//! against existing rows in the last 5 minutes. The CLI prints
//! `imported: <inserted> skipped: <skipped>` on success.

use anyhow::{anyhow, Context, Result};
use copypaste_ipc::METHOD_IMPORT;
use std::path::Path;

use crate::commands::common::exit_on_err;
use crate::ipc::IpcClient;

/// Maximum import file size accepted before reading into memory.
///
/// The daemon's IPC framing caps any single request at 16 MiB
/// ([`copypaste_ipc::MAX_IPC_REQUEST_BYTES`]). Because the CLI wraps the entire file contents in a
/// single `import` JSON-RPC frame (plus `{"method":"import","params":{"items":`
/// envelope overhead), we cap the **source file** at 12 MiB — safely under
/// 16 MiB after serialisation overhead — and surface a clear error here rather
/// than letting the daemon close the connection with `request_too_large`.
///
/// If you need to import larger exports, split them into ≤12 MiB chunks and
/// import each chunk separately.
const MAX_IMPORT_FILE_BYTES: u64 = 12 * 1024 * 1024; // 12 MiB (IPC limit headroom)

/// The daemon's IPC frame limit, mirrored here for the error message.
///
/// CopyPaste-8ebg.59: derived from [`copypaste_ipc::MAX_IPC_REQUEST_BYTES`],
/// the single shared source of truth also used by the daemon's Unix-socket
/// server and the frozen Windows named-pipe skeleton — this used to be an
/// independent `16 * 1024 * 1024` literal kept in sync only by a comment.
const DAEMON_MAX_REQUEST_BYTES: u64 = copypaste_ipc::MAX_IPC_REQUEST_BYTES as u64;

pub fn run(socket_path: &Path, file: &str) -> Result<()> {
    // Pre-check file size before reading into memory.  read_to_string would
    // allocate the full file contents up-front; without this guard a multi-GB
    // file could exhaust the process's virtual address space.
    //
    // The tighter 12 MiB cap (vs. the daemon's 16 MiB IPC frame limit) also
    // prevents the daemon from rejecting the request mid-flight with a cryptic
    // connection-closed error (CopyPaste-aazu).
    let file_size = std::fs::metadata(file)
        .with_context(|| format!("failed to stat import file: {file}"))?
        .len();
    if file_size > MAX_IMPORT_FILE_BYTES {
        return Err(anyhow!(
            "import file is too large ({file_size} bytes > {MAX_IMPORT_FILE_BYTES} byte limit): {file}\n\
             The daemon's IPC frame limit is {DAEMON_MAX_REQUEST_BYTES} bytes; \
             split the export into ≤12 MiB chunks and import each separately."
        ));
    }

    let content = std::fs::read_to_string(file)
        .with_context(|| format!("failed to read import file: {file}"))?;
    let data: serde_json::Value = serde_json::from_str(&content)
        .with_context(|| format!("invalid JSON in import file: {file}"))?;

    let items = match data.get("items").and_then(|v| v.as_array()) {
        Some(a) => a.clone(),
        None => {
            return Err(anyhow!(
                "invalid format: expected {{\"items\": [...]}} (file: {file})"
            ));
        }
    };

    if items.is_empty() {
        println!("no items in {file}");
        return Ok(());
    }

    let mut client = IpcClient::connect(socket_path)?;
    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_IMPORT,
        serde_json::json!({ "items": items }),
    );
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    let data = resp
        .data
        .ok_or_else(|| anyhow!("daemon returned ok with no data"))?;
    let inserted = data.get("inserted").and_then(|v| v.as_u64()).unwrap_or(0);
    let skipped = data.get("skipped").and_then(|v| v.as_u64()).unwrap_or(0);
    println!("imported: {inserted} skipped: {skipped}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_signature_compiles() {
        let _: fn(&Path, &str) -> Result<()> = run;
    }

    /// Helper: create a sparse file of exactly `size` bytes.
    fn make_sparse_file(dir: &std::path::Path, name: &str, size: u64) -> std::path::PathBuf {
        use std::fs::OpenOptions;
        let path = dir.join(name);
        let f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        f.set_len(size).unwrap();
        path
    }

    /// A file larger than MAX_IMPORT_FILE_BYTES (12 MiB) must be rejected with
    /// a clear "too large" error before read_to_string so we never allocate the
    /// full content in memory (CopyPaste-aazu).
    #[test]
    fn oversized_file_is_rejected_before_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = make_sparse_file(dir.path(), "big.json", MAX_IMPORT_FILE_BYTES + 1);

        let sock = dir.path().join("dummy.sock"); // doesn't need to exist
        let err = run(&sock, path.to_str().unwrap()).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("too large"),
            "expected 'too large' error, got: {msg}"
        );
    }

    /// A file between 12 MiB and 16 MiB (old cap vs. new cap) is now rejected
    /// at the CLI with a clear error that mentions the IPC frame limit — it must
    /// NOT reach the daemon and get a cryptic connection-closed error.
    /// (CopyPaste-aazu: the root mismatch was 64 MiB CLI cap vs 16 MiB daemon
    /// IPC frame limit; this range previously reached the daemon and failed.)
    #[test]
    fn file_between_12_and_16_mib_is_rejected_with_helpful_message() {
        let dir = tempfile::tempdir().unwrap();
        // 13 MiB: above our 12 MiB CLI cap but below the old 64 MiB cap and
        // below the daemon's 16 MiB IPC frame limit — the old code would have
        // sent this to the daemon, which would close the connection.
        let thirteen_mib: u64 = 13 * 1024 * 1024;
        let path = make_sparse_file(dir.path(), "medium.json", thirteen_mib);

        let sock = dir.path().join("dummy.sock"); // doesn't need to exist
        let err = run(&sock, path.to_str().unwrap()).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("too large"),
            "expected 'too large' error, got: {msg}"
        );
        // The error must explain the IPC limit so users know why.
        assert!(
            msg.contains("IPC") || msg.contains("16"),
            "expected error to mention IPC limit, got: {msg}"
        );
    }

    /// A file exactly at the cap (12 MiB) is NOT rejected — only files strictly
    /// greater than MAX_IMPORT_FILE_BYTES are. This documents the boundary.
    /// The file will still fail to parse as JSON (sparse file is all NUL bytes)
    /// but the size guard must pass without a "too large" error.
    #[test]
    fn file_at_exact_cap_passes_size_guard() {
        let dir = tempfile::tempdir().unwrap();
        let path = make_sparse_file(dir.path(), "exact.json", MAX_IMPORT_FILE_BYTES);

        let sock = dir.path().join("dummy.sock");
        let err = run(&sock, path.to_str().unwrap()).unwrap_err();
        let msg = format!("{err}");
        // Must NOT be a "too large" error — the exact-cap file is within the limit.
        assert!(
            !msg.contains("too large"),
            "exact-cap file should pass size guard but got: {msg}"
        );
    }
}

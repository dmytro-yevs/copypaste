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
/// A 64 MiB cap prevents an accidental (or malicious) multi-GB file from
/// OOM-ing the CLI process. Legitimate export files are bounded by the
/// daemon's own in-memory list limit, so this cap should never be hit in
/// normal usage.
const MAX_IMPORT_FILE_BYTES: u64 = 64 * 1024 * 1024; // 64 MiB

pub fn run(socket_path: &Path, file: &str) -> Result<()> {
    // Pre-check file size before reading into memory.  read_to_string would
    // allocate the full file contents up-front; without this guard a multi-GB
    // file could exhaust the process's virtual address space.
    let file_size = std::fs::metadata(file)
        .with_context(|| format!("failed to stat import file: {file}"))?
        .len();
    if file_size > MAX_IMPORT_FILE_BYTES {
        return Err(anyhow!(
            "import file is too large ({} bytes > {} byte limit): {file}",
            file_size,
            MAX_IMPORT_FILE_BYTES
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

    /// A file larger than MAX_IMPORT_FILE_BYTES must be rejected before
    /// read_to_string so we never allocate the full content in memory.
    /// We create a sparse file (via set_len) so the test stays fast.
    #[test]
    fn oversized_file_is_rejected_before_read() {
        use std::fs::OpenOptions;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.json");

        // Create a sparse file just over the cap without writing all the bytes.
        let f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        f.set_len(MAX_IMPORT_FILE_BYTES + 1).unwrap();
        drop(f);

        let sock = dir.path().join("dummy.sock"); // doesn't need to exist
        let err = run(&sock, path.to_str().unwrap()).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("too large"),
            "expected 'too large' error, got: {msg}"
        );
    }
}

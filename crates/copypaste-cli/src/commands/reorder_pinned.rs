//! `copypaste reorder-pinned` — reorder pinned clipboard items.
//!
//! Sends the [`METHOD_REORDER_PINNED`] IPC verb to the daemon with the complete
//! ordered list of pinned item IDs. The daemon stores the order and returns items
//! sorted by it in subsequent `history_page` responses.
//!
//! ## Usage
//!
//! ```text
//! copypaste reorder-pinned <id1> <id2> <id3> ...
//! ```
//!
//! All provided IDs must be pinned items; the daemon will error if any ID is
//! unknown or not pinned. The order is the complete replacement — any pinned
//! item not listed will fall back to the daemon's default ordering.
//!
//! ## Exit codes
//! - 0 — order saved successfully
//! - 1 — daemon not running, IPC error, or missing/invalid item IDs

use anyhow::{anyhow, Context, Result};
use copypaste_ipc::METHOD_REORDER_PINNED;
use std::path::Path;

use crate::commands::common::exit_on_err;
use crate::ipc::IpcClient;

/// Send the reorder-pinned request to the daemon.
///
/// `ids` is the complete ordered list of pinned item UUIDs. Provide them in
/// the desired display order (first = top).
pub fn run(socket_path: &Path, ids: &[String]) -> Result<()> {
    if ids.is_empty() {
        return Err(anyhow!(
            "at least one item ID is required — \
             provide the pinned item IDs in the desired order"
        ));
    }

    let mut client = IpcClient::connect(socket_path)
        .with_context(|| format!("daemon is not running (socket: {})", socket_path.display()))?;

    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_REORDER_PINNED,
        serde_json::json!({ "ids": ids }),
    );
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    println!("Pinned item order saved ({} item(s)).", ids.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_signature_compiles() {
        let _: fn(&Path, &[String]) -> Result<()> = run;
    }

    #[test]
    fn method_constant_has_correct_wire_name() {
        assert_eq!(METHOD_REORDER_PINNED, "reorder_pinned");
    }

    #[test]
    fn run_rejects_empty_ids() {
        let sock = Path::new("/tmp/nonexistent.sock");
        let err = run(sock, &[]).unwrap_err();
        assert!(
            err.to_string().contains("at least one item ID"),
            "expected clear error for empty ids, got: {err}"
        );
    }
}

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
use std::path::Path;

use crate::commands::common::exit_on_err;
use crate::ipc::IpcClient;

pub fn run(socket_path: &Path, file: &str) -> Result<()> {
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
    let req = IpcClient::build_request("1", "import", serde_json::json!({ "items": items }));
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
}

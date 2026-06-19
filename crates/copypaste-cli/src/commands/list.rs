use crate::commands::common::{exit_on_err, format_unix_ms};
use crate::ipc::IpcClient;
use anyhow::Result;
use copypaste_ipc::METHOD_LIST;
use std::path::Path;

pub fn run(socket_path: &Path, limit: u64, offset: u64) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_LIST,
        serde_json::json!({"limit": limit, "offset": offset}),
    );
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    let data = resp.data.unwrap_or(serde_json::Value::Null);
    let total = data["total"].as_u64().unwrap_or(0);
    let items = data["items"].as_array().cloned().unwrap_or_default();

    if items.is_empty() {
        println!("No items (total: {})", total);
        return Ok(());
    }

    // bhr8: header now includes preview, kind, and pinned columns.
    println!(
        "{:<38}  {:<6}  {:<9}  {:<7}  {:<8}  {:<28}  TIME (UTC)",
        "ID", "KIND", "TYPE", "PINNED", "SENSITIVE", "PREVIEW"
    );
    println!("{}", "-".repeat(130));

    for item in &items {
        let id = item["id"].as_str().unwrap_or("?");
        let ctype = item["content_type"].as_str().unwrap_or("?");
        let kind = item["kind"].as_str().unwrap_or("?");
        let sensitive = if item["is_sensitive"].as_bool().unwrap_or(false) {
            "yes"
        } else {
            "no"
        };
        let pinned = if item["pinned"].as_bool().unwrap_or(false) {
            "yes"
        } else {
            "no"
        };
        let preview_raw = item["preview"].as_str().unwrap_or("");
        // Truncate preview to 28 chars for terminal fit; mark truncation with …
        let preview: String = if preview_raw.chars().count() > 28 {
            let truncated: String = preview_raw.chars().take(27).collect();
            format!("{}…", truncated)
        } else {
            preview_raw.to_string()
        };
        // Replace newlines/tabs with spaces so the table stays single-line.
        let preview = preview.replace(['\n', '\r', '\t'], " ");
        let ts = item["wall_time"].as_i64().unwrap_or(0);
        let time_str = format_unix_ms(ts);
        println!(
            "{:<38}  {:<6}  {:<9}  {:<7}  {:<8}  {:<28}  {}",
            id, kind, ctype, pinned, sensitive, preview, time_str
        );
    }

    println!();
    println!(
        "Showing {}-{} of {} total",
        offset + 1,
        offset + items.len() as u64,
        total
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_signature_compiles() {
        let _: fn(&Path, u64, u64) -> Result<()> = run;
    }

    /// Preview truncation at 28 chars leaves a trailing ellipsis.
    #[test]
    fn preview_truncation_at_28() {
        let long: String = "a".repeat(35);
        let truncated: String = if long.chars().count() > 28 {
            let t: String = long.chars().take(27).collect();
            format!("{}…", t)
        } else {
            long.clone()
        };
        assert_eq!(truncated.chars().count(), 28); // 27 + ellipsis = 28
    }

    /// Whitespace in preview is collapsed to spaces.
    #[test]
    fn preview_newlines_replaced() {
        let raw = "hello\nworld\ttab";
        let cleaned = raw.replace(['\n', '\r', '\t'], " ");
        assert_eq!(cleaned, "hello world tab");
    }
}

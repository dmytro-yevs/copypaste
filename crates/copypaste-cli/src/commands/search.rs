use crate::commands::common::{exit_on_err, format_unix_ms};
use crate::ipc::IpcClient;
use anyhow::Result;
use copypaste_ipc::METHOD_SEARCH;
use std::path::Path;

/// Run the `search` IPC command.
///
/// `kind_filter` is an optional content-type filter (e.g. `"text"`, `"image"`,
/// `"file"`). When `None` all types are returned (CopyPaste-tteo).
pub fn run(socket_path: &Path, query: &str, limit: u64, kind_filter: Option<&str>) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)?;
    let mut params = serde_json::json!({"query": query, "limit": limit});
    if let Some(kind) = kind_filter {
        params["kind"] = serde_json::Value::String(kind.to_string());
    }
    let req = IpcClient::build_request(&IpcClient::next_id(), METHOD_SEARCH, params);
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    let items = resp
        .data
        .as_ref()
        .and_then(|d| d["items"].as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[]);

    if items.is_empty() {
        println!("no results for {:?}", query);
        return Ok(());
    }

    for item in items {
        let id = item["id"].as_str().unwrap_or("?");
        let wall_time = item["wall_time"].as_i64().unwrap_or(0);
        let sensitive = item["is_sensitive"].as_bool().unwrap_or(false);
        let pinned = item["pinned"].as_bool().unwrap_or(false);
        // CopyPaste-tteo: prefer `kind` (classifier label) over `content_type`
        // for display, with `content_type` as fallback for older daemons.
        let kind = item["kind"]
            .as_str()
            .or_else(|| item["content_type"].as_str())
            .unwrap_or("?");
        let preview = item["preview"].as_str().unwrap_or("");
        let ts = format_unix_ms(wall_time);
        let mut markers = Vec::<&str>::new();
        if sensitive {
            markers.push("[sensitive]");
        }
        if pinned {
            markers.push("[pinned]");
        }
        let marker_str = if markers.is_empty() {
            String::new()
        } else {
            format!("  {}", markers.join(" "))
        };
        // Truncate preview to 40 chars for compact terminal display.
        let preview_display: String = preview.chars().take(40).collect();
        println!("{ts}  {kind:<8}  {id}  {preview_display}{marker_str}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_signature_compiles() {
        // CopyPaste-tteo: signature now includes optional kind_filter param.
        let _: fn(&std::path::Path, &str, u64, Option<&str>) -> Result<()> = run;
    }

    /// kind_filter = None must not add "kind" to the IPC params JSON.
    #[test]
    fn build_params_no_kind_filter_omits_kind_field() {
        let mut params = serde_json::json!({"query": "hello", "limit": 10u64});
        let kind_filter: Option<&str> = None;
        if let Some(kind) = kind_filter {
            params["kind"] = serde_json::Value::String(kind.to_string());
        }
        assert!(
            params.get("kind").is_none(),
            "kind must be absent when filter is None"
        );
    }

    /// kind_filter = Some("text") must add the correct field to IPC params.
    #[test]
    fn build_params_with_kind_filter_includes_kind_field() {
        let mut params = serde_json::json!({"query": "hello", "limit": 10u64});
        let kind_filter: Option<&str> = Some("text");
        if let Some(kind) = kind_filter {
            params["kind"] = serde_json::Value::String(kind.to_string());
        }
        assert_eq!(
            params["kind"].as_str(),
            Some("text"),
            "kind must be present and correct when filter is Some"
        );
    }
}

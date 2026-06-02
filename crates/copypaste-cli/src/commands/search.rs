use crate::commands::common::{exit_on_err, format_unix_ms};
use crate::ipc::IpcClient;
use anyhow::Result;
use copypaste_ipc::METHOD_SEARCH;
use std::path::Path;

pub fn run(socket_path: &Path, query: &str, limit: u64) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_SEARCH,
        serde_json::json!({"query": query, "limit": limit}),
    );
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
        let content_type = item["content_type"].as_str().unwrap_or("?");
        let wall_time = item["wall_time"].as_i64().unwrap_or(0);
        let sensitive = item["is_sensitive"].as_bool().unwrap_or(false);
        let ts = format_unix_ms(wall_time);
        let sens_marker = if sensitive { " [sensitive]" } else { "" };
        println!("{ts}  {content_type:<6}  {id}{sens_marker}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_signature_compiles() {
        let _: fn(&std::path::Path, &str, u64) -> Result<()> = run;
    }
}

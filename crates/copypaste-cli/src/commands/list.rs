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

    println!(
        "{:<38}  {:<12}  {:<10}  TIME (UTC)",
        "ID", "TYPE", "SENSITIVE"
    );
    println!("{}", "-".repeat(90));

    for item in &items {
        let id = item["id"].as_str().unwrap_or("?");
        let ctype = item["content_type"].as_str().unwrap_or("?");
        let sensitive = if item["is_sensitive"].as_bool().unwrap_or(false) {
            "yes"
        } else {
            "no"
        };
        let ts = item["wall_time"].as_i64().unwrap_or(0);
        let time_str = format_unix_ms(ts);
        println!("{:<38}  {:<12}  {:<10}  {}", id, ctype, sensitive, time_str);
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
}

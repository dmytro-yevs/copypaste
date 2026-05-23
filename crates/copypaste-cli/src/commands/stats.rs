use crate::commands::common::exit_on_err;
use crate::ipc::IpcClient;
use anyhow::Result;
use std::path::Path;

pub fn run(socket_path: &Path) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = serde_json::json!({"id": "1", "method": "stats"});
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    if let Some(data) = &resp.data {
        let total = data["total_items"].as_i64().unwrap_or(0);
        let sensitive = data["sensitive_items"].as_i64().unwrap_or(0);
        let version = data["version"].as_str().unwrap_or("?");
        println!("total:     {total}");
        println!("sensitive: {sensitive}");
        println!("version:   {version}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn run_signature_compiles() {
        let _: fn(&Path) -> Result<()> = run;
    }
}

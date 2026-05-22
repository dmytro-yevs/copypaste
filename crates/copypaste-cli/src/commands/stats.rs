use anyhow::Result;
use crate::ipc::IpcClient;
use std::path::Path;

pub fn run(socket_path: &Path) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = serde_json::json!({"id": "1", "method": "stats"});
    let resp = client.call(&req)?;

    if !resp.ok {
        eprintln!("error: {}", resp.error.unwrap_or_default());
        std::process::exit(1);
    }

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

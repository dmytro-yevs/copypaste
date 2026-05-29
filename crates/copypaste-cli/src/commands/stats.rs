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
        // The daemon's `version` field is the IPC/schema version, NOT a semver
        // release. Label it as such and surface the CLI's real crate semver
        // separately so users don't mistake "1" for the app version.
        let schema_version = data["version"].as_str().unwrap_or("?");
        println!("total:      {total}");
        println!("sensitive:  {sensitive}");
        println!("schema:     {schema_version}");
        println!("cli version: {}", env!("CARGO_PKG_VERSION"));
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

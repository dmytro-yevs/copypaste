use anyhow::Result;
use crate::commands::common::exit_on_err;
use crate::ipc::IpcClient;
use std::path::Path;

pub fn run(socket_path: &Path) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = serde_json::json!({"id": "1", "method": "count", "params": {}});
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    let count = resp.data
        .as_ref()
        .and_then(|d| d["count"].as_u64())
        .unwrap_or(0);
    println!("{} items", count);
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

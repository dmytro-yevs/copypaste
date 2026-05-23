use anyhow::Result;
use crate::commands::common::exit_on_err;
use crate::ipc::IpcClient;
use std::path::Path;

pub fn run(socket_path: &Path) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = serde_json::json!({"id": "1", "method": "status", "params": {}});
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    println!("daemon: running");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: function signature compiles and types are correct.
    /// Live socket test is in tests/cli_integration.rs.
    #[test]
    fn run_signature_compiles() {
        let _: fn(&Path) -> Result<()> = run;
    }
}

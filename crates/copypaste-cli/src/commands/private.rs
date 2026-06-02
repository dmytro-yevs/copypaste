use crate::commands::common::exit_on_err;
use crate::ipc::IpcClient;
use anyhow::Result;
use copypaste_ipc::{METHOD_GET_PRIVATE_MODE, METHOD_SET_PRIVATE_MODE};
use std::path::Path;

/// Enable or disable private/pause mode on the daemon.
/// When enabled, the daemon skips recording new clipboard changes.
pub fn run(socket_path: &Path, enable: bool) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_SET_PRIVATE_MODE,
        serde_json::json!({ "enabled": enable }),
    );
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    let mode = if enable { "enabled" } else { "disabled" };
    println!("private mode {mode}");
    Ok(())
}

/// Query the current private mode state from the daemon.
pub fn run_get(socket_path: &Path) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_GET_PRIVATE_MODE,
        serde_json::json!({}),
    );
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    let enabled = resp
        .data
        .as_ref()
        .and_then(|d| d.get("private_mode"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    println!("private mode: {}", if enabled { "on" } else { "off" });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: function signatures compile.
    #[test]
    fn run_signature_compiles() {
        let _: fn(&Path, bool) -> Result<()> = run;
        let _: fn(&Path) -> Result<()> = run_get;
    }
}

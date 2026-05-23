use crate::commands::common::exit_on_err;
use crate::ipc::IpcClient;
use anyhow::Result;
use std::path::Path;

pub fn run(socket_path: &Path, id: &str) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = serde_json::json!({"id": "1", "method": "delete", "params": {"id": id}});
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    println!("deleted {}", id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_signature_compiles() {
        let _: fn(&Path, &str) -> Result<()> = run;
    }
}

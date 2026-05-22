use anyhow::Result;
use crate::ipc::IpcClient;
use std::path::Path;

pub fn run(socket_path: &Path, id: &str) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = serde_json::json!({"id": "1", "method": "delete", "params": {"id": id}});
    let resp = client.call(&req)?;

    if resp.ok {
        println!("deleted {}", id);
        Ok(())
    } else {
        eprintln!("error: {}", resp.error.unwrap_or_default());
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_signature_compiles() {
        let _: fn(&Path, &str) -> Result<()> = run;
    }
}

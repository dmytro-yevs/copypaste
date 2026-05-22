use anyhow::Result;
use crate::ipc::IpcClient;
use std::path::Path;

pub fn run(socket_path: &Path, id: &str) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = serde_json::json!({
        "id": "1",
        "method": "copy",
        "params": {"id": id}
    });
    let resp = client.call(&req)?;

    if resp.ok {
        println!("copied to clipboard");
        Ok(())
    } else {
        let err = resp.error.as_deref().unwrap_or("unknown error");
        if err.contains("unknown method") {
            eprintln!("copy: daemon does not yet support this command (requires Phase 2a+)");
            std::process::exit(2);
        }
        eprintln!("error: {err}");
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

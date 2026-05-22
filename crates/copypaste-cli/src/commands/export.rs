use anyhow::Result;
use crate::ipc::IpcClient;
use std::io::Write;
use std::path::Path;

pub fn run(socket_path: &Path, limit: u64, output: Option<&str>) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = serde_json::json!({
        "id": "1", "method": "list",
        "params": {"limit": limit, "offset": 0}
    });
    let resp = client.call(&req)?;

    if !resp.ok {
        eprintln!("error: {}", resp.error.unwrap_or_default());
        std::process::exit(1);
    }

    let json = serde_json::to_string_pretty(&resp.data)?;

    match output {
        Some(path) => {
            std::fs::write(path, &json)?;
            eprintln!("exported to {path}");
        }
        None => {
            std::io::stdout().write_all(json.as_bytes())?;
            println!();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn run_signature_compiles() {
        let _: fn(&Path, u64, Option<&str>) -> Result<()> = run;
    }
}

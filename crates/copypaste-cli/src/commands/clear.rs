use anyhow::Result;
use std::path::Path;

use crate::ipc::IpcClient;

pub fn run(socket_path: &Path, force: bool) -> Result<()> {
    if !force {
        eprint!("This will delete ALL clipboard history. Type 'yes' to confirm: ");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if input.trim() != "yes" {
            eprintln!("aborted.");
            return Ok(());
        }
    }

    let mut client = IpcClient::connect(socket_path)?;
    let req = serde_json::json!({"id": "1", "method": "delete_all"});
    let resp = client.call(&req)?;

    if resp.ok {
        let deleted = resp.data.as_ref().and_then(|d| d["deleted"].as_i64()).unwrap_or(0);
        println!("cleared {deleted} items");
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
        let _: fn(&Path, bool) -> Result<()> = run;
    }
}

use crate::commands::common::exit_on_err;
use crate::ipc::IpcClient;
use anyhow::Result;
use copypaste_ipc::METHOD_DELETE;
use std::path::Path;

pub fn run(socket_path: &Path, id: &str) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_DELETE,
        serde_json::json!({"id": id}),
    );
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

use crate::commands::common::exit_on_err;
use crate::ipc::IpcClient;
use anyhow::{anyhow, Result};
use std::io::Write;
use std::path::Path;

pub fn run(socket_path: &Path, limit: u64, output: Option<&str>, force: bool) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = IpcClient::build_request(
        "1",
        "list",
        serde_json::json!({"limit": limit, "offset": 0}),
    );
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    let json = serde_json::to_string_pretty(&resp.data)?;

    match output {
        Some(path) => {
            write_to_file(path.as_ref(), &json, force)?;
            eprintln!("exported to {path}");
        }
        None => {
            std::io::stdout().write_all(json.as_bytes())?;
            println!();
        }
    }
    Ok(())
}

/// Write `contents` to `path`. If `path` already exists and `force` is false,
/// returns an error instead of overwriting.
fn write_to_file(path: &Path, contents: &str, force: bool) -> Result<()> {
    if path.exists() && !force {
        return Err(anyhow!(
            "file exists, use --force to overwrite: {}",
            path.display()
        ));
    }
    std::fs::write(path, contents)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn run_signature_compiles() {
        let _: fn(&Path, u64, Option<&str>, bool) -> Result<()> = run;
    }

    fn tmp_path(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        let unique = format!(
            "copypaste-export-test-{}-{}-{}",
            std::process::id(),
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        p.push(unique);
        p
    }

    #[test]
    fn export_refuses_overwrite_without_force() {
        let path = tmp_path("refuse");
        fs::write(&path, "old contents").unwrap();

        let result = write_to_file(&path, "new contents", false);
        assert!(
            result.is_err(),
            "expected error when file exists without --force"
        );

        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("file exists"),
            "error message should mention 'file exists', got: {msg}"
        );

        // Original contents must remain untouched.
        let still = fs::read_to_string(&path).unwrap();
        assert_eq!(still, "old contents");

        fs::remove_file(&path).ok();
    }

    #[test]
    fn export_with_force_overwrites_existing_file() {
        let path = tmp_path("force");
        fs::write(&path, "old contents").unwrap();

        let result = write_to_file(&path, "new contents", true);
        assert!(
            result.is_ok(),
            "expected Ok when --force is set, got {result:?}"
        );

        let written = fs::read_to_string(&path).unwrap();
        assert_eq!(written, "new contents");

        fs::remove_file(&path).ok();
    }

    #[test]
    fn export_creates_new_file_when_missing() {
        let path = tmp_path("new");
        assert!(!path.exists(), "precondition: temp path must not exist");

        let result = write_to_file(&path, "fresh contents", false);
        assert!(
            result.is_ok(),
            "expected Ok when target file does not exist, got {result:?}"
        );

        let written = fs::read_to_string(&path).unwrap();
        assert_eq!(written, "fresh contents");

        fs::remove_file(&path).ok();
    }
}

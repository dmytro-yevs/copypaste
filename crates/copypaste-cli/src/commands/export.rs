use crate::ipc::IpcClient;
use anyhow::{anyhow, Result};
use copypaste_ipc::METHOD_EXPORT;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

pub fn run(
    socket_path: &Path,
    limit: u64,
    output: Option<&str>,
    force: bool,
    include_sensitive: bool,
) -> Result<()> {
    if include_sensitive {
        // Mirror the --raw warning style used by pair-qr: emit on stderr so
        // that stdout output remains pipeable/scriptable.
        eprintln!(
            "WARNING: exporting sensitive items in plaintext — \
             handle the output securely and delete it when done."
        );
    }

    let mut client = IpcClient::connect(socket_path)?;
    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_EXPORT,
        serde_json::json!({"limit": limit, "include_sensitive": include_sensitive}),
    );
    let resp = client.call(&req)?;

    // If the daemon does not recognise the `export` method (older daemon or
    // daemon not yet updated), it returns ok=false with an "unknown method"
    // error.  Emit a clear, actionable message rather than silently writing an
    // empty / incomplete file — a contentless backup is worse than no file.
    if !resp.ok {
        let msg = resp.error.as_deref().unwrap_or("unknown error");
        if msg.contains("unknown method") || msg.contains("not found") {
            return Err(anyhow!(
                "daemon does not support the 'export' method — \
                 please upgrade the daemon to a version that includes \
                 export/import support (error: {msg})"
            ));
        }
        return Err(anyhow!("daemon returned error: {msg}"));
    }

    // The daemon returns {"items": [...]} in the `data` field.  Each item
    // carries `content_bytes_b64` + `created_at_ms` which are exactly the
    // fields that `import` requires, so we can round-trip without any
    // transformation.
    let data = resp
        .data
        .ok_or_else(|| anyhow!("daemon returned ok with no data for 'export'"))?;

    // Sanity-check: the payload must contain an `items` array.  An export
    // file with a missing `items` key is silently unusable on re-import, so
    // we fail loudly here instead.
    if data.get("items").and_then(|v| v.as_array()).is_none() {
        return Err(anyhow!(
            "daemon 'export' response is missing the 'items' array — \
             daemon may be partially updated; try restarting it"
        ));
    }

    let json = serde_json::to_string_pretty(&data)?;

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
///
/// The non-force path uses `create_new(true)` which is atomic (single
/// open(2)/CreateFile call) and avoids the TOCTOU race that a separate
/// `path.exists()` check introduces.
fn write_to_file(path: &Path, contents: &str, force: bool) -> Result<()> {
    let mut file = if force {
        // Truncate-or-create: overwrite unconditionally.
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
    } else {
        // Atomic create-or-fail: errors if the file already exists.
        OpenOptions::new().write(true).create_new(true).open(path)
    }
    .map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            anyhow!("file exists, use --force to overwrite: {}", path.display())
        } else {
            anyhow!("could not open {}: {e}", path.display())
        }
    })?;

    file.write_all(contents.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn run_signature_compiles() {
        let _: fn(&Path, u64, Option<&str>, bool, bool) -> Result<()> = run;
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

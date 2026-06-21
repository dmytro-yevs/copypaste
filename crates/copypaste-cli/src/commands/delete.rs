use crate::commands::common::exit_on_err;
use crate::ipc::IpcClient;
use anyhow::Result;
// CopyPaste-abg1: use the current IPC method name.
// METHOD_DELETE_ITEM is the up-to-date verb. METHOD_DELETE is the legacy
// alias retained in copypaste-ipc for back-compat; new CLI code must not
// use it.
use copypaste_ipc::methods::METHOD_DELETE_ITEM;
use std::path::Path;

/// Run the `delete` command.
///
/// CopyPaste-2vp5: adds `--force` and `--dry-run` flags.
///
/// - `force`: skip the interactive confirmation prompt. Without it the user
///   is asked to confirm before the IPC call is made.
/// - `dry_run`: print what would be deleted without sending the IPC request.
///   Useful for scripting and double-checking the id before a destructive op.
pub fn run(socket_path: &Path, id: &str, force: bool, dry_run: bool) -> Result<()> {
    // Confirmation prompt: shown when neither --force nor --dry-run is set.
    // dry_run implies the user already knows what will happen; --force skips
    // the prompt for non-interactive / scripted invocations.
    if !force && !dry_run {
        eprint!("Delete item '{id}'? Type 'yes' to confirm: ");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if input.trim() != "yes" {
            // User declined. Print to stderr (not stdout) and return Ok so the
            // caller exits 0 — declining is a normal, non-error outcome.
            eprintln!("aborted.");
            return Ok(());
        }
    }

    if dry_run {
        // Dry-run: show what would happen without touching the daemon.
        println!("would delete {id}");
        return Ok(());
    }

    let mut client = IpcClient::connect(socket_path)?;
    // CopyPaste-abg1: use METHOD_DELETE_ITEM (current verb) instead of the
    // legacy METHOD_DELETE alias.
    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_DELETE_ITEM,
        serde_json::json!({"id": id}),
    );
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    println!("deleted {id}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::net::UnixListener;
    use std::thread;
    use tempfile::tempdir;

    /// CopyPaste-2vp5: updated signature must accept force and dry_run args.
    #[test]
    fn run_signature_compiles() {
        let _: fn(&Path, &str, bool, bool) -> Result<()> = run;
    }

    /// CopyPaste-2vp5: --dry-run prints the item id without an IPC call.
    /// We verify by calling run with a socket path that has no listener —
    /// if an IPC call were attempted it would fail with a connect error.
    #[test]
    fn dry_run_prints_without_ipc() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("no_listener.sock");
        // No UnixListener → IpcClient::connect would fail if called.
        let res = run(&sock, "test-uuid", true, true); // force=true, dry_run=true
        assert!(res.is_ok(), "dry_run must succeed without IPC: {res:?}");
    }

    fn mock_server_once(socket_path: &Path, response_template: &'static str) {
        let listener = UnixListener::bind(socket_path).unwrap();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = String::new();
                std::io::BufRead::read_line(&mut std::io::BufReader::new(&stream), &mut buf)
                    .unwrap();
                let req_id = serde_json::from_str::<serde_json::Value>(buf.trim())
                    .ok()
                    .and_then(|v| v["id"].as_str().map(|s| s.to_string()))
                    .unwrap_or_else(|| "1".to_string());
                let response = response_template.replace("ECHO_ID", &req_id);
                stream.write_all(response.as_bytes()).unwrap();
                stream.write_all(b"\n").unwrap();
            }
        });
    }

    /// CopyPaste-2vp5: --force skips the prompt and sends the IPC delete.
    #[test]
    fn force_skips_prompt_and_deletes() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("delete_force.sock");
        mock_server_once(
            &sock,
            r#"{"id":"ECHO_ID","ok":true,"data":{"deleted":true}}"#,
        );
        // force=true, dry_run=false → skips prompt, sends IPC
        let res = run(&sock, "some-uuid", true, false);
        assert!(res.is_ok(), "force delete must succeed: {res:?}");
    }

    /// CopyPaste-2vp5: --dry-run takes priority when both flags are set.
    #[test]
    fn dry_run_priority_over_force() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("no_listener2.sock");
        // No server — IPC must not be called.
        let res = run(&sock, "uuid-xyz", true, true);
        assert!(res.is_ok(), "dry_run+force must not fail: {res:?}");
    }

    /// CopyPaste-abg1: run (with --force) must send "delete_item" on the wire,
    /// NOT the legacy "delete" method. We capture the raw request in the mock
    /// server and assert on the "method" field.
    #[test]
    fn run_uses_delete_item_method() {
        use std::sync::{Arc, Mutex};

        let dir = tempdir().unwrap();
        let sock = dir.path().join("method_check_del.sock");
        let listener = UnixListener::bind(&sock).unwrap();
        let captured: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let cap = Arc::clone(&captured);

        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = String::new();
                std::io::BufRead::read_line(&mut std::io::BufReader::new(&stream), &mut buf)
                    .unwrap();
                *cap.lock().unwrap() = Some(buf.trim().to_string());
                let req_id = serde_json::from_str::<serde_json::Value>(buf.trim())
                    .ok()
                    .and_then(|v| v["id"].as_str().map(|s| s.to_string()))
                    .unwrap_or_else(|| "1".to_string());
                let resp = format!(r#"{{"id":"{req_id}","ok":true,"data":{{"deleted":true}}}}"#);
                stream.write_all(resp.as_bytes()).unwrap();
                stream.write_all(b"\n").unwrap();
            }
        });

        // force=true skips prompt; dry_run=false so IPC is actually called.
        let _ = run(&sock, "del-uuid", true, false);

        let raw = captured.lock().unwrap().clone().unwrap_or_default();
        let v: serde_json::Value =
            serde_json::from_str(&raw).expect("captured request must be JSON");
        assert_eq!(
            v["method"].as_str(),
            Some("delete_item"),
            "CopyPaste-abg1: must send 'delete_item', not 'delete' — got: {raw}"
        );
    }
}

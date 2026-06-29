use crate::commands::common::format_unix_ms;
use crate::ipc::IpcClient;
use anyhow::{anyhow, bail, Result};
// CopyPaste-abg1: use the current IPC method names.
// METHOD_COPY_ITEM is the up-to-date verb (returns richer response with
// decrypted text). METHOD_COPY is the legacy alias; kept in copypaste-ipc
// for back-compat but must not be used by new CLI code.
use copypaste_ipc::methods::METHOD_COPY_ITEM;
// CopyPaste-crh3.99: use METHOD_HISTORY_PAGE — the daemon's list handler now
// returns ERR_CODE_NOT_IMPLEMENTED, so fetching history must use history_page.
use copypaste_ipc::{METHOD_HISTORY_PAGE, METHOD_SEARCH};
use std::path::Path;

/// Exit code used when the `copy` command is invoked with no mode.
///
/// CopyPaste-nmr0: must be distinct from every other CLI exit code:
///   0 = success
///   1 = runtime error (daemon failure, IPC error)
///   2 = user aborted (`clear` confirmation declined — [`crate::commands::clear::ABORT_EXIT_CODE`])
///   3 = bad usage (this constant)
///
/// Using 3 here prevents scripts from confusing "user said no to clear" (2)
/// with "copy was invoked incorrectly" (3).
pub const USAGE_EXIT_CODE: i32 = 3;

/// Run the `copy` command.
///
/// Modes (exactly one must be active):
/// - `index`: 1-based position in history (most recent = 1)
/// - `id`: exact UUID of the item
/// - `search`: fuzzy-search text; copies the first match
/// - `list`: print numbered history table without copying
///
/// On success the item id is printed to stdout (pipe-friendly).
/// Status messages go to stderr.
pub fn run(
    socket_path: &Path,
    index: Option<u64>,
    id: Option<&str>,
    search: Option<&str>,
    list: bool,
    limit: u64,
) -> Result<()> {
    match (index, id, search, list) {
        // --list: show numbered history, no copy
        (None, None, None, true) => cmd_list(socket_path, limit),

        // --id <UUID>: copy by exact id
        (None, Some(uuid), None, false) => cmd_copy_by_id(socket_path, uuid),

        // --search <QUERY>: fuzzy-search, copy first match
        (None, None, Some(query), false) => cmd_copy_by_search(socket_path, query, limit),

        // INDEX: copy by 1-based position
        (Some(n), None, None, false) => cmd_copy_by_index(socket_path, n, limit),

        // No mode provided — print usage hint
        (None, None, None, false) => {
            eprintln!("copypaste copy: specify INDEX, --id <UUID>, --search <QUERY>, or --list");
            eprintln!("  copypaste copy 1          # copy most recent item");
            eprintln!("  copypaste copy --list     # show numbered history");
            // CopyPaste-liaz: process::exit is safe here — no Zeroizing<…>
            // or secret material is live in this scope.
            //
            // CopyPaste-nmr0: exit code 3 (USAGE_EXIT_CODE) is distinct from:
            //   1 = generic runtime error (main.rs)
            //   2 = clear abort (clear::ABORT_EXIT_CODE)
            // Returning Err would lose the distinct code because main.rs always
            // exits 1 on any Err.
            std::process::exit(USAGE_EXIT_CODE);
        }

        // Unreachable: clap enforces conflicts_with_all
        _ => bail!("conflicting copy flags"),
    }
}

// ── Mode implementations ───────────────────────────────────────────────────

/// Fetch history and print a numbered table.
pub fn cmd_list(socket_path: &Path, limit: u64) -> Result<()> {
    let items = fetch_history(socket_path, limit)?;

    if items.is_empty() {
        eprintln!("No clipboard history.");
        return Ok(());
    }

    // Header
    println!("{:<5}  {:<38}  {:<8}  TIME (UTC)", "INDEX", "ID", "TYPE");
    println!("{}", "-".repeat(76));

    for (i, item) in items.iter().enumerate() {
        let idx = i + 1;
        let id = item["id"].as_str().unwrap_or("?");
        let ctype = item["content_type"].as_str().unwrap_or("?");
        let ts = item["wall_time"].as_i64().unwrap_or(0);
        let time_str = format_unix_ms(ts);
        let sensitive = if item["is_sensitive"].as_bool().unwrap_or(false) {
            " *"
        } else {
            ""
        };
        println!(
            "{:<5}  {:<38}  {:<8}  {}{}",
            idx, id, ctype, time_str, sensitive
        );
    }

    println!();
    println!("(* = sensitive)");
    Ok(())
}

/// Copy by 1-based index (most recent = 1).
pub fn cmd_copy_by_index(socket_path: &Path, n: u64, limit: u64) -> Result<()> {
    if n == 0 {
        bail!("INDEX must be 1 or greater (1 = most recent)");
    }

    let effective_limit = limit.max(n);
    let items = fetch_history(socket_path, effective_limit)?;

    let idx = (n - 1) as usize;
    let item = items.get(idx).ok_or_else(|| {
        anyhow!(
            "index {} out of range (history has {} items)",
            n,
            items.len()
        )
    })?;

    let uuid = item["id"]
        .as_str()
        .ok_or_else(|| anyhow!("item has no id"))?;
    cmd_copy_by_id(socket_path, uuid)
}

/// Search history and copy the first result.
pub fn cmd_copy_by_search(socket_path: &Path, query: &str, limit: u64) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_SEARCH,
        serde_json::json!({"query": query, "limit": limit}),
    );
    let resp = client.call(&req)?;

    if !resp.ok {
        let err = resp.error.as_deref().unwrap_or("unknown error");
        bail!("search failed: {err}");
    }

    let items = resp
        .data
        .as_ref()
        .and_then(|d| d["items"].as_array())
        .ok_or_else(|| anyhow!("unexpected search response"))?;

    if items.is_empty() {
        return Err(anyhow!("no results for {:?}", query));
    }

    let uuid = items[0]["id"]
        .as_str()
        .ok_or_else(|| anyhow!("search result has no id"))?;

    cmd_copy_by_id(socket_path, uuid)
}

/// Send `copy_item` IPC for a known UUID.
///
/// CopyPaste-abg1: switched from the legacy `METHOD_COPY` to the current
/// `METHOD_COPY_ITEM` verb. The response shape is richer (includes
/// `decrypted_text`) but the CLI only needs `ok`; we forward the id to
/// stdout unchanged.
pub fn cmd_copy_by_id(socket_path: &Path, id: &str) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_COPY_ITEM,
        serde_json::json!({"id": id}),
    );
    let resp = client.call(&req)?;

    if resp.ok {
        // Print id to stdout (pipe-friendly)
        println!("{id}");
        eprintln!("Copied: {id}");
        Ok(())
    } else {
        let err = resp.error.as_deref().unwrap_or("unknown error");
        if err.contains("unknown method") {
            return Err(anyhow!(
                "copy: daemon does not support copy_item — update the daemon to v0.6+"
            ));
        }
        bail!("{err}");
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Fetch up to `limit` history items from the daemon via the `history_page` IPC method.
///
/// CopyPaste-crh3.99: the legacy `list` handler now returns
/// ERR_CODE_NOT_IMPLEMENTED; `history_page` is the current paginated API.
/// Response shape: `{ items: […], total: u32, own_device_id: String }`.
pub fn fetch_history(socket_path: &Path, limit: u64) -> Result<Vec<serde_json::Value>> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_HISTORY_PAGE,
        serde_json::json!({"limit": limit, "offset": 0}),
    );
    let resp = client.call(&req)?;

    if !resp.ok {
        let err = resp.error.as_deref().unwrap_or("unknown error");
        bail!("could not fetch history: {err}");
    }

    Ok(resp
        .data
        .as_ref()
        .and_then(|d| d["items"].as_array())
        .cloned()
        .unwrap_or_default())
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::net::UnixListener;
    use std::thread;
    use tempfile::tempdir;

    // ── Signature tests ─────────────────────────────────────────────────

    #[test]
    fn run_signature_accepts_all_modes() {
        // Compile-time signature check — the complex fn-pointer type is intentional here.
        #[allow(clippy::type_complexity)]
        let _: fn(&Path, Option<u64>, Option<&str>, Option<&str>, bool, u64) -> Result<()> = run;
    }

    /// CopyPaste-nmr0: USAGE_EXIT_CODE must be distinct from every other
    /// documented CLI exit code so scripts can branch on each case.
    ///
    /// Exit code table:
    ///   0 = success
    ///   1 = runtime error (daemon failure / IPC error, emitted by main.rs)
    ///   2 = user aborted clear prompt (clear::ABORT_EXIT_CODE)
    ///   3 = copy invoked with no mode (USAGE_EXIT_CODE, this constant)
    #[test]
    fn usage_exit_code_is_distinct_from_all_other_codes() {
        use crate::commands::clear::ABORT_EXIT_CODE;
        assert_ne!(USAGE_EXIT_CODE, 0, "must not be success");
        assert_ne!(USAGE_EXIT_CODE, 1, "must not be generic error");
        assert_ne!(
            USAGE_EXIT_CODE, ABORT_EXIT_CODE,
            "must not collide with clear abort"
        );
    }

    // ── Unit: index validation ──────────────────────────────────────────

    #[test]
    fn index_zero_is_invalid() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("sock.sock");
        // Zero validation happens before IPC — no server needed
        let res = cmd_copy_by_index(&sock, 0, 50);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("1 or greater"));
    }

    // ── Mock server helpers ─────────────────────────────────────────────

    /// Spawn a fake daemon that serves one connection, echoing the request id
    /// back in the response. `response_template` is a JSON object string where
    /// the literal `"ECHO_ID"` is replaced with the id from the incoming
    /// request, so the id-enforcement guard in `IpcClient::call` is satisfied.
    fn mock_server_once(socket_path: &Path, response_template: &'static str) {
        let listener = UnixListener::bind(socket_path).unwrap();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = String::new();
                std::io::BufRead::read_line(&mut std::io::BufReader::new(&stream), &mut buf)
                    .unwrap();
                // Parse the request id and splice it into the response so the
                // IpcClient id-enforcement guard accepts the response.
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

    /// Spawn a fake daemon that serves two sequential connections, echoing the
    /// request id in each response via the `ECHO_ID` placeholder.
    fn mock_server_two(socket_path: &Path, first: &'static str, second: &'static str) {
        let listener = UnixListener::bind(socket_path).unwrap();
        thread::spawn(move || {
            for resp_template in [first, second] {
                if let Ok((mut stream, _)) = listener.accept() {
                    let mut buf = String::new();
                    std::io::BufRead::read_line(&mut std::io::BufReader::new(&stream), &mut buf)
                        .unwrap();
                    let req_id = serde_json::from_str::<serde_json::Value>(buf.trim())
                        .ok()
                        .and_then(|v| v["id"].as_str().map(|s| s.to_string()))
                        .unwrap_or_else(|| "1".to_string());
                    let response = resp_template.replace("ECHO_ID", &req_id);
                    stream.write_all(response.as_bytes()).unwrap();
                    stream.write_all(b"\n").unwrap();
                }
            }
        });
    }

    // ── Integration: --list ─────────────────────────────────────────────

    #[test]
    fn cmd_list_empty_history_succeeds() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("list_empty.sock");
        mock_server_once(
            &sock,
            r#"{"id":"ECHO_ID","ok":true,"data":{"items":[],"total":0}}"#,
        );
        let res = cmd_list(&sock, 10);
        assert!(res.is_ok());
    }

    #[test]
    fn cmd_list_with_items_succeeds() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("list_items.sock");
        mock_server_once(
            &sock,
            r#"{"id":"ECHO_ID","ok":true,"data":{"items":[{"id":"aaaa-1111","content_type":"text","is_sensitive":false,"wall_time":1704067200000,"lamport_ts":1},{"id":"bbbb-2222","content_type":"text","is_sensitive":true,"wall_time":1704070800000,"lamport_ts":2}],"total":2}}"#,
        );
        let res = cmd_list(&sock, 50);
        assert!(res.is_ok());
    }

    // ── Integration: copy by ID ─────────────────────────────────────────

    #[test]
    fn cmd_copy_by_id_success() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("copy_id_ok.sock");
        mock_server_once(
            &sock,
            r#"{"id":"ECHO_ID","ok":true,"data":{"id":"test-uuid","found":true}}"#,
        );
        let res = cmd_copy_by_id(&sock, "test-uuid");
        assert!(res.is_ok());
    }

    #[test]
    fn cmd_copy_by_id_not_found() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("copy_id_err.sock");
        mock_server_once(
            &sock,
            r#"{"id":"ECHO_ID","ok":false,"error":"item not found: bad-id"}"#,
        );
        let res = cmd_copy_by_id(&sock, "bad-id");
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("not found"));
    }

    // ── Integration: copy by index ──────────────────────────────────────

    #[test]
    fn cmd_copy_by_index_first_item() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("copy_idx.sock");
        mock_server_two(
            &sock,
            r#"{"id":"ECHO_ID","ok":true,"data":{"items":[{"id":"first-uuid","content_type":"text","is_sensitive":false,"wall_time":1704067200000,"lamport_ts":1}],"total":1}}"#,
            r#"{"id":"ECHO_ID","ok":true,"data":{"id":"first-uuid","found":true}}"#,
        );
        let res = cmd_copy_by_index(&sock, 1, 50);
        assert!(res.is_ok());
    }

    #[test]
    fn cmd_copy_by_index_out_of_range() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("copy_idx_oor.sock");
        mock_server_once(
            &sock,
            r#"{"id":"ECHO_ID","ok":true,"data":{"items":[{"id":"only-one","content_type":"text","is_sensitive":false,"wall_time":1704067200000,"lamport_ts":1}],"total":1}}"#,
        );
        let res = cmd_copy_by_index(&sock, 5, 50);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("out of range"));
    }

    // ── Integration: copy by search ─────────────────────────────────────

    #[test]
    fn cmd_copy_by_search_first_match() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("copy_search_match.sock");
        mock_server_two(
            &sock,
            r#"{"id":"ECHO_ID","ok":true,"data":{"items":[{"id":"match-uuid","content_type":"text","is_sensitive":false,"wall_time":1704067200000,"lamport_ts":1}]}}"#,
            r#"{"id":"ECHO_ID","ok":true,"data":{"id":"match-uuid","found":true}}"#,
        );
        let res = cmd_copy_by_search(&sock, "hello", 20);
        assert!(res.is_ok());
    }

    #[test]
    fn cmd_copy_by_search_error_propagates() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("copy_search_err.sock");
        mock_server_once(
            &sock,
            r#"{"id":"ECHO_ID","ok":false,"error":"missing param: query"}"#,
        );
        let res = cmd_copy_by_search(&sock, "hello", 20);
        assert!(res.is_err());
    }

    // ── Integration: error propagation ─────────────────────────────────

    #[test]
    fn fetch_history_daemon_error_propagates() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("hist_err.sock");
        mock_server_once(
            &sock,
            r#"{"id":"ECHO_ID","ok":false,"error":"db corrupted"}"#,
        );
        let res = fetch_history(&sock, 50);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("db corrupted"));
    }

    #[test]
    fn fetch_history_empty_items_returns_vec() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("hist_empty.sock");
        mock_server_once(
            &sock,
            r#"{"id":"ECHO_ID","ok":true,"data":{"items":[],"total":0}}"#,
        );
        let items = fetch_history(&sock, 50).unwrap();
        assert!(items.is_empty());
    }

    /// CopyPaste-abg1: cmd_copy_by_id must send "copy_item" on the wire,
    /// NOT the legacy "copy" method. We capture the raw request in the mock
    /// server and assert on the "method" field.
    /// CopyPaste-crh3.99: fetch_history must send "history_page" on the wire,
    /// NOT the legacy "list" method which the daemon now returns not_implemented for.
    #[test]
    fn fetch_history_uses_history_page_method() {
        use std::sync::{Arc, Mutex};

        let dir = tempdir().unwrap();
        let sock = dir.path().join("fetch_hist_method.sock");
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
                let resp =
                    format!(r#"{{"id":"{req_id}","ok":true,"data":{{"items":[],"total":0}}}}"#);
                stream.write_all(resp.as_bytes()).unwrap();
                stream.write_all(b"\n").unwrap();
            }
        });

        let _ = fetch_history(&sock, 10);

        let raw = captured.lock().unwrap().clone().unwrap_or_default();
        let v: serde_json::Value =
            serde_json::from_str(&raw).expect("captured request must be JSON");
        assert_eq!(
            v["method"].as_str(),
            Some("history_page"),
            "CopyPaste-crh3.99: must send 'history_page', not 'list' — got: {raw}"
        );
    }

    #[test]
    fn cmd_copy_by_id_uses_copy_item_method() {
        use std::sync::{Arc, Mutex};

        let dir = tempdir().unwrap();
        let sock = dir.path().join("method_check.sock");
        let listener = UnixListener::bind(&sock).unwrap();
        let captured: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let cap = Arc::clone(&captured);

        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = String::new();
                std::io::BufRead::read_line(&mut std::io::BufReader::new(&stream), &mut buf)
                    .unwrap();
                *cap.lock().unwrap() = Some(buf.trim().to_string());
                // Send a success response that echoes the request id.
                let req_id = serde_json::from_str::<serde_json::Value>(buf.trim())
                    .ok()
                    .and_then(|v| v["id"].as_str().map(|s| s.to_string()))
                    .unwrap_or_else(|| "1".to_string());
                let resp = format!(
                    r#"{{"id":"{req_id}","ok":true,"data":{{"id":"test-uuid","found":true}}}}"#
                );
                stream.write_all(resp.as_bytes()).unwrap();
                stream.write_all(b"\n").unwrap();
            }
        });

        let _ = cmd_copy_by_id(&sock, "test-uuid");

        let raw = captured.lock().unwrap().clone().unwrap_or_default();
        let v: serde_json::Value =
            serde_json::from_str(&raw).expect("captured request must be JSON");
        assert_eq!(
            v["method"].as_str(),
            Some("copy_item"),
            "CopyPaste-abg1: must send 'copy_item', not 'copy' — got: {raw}"
        );
    }
}

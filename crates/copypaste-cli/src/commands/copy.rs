use anyhow::{anyhow, bail, Result};
use crate::ipc::IpcClient;
use std::path::Path;

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
            std::process::exit(2);
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
    println!("{:<5}  {:<38}  {:<8}  {}", "INDEX", "ID", "TYPE", "TIME (UTC)");
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
        println!("{:<5}  {:<38}  {:<8}  {}{}", idx, id, ctype, time_str, sensitive);
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
        anyhow!("index {} out of range (history has {} items)", n, items.len())
    })?;

    let uuid = item["id"].as_str().ok_or_else(|| anyhow!("item has no id"))?;
    cmd_copy_by_id(socket_path, uuid)
}

/// Search history and copy the first result.
pub fn cmd_copy_by_search(socket_path: &Path, query: &str, limit: u64) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = serde_json::json!({
        "id": "1",
        "method": "search",
        "params": {"query": query, "limit": limit}
    });
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
        eprintln!("No results for {:?}", query);
        std::process::exit(1);
    }

    let uuid = items[0]["id"]
        .as_str()
        .ok_or_else(|| anyhow!("search result has no id"))?;

    cmd_copy_by_id(socket_path, uuid)
}

/// Send `copy` IPC for a known UUID.
pub fn cmd_copy_by_id(socket_path: &Path, id: &str) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = serde_json::json!({
        "id": "1",
        "method": "copy",
        "params": {"id": id}
    });
    let resp = client.call(&req)?;

    if resp.ok {
        // Print id to stdout (pipe-friendly)
        println!("{}", id);
        eprintln!("Copied: {}", id);
        Ok(())
    } else {
        let err = resp.error.as_deref().unwrap_or("unknown error");
        if err.contains("unknown method") {
            eprintln!("copy: daemon does not yet support this command (requires Phase 2a+)");
            std::process::exit(2);
        }
        bail!("{err}");
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Fetch up to `limit` history items from the daemon via the `list` IPC method.
pub fn fetch_history(socket_path: &Path, limit: u64) -> Result<Vec<serde_json::Value>> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = serde_json::json!({
        "id": "1",
        "method": "list",
        "params": {"limit": limit, "offset": 0}
    });
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

/// Format Unix epoch milliseconds as "YYYY-MM-DD HH:MM:SS" (std only, no chrono).
fn format_unix_ms(ms: i64) -> String {
    if ms <= 0 {
        return "\u{2014}".to_string();
    }
    let secs = (ms / 1000) as u64;
    let ss = secs % 60;
    let mins = secs / 60;
    let mi = mins % 60;
    let hours = mins / 60;
    let h = hours % 24;
    let days = hours / 24;
    let (y, mo, d) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, mo, d, h, mi, ss)
}

fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    let mut remaining = days;
    let mut year = 1970u64;
    loop {
        let diy = if is_leap(year) { 366 } else { 365 };
        if remaining < diy {
            break;
        }
        remaining -= diy;
        year += 1;
    }
    let leap = is_leap(year);
    let month_days: [u64; 12] =
        [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1u64;
    for &md in &month_days {
        if remaining < md {
            break;
        }
        remaining -= md;
        month += 1;
    }
    (year, month, remaining + 1)
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
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
        let _: fn(&Path, Option<u64>, Option<&str>, Option<&str>, bool, u64) -> Result<()> = run;
    }

    // ── Unit: format_unix_ms ────────────────────────────────────────────

    #[test]
    fn format_unix_ms_zero_returns_dash() {
        assert_eq!(format_unix_ms(0), "\u{2014}");
    }

    #[test]
    fn format_unix_ms_negative_returns_dash() {
        assert_eq!(format_unix_ms(-1), "\u{2014}");
    }

    #[test]
    fn format_unix_ms_known_epoch() {
        // 2024-01-01 00:00:00 UTC = 1704067200000 ms
        assert_eq!(format_unix_ms(1_704_067_200_000i64), "2024-01-01 00:00:00");
    }

    #[test]
    fn format_unix_ms_structure() {
        let s = format_unix_ms(1_750_000_496_000i64);
        assert_eq!(s.len(), 19);
        assert_eq!(&s[4..5], "-");
        assert_eq!(&s[7..8], "-");
    }

    // ── Unit: is_leap ───────────────────────────────────────────────────

    #[test]
    fn is_leap_correct() {
        assert!(is_leap(2000));
        assert!(is_leap(2024));
        assert!(!is_leap(1900));
        assert!(!is_leap(2025));
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

    /// Spawn a fake daemon that serves one canned JSON response line.
    fn mock_server_once(socket_path: &Path, response_json: &'static str) {
        let listener = UnixListener::bind(socket_path).unwrap();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = String::new();
                std::io::BufRead::read_line(
                    &mut std::io::BufReader::new(&stream),
                    &mut buf,
                )
                .unwrap();
                stream.write_all(response_json.as_bytes()).unwrap();
                stream.write_all(b"\n").unwrap();
            }
        });
    }

    /// Spawn a fake daemon that serves two sequential connections.
    fn mock_server_two(
        socket_path: &Path,
        first: &'static str,
        second: &'static str,
    ) {
        let listener = UnixListener::bind(socket_path).unwrap();
        thread::spawn(move || {
            for resp in [first, second] {
                if let Ok((mut stream, _)) = listener.accept() {
                    let mut buf = String::new();
                    std::io::BufRead::read_line(
                        &mut std::io::BufReader::new(&stream),
                        &mut buf,
                    )
                    .unwrap();
                    stream.write_all(resp.as_bytes()).unwrap();
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
            r#"{"id":"1","ok":true,"data":{"items":[],"total":0}}"#,
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
            r#"{"id":"1","ok":true,"data":{"items":[{"id":"aaaa-1111","content_type":"text","is_sensitive":false,"wall_time":1704067200000,"lamport_ts":1},{"id":"bbbb-2222","content_type":"text","is_sensitive":true,"wall_time":1704070800000,"lamport_ts":2}],"total":2}}"#,
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
            r#"{"id":"1","ok":true,"data":{"id":"test-uuid","found":true}}"#,
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
            r#"{"id":"1","ok":false,"error":"item not found: bad-id"}"#,
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
            r#"{"id":"1","ok":true,"data":{"items":[{"id":"first-uuid","content_type":"text","is_sensitive":false,"wall_time":1704067200000,"lamport_ts":1}],"total":1}}"#,
            r#"{"id":"1","ok":true,"data":{"id":"first-uuid","found":true}}"#,
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
            r#"{"id":"1","ok":true,"data":{"items":[{"id":"only-one","content_type":"text","is_sensitive":false,"wall_time":1704067200000,"lamport_ts":1}],"total":1}}"#,
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
            r#"{"id":"1","ok":true,"data":{"items":[{"id":"match-uuid","content_type":"text","is_sensitive":false,"wall_time":1704067200000,"lamport_ts":1}]}}"#,
            r#"{"id":"1","ok":true,"data":{"id":"match-uuid","found":true}}"#,
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
            r#"{"id":"1","ok":false,"error":"missing param: query"}"#,
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
            r#"{"id":"1","ok":false,"error":"db corrupted"}"#,
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
            r#"{"id":"1","ok":true,"data":{"items":[],"total":0}}"#,
        );
        let items = fetch_history(&sock, 50).unwrap();
        assert!(items.is_empty());
    }
}

use anyhow::Result;
use crate::ipc::IpcClient;
use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;
use std::thread;

pub fn run(socket_path: &Path, interval_ms: u64) -> Result<()> {
    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut first_run = true;

    eprintln!("watching clipboard (Ctrl+C to stop)...");

    loop {
        match poll_once(socket_path, &mut seen_ids, first_run) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("watch: {e}");
                // If daemon not running, retry after interval
            }
        }
        first_run = false;
        thread::sleep(Duration::from_millis(interval_ms));
    }
}

fn poll_once(socket_path: &Path, seen_ids: &mut HashSet<String>, silent_first: bool) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = serde_json::json!({"id": "1", "method": "list", "params": {"limit": 20, "offset": 0}});
    let resp = client.call(&req)?;

    if !resp.ok {
        return Err(anyhow::anyhow!("{}", resp.error.unwrap_or_default()));
    }

    let items = resp.data
        .as_ref()
        .and_then(|d| d["items"].as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[]);

    for item in items {
        let id = item["id"].as_str().unwrap_or("?");
        if seen_ids.contains(id) {
            continue;
        }
        seen_ids.insert(id.to_string());
        if silent_first {
            continue; // populate seen on first run, don't print
        }
        let content_type = item["content_type"].as_str().unwrap_or("?");
        let wall_time = item["wall_time"].as_i64().unwrap_or(0);
        let sensitive = item["is_sensitive"].as_bool().unwrap_or(false);
        let sens = if sensitive { " [sensitive]" } else { "" };
        let ts = format_ts(wall_time);
        println!("+ {ts}  {content_type:<6}  {id}{sens}");
    }
    Ok(())
}

fn format_ts(ms: i64) -> String {
    let secs = ms / 1000;
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    format!("{h:02}:{m:02}:{s:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn run_signature_compiles() {
        let _: fn(&Path, u64) -> Result<()> = run;
    }
    #[test]
    fn format_ts_midnight() {
        assert_eq!(format_ts(0), "00:00:00");
    }
    #[test]
    fn format_ts_noon() {
        assert_eq!(format_ts(43_200_000), "12:00:00");
    }
}

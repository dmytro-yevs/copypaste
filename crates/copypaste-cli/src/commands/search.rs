use anyhow::Result;
use crate::ipc::IpcClient;
use std::path::Path;

pub fn run(socket_path: &Path, query: &str, limit: u64) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = serde_json::json!({
        "id": "1",
        "method": "search",
        "params": {"query": query, "limit": limit}
    });
    let resp = client.call(&req)?;

    if !resp.ok {
        eprintln!("error: {}", resp.error.unwrap_or_default());
        std::process::exit(1);
    }

    let items = resp.data
        .as_ref()
        .and_then(|d| d["items"].as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[]);

    if items.is_empty() {
        println!("no results for {:?}", query);
        return Ok(());
    }

    for item in items {
        let id = item["id"].as_str().unwrap_or("?");
        let content_type = item["content_type"].as_str().unwrap_or("?");
        let wall_time = item["wall_time"].as_i64().unwrap_or(0);
        let sensitive = item["is_sensitive"].as_bool().unwrap_or(false);
        let ts = format_unix_ms(wall_time);
        let sens_marker = if sensitive { " [sensitive]" } else { "" };
        println!("{ts}  {content_type:<6}  {id}{sens_marker}");
    }
    Ok(())
}

fn format_unix_ms(ms: i64) -> String {
    let secs = ms / 1000;
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;
    let y400 = days / 146097;
    let mut rem = days % 146097;
    let y100 = (rem / 36524).min(3);
    rem -= y100 * 36524;
    let y4 = rem / 1461;
    rem %= 1461;
    let y1 = (rem / 365).min(3);
    rem -= y1 * 365;
    let year = y400 * 400 + y100 * 100 + y4 * 4 + y1 + 1970;
    let leap = (y1 == 3 && !(y100 == 3 && y4 == 0)) as i64;
    let months = [31i64,28+leap,31,30,31,30,31,31,30,31,30,31];
    let mut month = 1i64;
    let mut day = rem + 1;
    for mlen in &months {
        if day <= *mlen { break; }
        day -= mlen;
        month += 1;
    }
    format!("{year:04}-{month:02}-{day:02} {h:02}:{m:02}:{s:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_signature_compiles() {
        let _: fn(&std::path::Path, &str, u64) -> Result<()> = run;
    }

    #[test]
    fn format_unix_ms_epoch() {
        let s = format_unix_ms(0);
        assert!(s.starts_with("1970-01-01"));
    }
}

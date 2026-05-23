use anyhow::Result;
use crate::commands::common::exit_on_err;
use crate::ipc::IpcClient;
use std::path::Path;

pub fn run(socket_path: &Path, limit: u64, offset: u64) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = serde_json::json!({
        "id": "1",
        "method": "list",
        "params": {"limit": limit, "offset": offset}
    });
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    let data = resp.data.unwrap_or(serde_json::Value::Null);
    let total = data["total"].as_u64().unwrap_or(0);
    let items = data["items"].as_array().cloned().unwrap_or_default();

    if items.is_empty() {
        println!("No items (total: {})", total);
        return Ok(());
    }

    println!("{:<38}  {:<12}  {:<10}  TIME (UTC)", "ID", "TYPE", "SENSITIVE");
    println!("{}", "-".repeat(90));

    for item in &items {
        let id = item["id"].as_str().unwrap_or("?");
        let ctype = item["content_type"].as_str().unwrap_or("?");
        let sensitive = if item["is_sensitive"].as_bool().unwrap_or(false) {
            "yes"
        } else {
            "no"
        };
        let ts = item["wall_time"].as_i64().unwrap_or(0);
        let time_str = format_unix_ms(ts);
        println!("{:<38}  {:<12}  {:<10}  {}", id, ctype, sensitive, time_str);
    }

    println!();
    println!(
        "Showing {}-{} of {} total",
        offset + 1,
        offset + items.len() as u64,
        total
    );

    Ok(())
}

/// Format Unix epoch milliseconds as "YYYY-MM-DD HH:MM:SS" using std only.
fn format_unix_ms(ms: i64) -> String {
    if ms <= 0 {
        return "\u{2014}".to_string(); // em dash
    }
    let secs = (ms / 1000) as u64;
    let (y, mo, d, h, mi, s) = secs_to_ymd_hms(secs);
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, mo, d, h, mi, s)
}

fn secs_to_ymd_hms(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let ss = secs % 60;
    let mins = secs / 60;
    let mi = mins % 60;
    let hours = mins / 60;
    let h = hours % 24;
    let days = hours / 24;
    let (y, mo, d) = days_to_ymd(days);
    (y, mo, d, h, mi, ss)
}

fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    let mut remaining = days;
    let mut year = 1970u64;

    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_signature_compiles() {
        let _: fn(&Path, u64, u64) -> Result<()> = run;
    }

    #[test]
    fn format_unix_ms_zero_returns_dash() {
        assert_eq!(format_unix_ms(0), "\u{2014}");
    }

    #[test]
    fn format_unix_ms_known_date() {
        // 2024-01-01 00:00:00 UTC = 1704067200 seconds = 1704067200000 ms
        let ms = 1_704_067_200_000i64;
        let s = format_unix_ms(ms);
        assert_eq!(s, "2024-01-01 00:00:00");
    }

    #[test]
    fn format_unix_ms_structure() {
        // 2025-06-15 approx — just verify structure
        let ms = 1_750_000_496_000i64;
        let s = format_unix_ms(ms);
        assert_eq!(s.len(), 19);
        assert_eq!(&s[4..5], "-");
        assert_eq!(&s[7..8], "-");
        assert_eq!(&s[10..11], " ");
        assert_eq!(&s[13..14], ":");
        assert_eq!(&s[16..17], ":");
    }

    #[test]
    fn is_leap_year_correct() {
        assert!(is_leap(2000));
        assert!(is_leap(2024));
        assert!(!is_leap(1900));
        assert!(!is_leap(2023));
    }
}

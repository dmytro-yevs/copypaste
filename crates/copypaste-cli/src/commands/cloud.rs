//! `copypaste cloud …` — Supabase cloud-sync setup and diagnostics.
//!
//! These subcommands collapse the previously manual, multi-step Supabase
//! configuration into one-line operations over the daemon IPC socket:
//!
//! - `setup` — store the project URL + anon key in the daemon config (the same
//!   `config.json` the desktop UI writes). No env vars or daemon restart
//!   required for the credentials.
//! - `status` — show the current cloud-sync state (configured / signed in /
//!   passphrase set / last sync).
//! - `test` — validate the configured credentials end-to-end against Supabase
//!   and print a precise, actionable diagnostic.
//! - `setup-sql` — print the idempotent provisioning SQL (schema + RLS) so it
//!   can be pasted once into the Supabase SQL Editor
//!   (`copypaste cloud setup-sql | pbcopy`).

use crate::commands::common::exit_on_err;
use crate::ipc::IpcClient;
use anyhow::{anyhow, Result};
use std::path::Path;

/// Idempotent schema + RLS provisioning SQL, embedded so the CLI always emits
/// exactly the file shipped in the repo. Kept in sync via `include_str!`.
const SETUP_SQL: &str = include_str!("../../../../docs/supabase/setup.sql");

/// Store the Supabase project URL and anon key in the daemon config.
///
/// Reads the existing config first and merges, so unrelated settings
/// (e.g. `p2p_enabled`) are preserved rather than clobbered. Validates that the
/// URL is HTTPS before sending — the daemon refuses plain http, and failing
/// here gives a clearer message than a silent no-op later.
pub fn setup(socket_path: &Path, url: &str, anon_key: &str) -> Result<()> {
    let url = url.trim().trim_end_matches('/');
    let anon_key = anon_key.trim();

    if !url.to_ascii_lowercase().starts_with("https://") {
        return Err(anyhow!(
            "Supabase URL must start with https:// (got {url}). Cloud sync refuses plain http."
        ));
    }
    if anon_key.is_empty() {
        return Err(anyhow!("anon key must not be empty"));
    }

    // Read-merge-write: fetch current config so we don't drop other fields.
    let mut cfg = {
        let mut client = IpcClient::connect(socket_path)?;
        let req = serde_json::json!({ "id": "1", "method": "get_config", "params": {} });
        let resp = client.call(&req)?;
        exit_on_err(&resp);
        resp.data.unwrap_or_else(|| serde_json::json!({}))
    };

    if let Some(obj) = cfg.as_object_mut() {
        obj.insert("supabase_url".into(), serde_json::json!(url));
        obj.insert("supabase_anon_key".into(), serde_json::json!(anon_key));
    } else {
        cfg = serde_json::json!({
            "supabase_url": url,
            "supabase_anon_key": anon_key,
        });
    }

    let mut client = IpcClient::connect(socket_path)?;
    let req = serde_json::json!({ "id": "1", "method": "set_config", "params": cfg });
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    println!("Supabase credentials saved.");
    println!("Next:");
    println!("  1. copypaste cloud setup-sql | pbcopy   # provision schema + RLS in Supabase");
    println!("  2. copypaste cloud test                 # verify the connection");
    Ok(())
}

/// Print the current cloud-sync status reported by the daemon.
pub fn status(socket_path: &Path) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = serde_json::json!({ "id": "1", "method": "get_sync_status", "params": {} });
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    let data = resp.data.unwrap_or_else(|| serde_json::json!({}));
    let yn = |b: bool| if b { "yes" } else { "no" };
    let get_bool = |k: &str| data.get(k).and_then(|v| v.as_bool()).unwrap_or(false);

    println!(
        "Supabase configured: {}",
        yn(get_bool("supabase_configured"))
    );
    if let Some(url) = data.get("supabase_url").and_then(|v| v.as_str()) {
        println!("Project URL:         {url}");
    }
    println!("Signed in:           {}", yn(get_bool("signed_in")));
    if let Some(email) = data.get("email").and_then(|v| v.as_str()) {
        println!("Account:             {email}");
    }
    println!("Passphrase set:      {}", yn(get_bool("passphrase_set")));
    match data.get("last_sync_ms").and_then(|v| v.as_i64()) {
        Some(ms) if ms > 0 => println!("Last sync (epoch ms): {ms}"),
        _ => println!("Last sync:           never"),
    }
    Ok(())
}

/// Run the daemon-side connection diagnostic and print the result.
///
/// Exits non-zero when the daemon reports the connection is not ready, so this
/// is scriptable (`copypaste cloud test && echo ok`).
pub fn test(socket_path: &Path) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = serde_json::json!({ "id": "1", "method": "cloud_test_connection", "params": {} });
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    let data = resp.data.unwrap_or_else(|| serde_json::json!({}));
    let ok = data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
    let message = data
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("no diagnostic message returned");

    if ok {
        println!("OK: {message}");
        Ok(())
    } else {
        // Non-zero exit for scripting; print the actionable message to stderr.
        Err(anyhow!("{message}"))
    }
}

/// Print the idempotent provisioning SQL to stdout.
pub fn setup_sql() -> Result<()> {
    print!("{SETUP_SQL}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_signatures_compile() {
        let _: fn(&Path, &str, &str) -> Result<()> = setup;
        let _: fn(&Path) -> Result<()> = status;
        let _: fn(&Path) -> Result<()> = test;
        let _: fn() -> Result<()> = setup_sql;
    }

    /// The embedded SQL must contain both the table DDL and the RLS policy so
    /// `setup-sql` provisions everything in one paste.
    #[test]
    fn embedded_sql_has_schema_and_rls() {
        assert!(
            SETUP_SQL.contains("create table if not exists public.clipboard_items"),
            "embedded SQL must create the clipboard_items table"
        );
        assert!(
            SETUP_SQL.contains("enable row level security"),
            "embedded SQL must enable RLS"
        );
        assert!(
            SETUP_SQL.contains("clipboard_items_insert_own"),
            "embedded SQL must define the insert RLS policy"
        );
    }
}

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

/// Store the Supabase project URL, anon key, and account credentials in the
/// daemon config.
///
/// The email/password are required because the provisioning SQL grants table
/// access only to the `authenticated` role (RLS `using (user_id = auth.uid())`).
/// Without them the daemon would authenticate as the public `anon` role and
/// every REST insert/select would be rejected by RLS — sync would silently
/// fail. They are persisted into the same `0600` `config.json` as the anon key
/// so the documented one-command flow yields working authenticated sync with
/// no env vars or daemon restart.
///
/// Reads the existing config first and merges, so unrelated settings
/// (e.g. `p2p_enabled`) are preserved rather than clobbered. Validates that the
/// URL is HTTPS before sending — the daemon refuses plain http, and failing
/// here gives a clearer message than a silent no-op later.
///
/// Both the anon key and password are resolved without ever requiring a plain
/// argv flag value: callers may pass `None` and we read the matching env var
/// (`SUPABASE_ANON_KEY` / `SUPABASE_PASSWORD`) or prompt on stdin, avoiding
/// shell-history and process-list (`ps`) leakage. An explicit flag value is
/// still accepted as a deprecated fallback.
pub fn setup(
    socket_path: &Path,
    url: &str,
    anon_key: Option<String>,
    email: &str,
    password: Option<String>,
) -> Result<()> {
    let url = url.trim().trim_end_matches('/');
    let email = email.trim();

    if !url.to_ascii_lowercase().starts_with("https://") {
        return Err(anyhow!(
            "Supabase URL must start with https:// (got {url}). Cloud sync refuses plain http."
        ));
    }
    if email.is_empty() {
        return Err(anyhow!("email must not be empty"));
    }

    // Resolve the anon key without leaking it via `ps`: explicit --anon-key
    // (deprecated) → SUPABASE_ANON_KEY env → no-echo interactive prompt.
    let anon_key = resolve_secret(
        anon_key,
        "SUPABASE_ANON_KEY",
        "Supabase anon/public API key: ",
    )?;
    let anon_key = anon_key.trim();
    if anon_key.is_empty() {
        return Err(anyhow!("anon key must not be empty"));
    }

    // Resolve the password without leaking it into shell history: explicit
    // --password arg (discouraged) → SUPABASE_PASSWORD env → interactive prompt.
    let password = resolve_secret(password, "SUPABASE_PASSWORD", "Supabase account password: ")?;
    if password.trim().is_empty() {
        return Err(anyhow!("password must not be empty"));
    }

    // Read-merge-write: fetch current config so we don't drop other fields.
    let mut cfg = {
        let mut client = IpcClient::connect(socket_path)?;
        let req = IpcClient::build_request("1", "get_config", serde_json::json!({}));
        let resp = client.call(&req)?;
        exit_on_err(&resp);
        resp.data.unwrap_or_else(|| serde_json::json!({}))
    };

    if let Some(obj) = cfg.as_object_mut() {
        obj.insert("supabase_url".into(), serde_json::json!(url));
        obj.insert("supabase_anon_key".into(), serde_json::json!(anon_key));
        obj.insert("supabase_email".into(), serde_json::json!(email));
        obj.insert("supabase_password".into(), serde_json::json!(password));
    } else {
        cfg = serde_json::json!({
            "supabase_url": url,
            "supabase_anon_key": anon_key,
            "supabase_email": email,
            "supabase_password": password,
        });
    }

    let mut client = IpcClient::connect(socket_path)?;
    let req = IpcClient::build_request("1", "set_config", cfg);
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    println!("Supabase credentials saved (URL, anon key, email/password).");
    println!("Next:");
    println!("  1. copypaste cloud setup-sql | pbcopy   # provision schema + RLS in Supabase");
    println!("  2. copypaste cloud test                 # verify the connection");
    Ok(())
}

/// Resolve a secret value (anon key or password) without leaking it via the
/// process list or shell history: explicit value (deprecated argv flag) →
/// `env_var` → interactive no-echo prompt.
///
/// The interactive path uses `rpassword::prompt_password` which disables
/// terminal echo in-process (via termios on Unix) so the secret is never
/// visible on-screen or in terminal scroll-back. Echo is always restored by
/// rpassword even when the user hits Ctrl-C or an error occurs.
///
/// Non-TTY path (pipes, CI): `rpassword` falls back to reading stdin without
/// echo-disabling; callers that truly cannot provide a TTY should set the env
/// var instead.
fn resolve_secret(explicit: Option<String>, env_var: &str, prompt: &str) -> Result<String> {
    if let Some(v) = explicit {
        return Ok(v);
    }
    if let Ok(v) = std::env::var(env_var) {
        if !v.is_empty() {
            return Ok(v);
        }
    }
    // rpassword::prompt_password disables terminal echo in-process (termios)
    // and always restores it on return, even on error. This prevents the secret
    // from appearing in the terminal or in scroll-back history.
    let value = rpassword::prompt_password(prompt)
        .map_err(|e| anyhow::anyhow!("failed to read secret from terminal: {e}"))?;
    Ok(value)
}

/// Print the current cloud-sync status reported by the daemon.
pub fn status(socket_path: &Path) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = IpcClient::build_request("1", "get_sync_status", serde_json::json!({}));
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
    let req = IpcClient::build_request("1", "cloud_test_connection", serde_json::json!({}));
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
        // Reference each entry point so a signature change is caught at compile
        // time. `setup` takes `Option<String>` for the anon key and password so
        // neither secret has to be passed on the (process-list-visible) argv.
        let _ = setup;
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

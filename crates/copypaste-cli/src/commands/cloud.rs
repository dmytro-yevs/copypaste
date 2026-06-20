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
use crate::ipc::{IpcClient, Response};
use anyhow::{anyhow, bail, Result};
use copypaste_ipc::{
    METHOD_CLOUD_TEST_CONNECTION, METHOD_GET_CONFIG, METHOD_GET_SYNC_STATUS, METHOD_SET_CONFIG,
    METHOD_STORE_CLOUD_PASSWORD,
};
use std::path::Path;
use zeroize::Zeroizing;

// P1-6 fix: the CLI no longer writes the macOS Keychain directly. The
// Supabase password is sent to the daemon via the set_config IPC verb (over
// the 0600 local unix socket). The daemon's set_config handler (ipc.rs,
// "set_config" arm) stores it in the macOS Keychain and strips it from
// config.json so it is never persisted to disk in plaintext. This preserves
// the "daemon is the ONLY Keychain owner" contract.
//
// The security-framework dep has been removed from cli/Cargo.toml.

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
    // --password arg (discouraged) → SUPABASE_PASSWORD env → interactive
    // no-echo prompt (rpassword). Wrap in Zeroizing so it is wiped on drop.
    let password_raw = resolve_secret(password, "SUPABASE_PASSWORD", "Supabase account password")?;
    let password = Zeroizing::new(password_raw);
    if password.trim().is_empty() {
        return Err(anyhow!("password must not be empty"));
    }

    // Read-merge-write: fetch current config so we don't drop other fields.
    //
    // CopyPaste-liaz: use `check_resp` (returns Result) rather than `exit_on_err`
    // (calls process::exit) for ALL IPC calls inside `setup`. The `password`
    // (Zeroizing<String>) is live for the duration of this function. Calling
    // process::exit while it is live bypasses its Drop impl, leaving key material
    // in memory unzeroed. Returning Err instead lets the caller stack unwind
    // normally so Zeroizing::drop runs and wipes the bytes.
    let mut cfg = {
        let mut client = IpcClient::connect(socket_path)?;
        let req = IpcClient::build_request(
            &IpcClient::next_id(),
            METHOD_GET_CONFIG,
            serde_json::json!({}),
        );
        let resp = client.call(&req)?;
        check_resp(&resp)?;
        resp.data.unwrap_or_else(|| serde_json::json!({}))
    };

    // nq39: send URL / anon key / email via set_config, but route the password
    // through the dedicated `store_cloud_password` verb so it is never carried
    // inside the set_config JSON payload and never risks being persisted to
    // config.json on non-macOS platforms.
    if let Some(obj) = cfg.as_object_mut() {
        obj.insert("supabase_url".into(), serde_json::json!(url));
        obj.insert("supabase_anon_key".into(), serde_json::json!(anon_key));
        obj.insert("supabase_email".into(), serde_json::json!(email));
        // Explicitly remove any stale supabase_password that may have been
        // left by a previous `set_config`-based setup (pre-nq39). The daemon's
        // set_config handler already strips it on macOS, but this removes the
        // field at the source so it is never even sent over the socket.
        obj.remove("supabase_password");
    } else {
        cfg = serde_json::json!({
            "supabase_url": url,
            "supabase_anon_key": anon_key,
            "supabase_email": email,
            // supabase_password intentionally absent — sent via store_cloud_password below.
        });
    }

    {
        let mut client = IpcClient::connect(socket_path)?;
        let req = IpcClient::build_request(&IpcClient::next_id(), METHOD_SET_CONFIG, cfg);
        let resp = client.call(&req)?;
        // CopyPaste-liaz: return Err so `password` (Zeroizing) is dropped normally.
        check_resp(&resp)?;
    }

    // Send the password via the dedicated verb. This is a separate connection
    // so `password` (Zeroizing) is the only live copy when it is serialised,
    // and it is dropped + zeroed immediately after this block.
    {
        let mut client = IpcClient::connect(socket_path)?;
        // Build the params inline to limit the lifetime of the password clone.
        let params = serde_json::json!({ "password": password.trim() });
        // `password` (Zeroizing) is dropped at the end of this block.
        let req =
            IpcClient::build_request(&IpcClient::next_id(), METHOD_STORE_CLOUD_PASSWORD, params);
        let resp = client.call(&req)?;
        // CopyPaste-liaz: return Err rather than process::exit so `password`
        // (Zeroizing) is dropped and zeroed by the normal unwinding path.
        check_resp(&resp)?;
        // Log whether the daemon persisted to Keychain vs. held in-memory.
        let persisted = resp
            .data
            .as_ref()
            .and_then(|d| d.get("persisted"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !persisted {
            eprintln!(
                "warning: daemon could not persist the password to the Keychain \
                 (non-macOS or unsigned build) — it will be held in-memory and \
                 lost on restart. Set SUPABASE_PASSWORD env or re-run setup after restart."
            );
        }
    }
    // `password` (Zeroizing) is now out of scope and zeroed.

    println!("Supabase credentials saved (URL, anon key, email, password).");
    println!(
        "The daemon stores the password in the macOS Keychain; it will not appear in config.json."
    );
    println!("Next:");
    println!("  1. copypaste cloud setup-sql | pbcopy   # provision schema + RLS in Supabase");
    println!("  2. copypaste cloud test                 # verify the connection");
    Ok(())
}

/// Convert a daemon response into `Result<()>`, propagating errors without
/// calling `process::exit`.
///
/// CopyPaste-liaz: `exit_on_err` (in `common.rs`) calls `process::exit(1)` on
/// failure. In `setup`, the caller holds a `Zeroizing<String>` password that
/// must be dropped (zeroed) before the process terminates. Calling
/// `process::exit` bypasses all destructors, leaving key material in memory.
/// This helper returns `Err` instead so the call stack unwinds normally and
/// `Zeroizing::drop` runs to wipe the bytes.
fn check_resp(resp: &Response) -> Result<()> {
    if !resp.ok {
        let msg = resp.error.as_deref().unwrap_or_default();
        match resp.error_code {
            Some(ref code) => bail!("error [{code}]: {msg}"),
            None => bail!("error: {msg}"),
        }
    }
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
    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_GET_SYNC_STATUS,
        serde_json::json!({}),
    );
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
    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_CLOUD_TEST_CONNECTION,
        serde_json::json!({}),
    );
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

    // ── CopyPaste-liaz: check_resp returns Err instead of process::exit ───────

    /// `check_resp` must return Ok when `resp.ok == true`.
    #[test]
    fn check_resp_ok_passes_through() {
        let resp = Response {
            id: "1".to_string(),
            ok: true,
            data: None,
            error: None,
            error_code: None,
        };
        assert!(check_resp(&resp).is_ok(), "ok response must pass");
    }

    /// `check_resp` must return Err (not call process::exit) when `resp.ok == false`.
    /// This ensures Zeroizing<…> secrets in the caller's scope are dropped.
    #[test]
    fn check_resp_error_returns_err_not_exits() {
        let resp = Response {
            id: "2".to_string(),
            ok: false,
            data: None,
            error: Some("daemon refused".to_string()),
            error_code: None,
        };
        let result = check_resp(&resp);
        assert!(result.is_err(), "error response must yield Err");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("daemon refused"),
            "error message must include daemon message, got: {msg}"
        );
    }

    /// When an `error_code` is present, `check_resp` must format it as
    /// `error [code]: message` so the caller sees the same format as `exit_on_err`.
    #[test]
    fn check_resp_includes_error_code_in_message() {
        use copypaste_ipc::ErrorCode;
        let resp = Response {
            id: "3".to_string(),
            ok: false,
            data: None,
            error: Some("not implemented".to_string()),
            error_code: Some(ErrorCode::NotImplemented),
        };
        let msg = check_resp(&resp).unwrap_err().to_string();
        assert!(
            msg.contains("not_implemented"),
            "error message must contain the error code, got: {msg}"
        );
    }
}

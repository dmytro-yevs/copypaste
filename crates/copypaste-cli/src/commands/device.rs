//! `copypaste device` — list and manage paired devices.
//!
//! Exposes the daemon's peer-management IPC verbs (`list_peers`, `revoke_peer`,
//! `revoke_all_peers`) over a `device` subcommand surface so users can inspect
//! and remove paired devices from the CLI without opening the macOS UI.
//!
//! ## Subcommands
//!
//! - `copypaste device list` — show all paired peers (online/offline, last seen)
//! - `copypaste device revoke <fingerprint>` — revoke a specific peer
//! - `copypaste device revoke-all` — revoke every paired peer at once
//!
//! ## Exit codes
//! - 0 — operation succeeded
//! - 1 — daemon not running, IPC error, or daemon returned an error

use anyhow::{anyhow, Context, Result};
use copypaste_ipc::{
    METHOD_LIST_PEERS, METHOD_REVOKE_ALL_PEERS, METHOD_REVOKE_AND_ROTATE, METHOD_REVOKE_PEER,
    METHOD_UNPAIR_PEER,
};
use std::path::Path;

use crate::commands::common::exit_on_err;
use crate::commands::sync::{check_resp, resolve_passphrase};
use crate::ipc::IpcClient;
use zeroize::Zeroizing;

/// List all currently paired devices (peers).
pub fn run_list(socket_path: &Path) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)
        .with_context(|| format!("daemon is not running (socket: {})", socket_path.display()))?;

    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_LIST_PEERS,
        serde_json::json!({}),
    );
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    let data = resp
        .data
        .as_ref()
        .ok_or_else(|| anyhow!("daemon returned no data for list_peers"))?;

    let peers = data["peers"]
        .as_array()
        .ok_or_else(|| anyhow!("daemon response missing 'peers' array"))?;

    if peers.is_empty() {
        println!("No paired devices.");
        return Ok(());
    }

    println!("{:<20} {:<48} {:<10}", "Name", "Fingerprint", "Status");
    println!("{}", "-".repeat(82));
    for peer in peers {
        let name = peer["device_name"]
            .as_str()
            .or_else(|| peer["name"].as_str())
            .unwrap_or("(unknown)");
        let fp = peer["fingerprint"].as_str().unwrap_or("?");
        let online = peer["online"].as_bool().unwrap_or(false);
        let status = if online { "online" } else { "offline" };
        println!("{:<20} {:<48} {:<10}", name, fp, status);
    }

    Ok(())
}

/// Revoke a single paired peer by fingerprint.
pub fn run_revoke(socket_path: &Path, fingerprint: &str, force: bool) -> Result<()> {
    if !force {
        eprintln!("WARNING: This will remove the paired device with fingerprint:");
        eprintln!("  {fingerprint}");
        eprintln!("The device will no longer be able to sync with this one.");
        eprint!("Type 'yes' to confirm: ");

        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .context("failed to read confirmation from stdin")?;
        if input.trim() != "yes" {
            return Err(anyhow!("aborted: pass --force to skip this prompt"));
        }
    }

    let mut client = IpcClient::connect(socket_path)
        .with_context(|| format!("daemon is not running (socket: {})", socket_path.display()))?;

    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_REVOKE_PEER,
        serde_json::json!({ "fingerprint": fingerprint }),
    );
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    if let Some(data) = &resp.data {
        let revoked_at = data["revoked_at"].as_i64().unwrap_or(0);
        if revoked_at > 0 {
            println!("Device revoked (revoked_at = {revoked_at}).");
        } else {
            println!("Device revoked.");
        }
    } else {
        println!("Device revoked.");
    }

    Ok(())
}

/// Revoke all paired peers at once.
pub fn run_revoke_all(socket_path: &Path, force: bool) -> Result<()> {
    if !force {
        eprintln!("WARNING: This will revoke ALL paired devices.");
        eprintln!("No devices will be able to sync with this one until re-paired.");
        eprint!("Type 'yes' to confirm: ");

        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .context("failed to read confirmation from stdin")?;
        if input.trim() != "yes" {
            return Err(anyhow!("aborted: pass --force to skip this prompt"));
        }
    }

    let mut client = IpcClient::connect(socket_path)
        .with_context(|| format!("daemon is not running (socket: {})", socket_path.display()))?;

    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_REVOKE_ALL_PEERS,
        serde_json::json!({}),
    );
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    let revoked = resp
        .data
        .as_ref()
        .and_then(|d| d["revoked"].as_u64())
        .unwrap_or(0);

    if revoked == 0 {
        println!("No paired devices to revoke.");
    } else {
        println!("Revoked {revoked} device(s).");
    }

    Ok(())
}

/// Remove a paired peer from the trust store without logging a revocation
/// timestamp (`unpair_peer`).
///
/// Items the device previously synced remain in history. Use
/// `copypaste device revoke` for a stronger revoke that logs a revocation
/// timestamp and blocks future reconnects.
pub fn run_unpair(socket_path: &Path, fingerprint: &str, force: bool) -> Result<()> {
    if !force {
        eprintln!("WARNING: This will remove the paired device with fingerprint:");
        eprintln!("  {fingerprint}");
        eprintln!("The device will no longer be able to sync with this one.");
        eprint!("Type 'yes' to confirm: ");

        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .context("failed to read confirmation from stdin")?;
        if input.trim() != "yes" {
            return Err(anyhow!("aborted: pass --force to skip this prompt"));
        }
    }

    let mut client = IpcClient::connect(socket_path)
        .with_context(|| format!("daemon is not running (socket: {})", socket_path.display()))?;

    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_UNPAIR_PEER,
        serde_json::json!({ "fingerprint": fingerprint }),
    );
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    println!("Device unpaired.");
    Ok(())
}

/// Revoke a paired peer AND rotate the sync key in one atomic operation.
///
/// The daemon derives the new key from `passphrase` first — a bad passphrase
/// fails before any state is mutated. It then removes the peer and rotates
/// the key. This ensures the revoked device cannot decrypt new items even if
/// it captured the old key material.
///
/// Reads the new passphrase without terminal echo and wraps it in
/// `Zeroizing<String>` so the bytes are wiped on drop. Never calls
/// `process::exit` while the secret is live (see CopyPaste-liaz).
pub fn run_revoke_and_rotate(
    socket_path: &Path,
    fingerprint: &str,
    passphrase: Option<String>,
    force: bool,
) -> Result<()> {
    if !force {
        eprintln!("WARNING: This will revoke the device with fingerprint:");
        eprintln!("  {fingerprint}");
        eprintln!("AND rotate the sync key. Previously synced items remain in history.");
        eprint!("Type 'yes' to confirm: ");

        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .context("failed to read confirmation from stdin")?;
        if input.trim() != "yes" {
            return Err(anyhow!("aborted: pass --force to skip this prompt"));
        }
    }

    let raw = resolve_passphrase(
        passphrase,
        "COPYPASTE_SYNC_PASSPHRASE",
        "New sync passphrase for rotation: ",
    )?;
    let passphrase_z = Zeroizing::new(raw);
    if passphrase_z.trim().is_empty() {
        // CopyPaste-liaz: return Err so `passphrase_z` is dropped + zeroed.
        return Err(anyhow!("passphrase must not be empty"));
    }

    let params = serde_json::json!({
        "fingerprint": fingerprint,
        "passphrase": passphrase_z.trim(),
    });
    let resp = {
        let mut client = IpcClient::connect(socket_path).with_context(|| {
            format!("daemon is not running (socket: {})", socket_path.display())
        })?;
        let req = IpcClient::build_request(&IpcClient::next_id(), METHOD_REVOKE_AND_ROTATE, params);
        client.call(&req)?
    };
    // `passphrase_z` (Zeroizing) is dropped + zeroed here, before any call
    // that could call process::exit.
    drop(passphrase_z);

    // CopyPaste-liaz: use check_resp (returns Err) not exit_on_err (calls
    // process::exit) so the Zeroizing passphrase is dropped by the normal
    // unwind path. By the time we reach this point `passphrase_z` is already
    // dropped above, but keeping the pattern consistent guards future refactors.
    check_resp(&resp)?;

    let revoked_at = resp
        .data
        .as_ref()
        .and_then(|d| d["revoked_at"].as_i64())
        .unwrap_or(0);
    let rotated = resp
        .data
        .as_ref()
        .and_then(|d| d["rotated"].as_bool())
        .unwrap_or(true);

    if revoked_at > 0 {
        println!("Device revoked (revoked_at = {revoked_at}).");
    } else {
        println!("Device revoked.");
    }
    if rotated {
        println!("Sync key rotated. The revoked device can no longer decrypt new items.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_list_signature_compiles() {
        let _: fn(&Path) -> Result<()> = run_list;
    }

    #[test]
    fn run_revoke_signature_compiles() {
        let _: fn(&Path, &str, bool) -> Result<()> = run_revoke;
    }

    #[test]
    fn run_revoke_all_signature_compiles() {
        let _: fn(&Path, bool) -> Result<()> = run_revoke_all;
    }

    #[test]
    fn run_unpair_signature_compiles() {
        let _: fn(&Path, &str, bool) -> Result<()> = run_unpair;
    }

    #[test]
    fn run_revoke_and_rotate_signature_compiles() {
        let _: fn(&Path, &str, Option<String>, bool) -> Result<()> = run_revoke_and_rotate;
    }

    #[test]
    fn method_constants_have_correct_wire_names() {
        assert_eq!(METHOD_LIST_PEERS, "list_peers");
        assert_eq!(METHOD_REVOKE_PEER, "revoke_peer");
        assert_eq!(METHOD_REVOKE_ALL_PEERS, "revoke_all_peers");
        assert_eq!(METHOD_UNPAIR_PEER, "unpair_peer");
        assert_eq!(METHOD_REVOKE_AND_ROTATE, "revoke_and_rotate");
    }
}

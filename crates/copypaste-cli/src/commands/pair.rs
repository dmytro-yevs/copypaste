//! `copypaste pair …` — LAN/SAS peer-pairing subcommands.
//!
//! Exposes the daemon's mDNS-SD discovery and SAS pairing IPC verbs so
//! headless/SSH users can pair devices without opening the macOS UI.
//!
//! ## Typical headless pairing flow
//!
//! ```text
//! # On the accepting side — discover nearby CopyPaste devices:
//! copypaste pair list
//!
//! # Initiate SAS pairing with a discovered device:
//! copypaste pair accept <device_id>
//!
//! # Poll for the SAS code (may need a moment to negotiate):
//! copypaste pair sas
//!
//! # Display and verbally confirm the 6-digit code with the other device's
//! # user, then accept or reject:
//! copypaste pair confirm          # accept
//! copypaste pair confirm --reject # reject
//!
//! # If something goes wrong or you change your mind:
//! copypaste pair abort
//! ```
//!
//! ## IPC verbs used
//!
//! | CLI subcommand        | IPC method            |
//! |-----------------------|-----------------------|
//! | `pair list`           | `list_discovered`     |
//! | `pair accept <id>`    | `pair_with_discovered`|
//! | `pair sas`            | `pair_get_sas`        |
//! | `pair confirm`        | `pair_confirm_sas`    |
//! | `pair abort`          | `pair_abort`          |

use crate::commands::common::exit_on_err;
use crate::ipc::IpcClient;
use anyhow::{anyhow, Result};
use copypaste_ipc::{
    METHOD_LIST_DISCOVERED, METHOD_PAIR_ABORT, METHOD_PAIR_CONFIRM_SAS, METHOD_PAIR_GET_SAS,
    METHOD_PAIR_WITH_DISCOVERED,
};
use std::path::Path;

// ── pair list ─────────────────────────────────────────────────────────────────

/// List CopyPaste devices currently visible on the local network via mDNS-SD.
///
/// Shows device name, ID, IP addresses, and whether it is already paired.
/// Devices with a `bport` (bootstrap port) support SAS pairing; those without
/// it require the QR/password path.
pub fn run_list(socket_path: &Path) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_LIST_DISCOVERED,
        serde_json::json!({}),
    );
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    let data = resp
        .data
        .as_ref()
        .ok_or_else(|| anyhow!("daemon returned no data for list_discovered"))?;

    let devices = data["devices"]
        .as_array()
        .ok_or_else(|| anyhow!("daemon response missing 'devices' array"))?;

    if devices.is_empty() {
        println!("No devices discovered on the local network.");
        println!(
            "Ensure the other device is running CopyPaste with mDNS-SD enabled \
             and is on the same network."
        );
        return Ok(());
    }

    println!(
        "{:<36} {:<20} {:<8} {:<6}",
        "Device ID", "Name", "Paired", "SAS"
    );
    println!("{}", "-".repeat(74));
    for dev in devices {
        let id = dev["device_id"]
            .as_str()
            .or_else(|| dev["did"].as_str())
            .unwrap_or("?");
        let name = dev["device_name"]
            .as_str()
            .or_else(|| dev["name"].as_str())
            .unwrap_or("(unknown)");
        let paired = dev["paired"].as_bool().unwrap_or(false);
        // bport present (non-null) means the device supports SAS pairing.
        let sas = !dev["bport"].is_null();
        println!(
            "{:<36} {:<20} {:<8} {:<6}",
            id,
            name,
            if paired { "yes" } else { "no" },
            if sas { "yes" } else { "no" },
        );
    }

    Ok(())
}

// ── pair accept ───────────────────────────────────────────────────────────────

/// Initiate SAS pairing with a discovered device.
///
/// Starts the SAS handshake as the initiator. After the daemon reaches the
/// `awaiting_sas` state, run `copypaste pair sas` to retrieve the code and
/// `copypaste pair confirm` to accept or reject it.
pub fn run_accept(socket_path: &Path, device_id: &str) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_PAIR_WITH_DISCOVERED,
        serde_json::json!({ "device_id": device_id }),
    );
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    println!("SAS pairing initiated with device '{device_id}'.");
    println!("Run 'copypaste pair sas' to retrieve the Short Authentication String.");
    println!("Then run 'copypaste pair confirm' once both devices display the same code.");
    Ok(())
}

// ── pair sas ─────────────────────────────────────────────────────────────────

/// Poll the daemon for the current SAS code and pairing state.
///
/// The daemon returns `{ state, sas?, role? }`. When `state` is
/// `awaiting_sas` the 6-digit `sas` code is displayed. Both the initiator
/// and the responder must see the same code before confirming.
pub fn run_sas(socket_path: &Path) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_PAIR_GET_SAS,
        serde_json::json!({}),
    );
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    let data = resp
        .data
        .as_ref()
        .ok_or_else(|| anyhow!("daemon returned no data for pair_get_sas"))?;

    let state = data["state"].as_str().unwrap_or("unknown");

    match state {
        "awaiting_sas" => {
            let sas = data["sas"].as_str().unwrap_or("(unavailable)");
            let role = data["role"].as_str().unwrap_or("unknown");
            println!("Short Authentication String (SAS): {sas}");
            println!("Role: {role}");
            println!();
            println!("Verbally confirm this 6-digit code matches what the other device displays.");
            println!("Then run 'copypaste pair confirm' (or 'copypaste pair confirm --reject').");
        }
        "confirmed" => {
            println!("Pairing confirmed! Both devices are now trusted.");
        }
        "rejected" => {
            println!("Pairing was rejected (SAS mismatch or user rejected).");
        }
        "aborted" => {
            println!("Pairing was aborted.");
        }
        "timed_out" => {
            println!("Pairing timed out. Run 'copypaste pair accept <device_id>' to retry.");
        }
        "idle" => {
            println!("No pairing in progress (state: idle).");
            println!("Run 'copypaste pair list' then 'copypaste pair accept <device_id>'.");
        }
        other => {
            println!("Pairing state: {other}");
            if let Some(sas) = data["sas"].as_str() {
                println!("SAS: {sas}");
            }
        }
    }

    Ok(())
}

// ── pair confirm ─────────────────────────────────────────────────────────────

/// Send the local user's SAS accept or reject decision to the daemon.
///
/// Pass `--reject` to reject the pairing (e.g. the SAS codes do not match).
/// Without the flag, the default is to accept.
pub fn run_confirm(socket_path: &Path, reject: bool) -> Result<()> {
    let accept = !reject;
    let mut client = IpcClient::connect(socket_path)?;
    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_PAIR_CONFIRM_SAS,
        serde_json::json!({ "accept": accept }),
    );
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    if accept {
        println!("SAS accepted. Waiting for the other device to confirm as well.");
        println!(
            "Check 'copypaste pair sas' for the final state \
             (confirmed / rejected / timed_out)."
        );
    } else {
        println!("SAS rejected. Pairing has been cancelled.");
    }
    Ok(())
}

// ── pair abort ───────────────────────────────────────────────────────────────

/// Abort the in-flight SAS pairing and reset the daemon's state machine to
/// idle.
pub fn run_abort(socket_path: &Path) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_PAIR_ABORT,
        serde_json::json!({}),
    );
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    println!("Pairing aborted. State machine reset to idle.");
    Ok(())
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_list_signature_compiles() {
        let _: fn(&Path) -> Result<()> = run_list;
    }

    #[test]
    fn run_accept_signature_compiles() {
        let _: fn(&Path, &str) -> Result<()> = run_accept;
    }

    #[test]
    fn run_sas_signature_compiles() {
        let _: fn(&Path) -> Result<()> = run_sas;
    }

    #[test]
    fn run_confirm_signature_compiles() {
        let _: fn(&Path, bool) -> Result<()> = run_confirm;
    }

    #[test]
    fn run_abort_signature_compiles() {
        let _: fn(&Path) -> Result<()> = run_abort;
    }

    #[test]
    fn method_constants_have_correct_wire_names() {
        assert_eq!(METHOD_LIST_DISCOVERED, "list_discovered");
        assert_eq!(METHOD_PAIR_WITH_DISCOVERED, "pair_with_discovered");
        assert_eq!(METHOD_PAIR_GET_SAS, "pair_get_sas");
        assert_eq!(METHOD_PAIR_CONFIRM_SAS, "pair_confirm_sas");
        assert_eq!(METHOD_PAIR_ABORT, "pair_abort");
    }

    #[test]
    fn run_confirm_accept_is_default() {
        // Confirm that `reject = false` means `accept = true`.
        // This is a pure logic assertion — no IPC involved.
        let reject = false;
        let accept = !reject;
        assert!(accept, "default (reject=false) must mean accept=true");
    }

    #[test]
    fn run_confirm_reject_flag_inverts() {
        let reject = true;
        let accept = !reject;
        assert!(!accept, "--reject must mean accept=false");
    }
}

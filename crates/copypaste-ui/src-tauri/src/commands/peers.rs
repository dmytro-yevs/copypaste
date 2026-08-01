//! Peer commands: pair, peers, unpair, sync.
//!
//! These exist because CLAUDE.md rule 6 says a feature is not done until it has
//! a UI, and names *this exact feature* as the case the rule was written from:
//! the transport, the merge, discovery, the daemon wiring and the CLI verbs all
//! landed while the only way to pair two devices was to type a command into a
//! terminal.
//!
//! What a pairing screen needs, and what these five commands are shaped to
//! give it:
//!
//! | screen state | command |
//! |---|---|
//! | show a code and where to connect | [`pair_create`] |
//! | enter the other device's code | [`pair_accept`] |
//! | the list of known devices | [`peers`] |
//! | forget one | [`unpair`] |
//! | sync now, with per-device results | [`sync_now`] |
//! | the devices on this network | [`discovered`] / [`rescan`] |
//!
//! # The code is a secret
//!
//! [`pair_create`] returns a pre-shared key in transferable form. Anyone
//! holding it can pair with this device. It is minted once and the backend
//! keeps no retrievable copy, so there is deliberately no "show me the code
//! again" command — the frontend must treat it like a password: shown once,
//! never persisted, never logged, and cleared from component state when the
//! pairing screen closes.

use tauri::State;

use crate::backend::{Backend, BackendError, SelectedBackend};
use crate::model::{UiDiscovered, UiPairing, UiPeer, UiSyncResult};

type Result<T> = std::result::Result<T, BackendError>;

/// Mint a pairing code on this device, to be entered on the other one.
///
/// `name` is what to call the other device until it says its own name.
#[tauri::command]
pub async fn pair_create(backend: State<'_, SelectedBackend>, name: String) -> Result<UiPairing> {
    backend.pair_create(&name).await
}

/// Consume a code from another device and complete the pairing.
///
/// The pairing is only kept if a sync session with that device succeeds, so a
/// wrong code or an unreachable address leaves nothing behind — which means the
/// UI can treat a failure here as "nothing happened" and let the user retype.
///
/// Returns the paired device, so the screen can move straight to the success
/// state without re-listing.
#[tauri::command]
pub async fn pair_accept(
    backend: State<'_, SelectedBackend>,
    code: String,
    addr: String,
) -> Result<Vec<UiPeer>> {
    if code.trim().is_empty() {
        return Err(BackendError::Invalid(
            "Enter the code from the other device.",
        ));
    }
    if addr.trim().is_empty() {
        return Err(BackendError::Invalid(
            "Enter the address the other device is listening on.",
        ));
    }
    backend.pair_accept(code.trim(), addr.trim()).await
}

/// Open Android's system-owned QR scanner and return the string it decoded.
///
/// The scanner owns camera access, so CopyPaste never requests `CAMERA` and
/// the untrusted result is validated by the React pairing decoder before it can
/// reach [`pair_accept`]. macOS uses its in-window scanner instead.
#[cfg(target_os = "android")]
#[tauri::command]
pub async fn scan_pairing_qr(
    scanner: tauri::State<'_, crate::pairing_scanner::AndroidPairingScanner>,
) -> Result<Option<String>> {
    scanner.scan().await
}

/// macOS scans in the WebView so it can use the app's camera entitlement.
/// Keeping the command present means an unexpected platform check degrades to
/// the manual-code fallback instead of becoming a missing-command failure.
#[cfg(not(target_os = "android"))]
#[tauri::command]
pub fn scan_pairing_qr() -> Result<Option<String>> {
    Err(BackendError::Unsupported(
        "The native QR scanner is only available on Android.",
    ))
}

/// Known devices and when each was last reachable.
///
/// `online: false` means "not seen on the network", never "unreachable" — a
/// device on a network without multicast is reachable by address and still
/// reads as offline, so the UI must not render it as an error state.
#[tauri::command]
pub async fn peers(backend: State<'_, SelectedBackend>) -> Result<Vec<UiPeer>> {
    backend.peers().await
}

/// Forget a device.
///
/// Local and one-sided: the other device keeps its half until it also unpairs.
/// Worth saying in the confirmation dialog, because "unpair" reads as mutual.
#[tauri::command]
pub async fn unpair(backend: State<'_, SelectedBackend>, pairing_id: String) -> Result<()> {
    backend.unpair(&pairing_id).await
}

/// Cut a device off for good.
///
/// [`unpair`] and this differ in one respect, and it is the one that matters
/// after a phone is lost: the pairing code is the long-term key, so an unpaired
/// device can be paired again by re-entering the code someone kept, while a
/// revoked pairing id is refused from here on. Nothing is negotiated — the
/// other device is not told and keeps its own half — and no clipboard history
/// is deleted on either side.
///
/// Irreversible, so the confirmation is the caller's to obtain. Reaching here
/// *is* the confirmation, exactly as with `delete_all`.
#[tauri::command]
pub async fn revoke(backend: State<'_, SelectedBackend>, pairing_id: String) -> Result<()> {
    if pairing_id.trim().is_empty() {
        return Err(BackendError::Invalid("There is no device to revoke."));
    }
    backend.revoke(pairing_id.trim()).await
}

/// Sync with one device, or with all of them when `pairing_id` is absent.
///
/// Named for `copypaste_ipc::Method::SyncNow` rather than plain `sync`, for the
/// same reason as the history commands: the wire enum is the single model, and
/// the frontend already calls this name.
///
/// One device failing does not abort the rest: each gets its own result with
/// its own `error` field, so the UI reports per device rather than collapsing a
/// partial success into one failure.
#[tauri::command]
pub async fn sync_now(
    backend: State<'_, SelectedBackend>,
    pairing_id: Option<String>,
) -> Result<Vec<UiSyncResult>> {
    backend.sync(pairing_id.as_deref()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Empty input is caught before it reaches a backend, and the messages tell
    /// the user what to do rather than what went wrong.
    #[test]
    fn empty_pairing_input_is_rejected_actionably() {
        for err in [
            BackendError::Invalid("Enter the code from the other device."),
            BackendError::Invalid("Enter the address the other device is listening on."),
        ] {
            let shown = err.to_string();
            assert!(shown.starts_with("Enter "), "{shown}");
            assert!(!shown.contains('/'), "{shown}");
        }
    }
}

/// Devices this network has advertised, paired or not.
///
/// The reason pairing can be a gesture rather than a transcription: without it
/// the accepting device has to be told a `host:port` as well as a 52-character
/// code, and both by hand.
///
/// **Nothing here is authenticated.** Name, address and pairing id are what an
/// mDNS record claimed; only the Noise handshake proves any of it, so the UI
/// labels these as unverified (manifest 06, INV-15).
#[tauri::command]
pub async fn discovered(backend: State<'_, SelectedBackend>) -> Result<Vec<UiDiscovered>> {
    backend.discovered().await
}

/// Advertise and browse again, then answer as [`discovered`] does.
///
/// A user-visible verb because multicast is unreliable in exactly the places
/// people try to pair — a phone that just joined the network, a Mac that just
/// woke — and "nothing found" needs a way to be retried that is not "quit the
/// app".
#[tauri::command]
pub async fn rescan(backend: State<'_, SelectedBackend>) -> Result<Vec<UiDiscovered>> {
    backend.rescan().await
}

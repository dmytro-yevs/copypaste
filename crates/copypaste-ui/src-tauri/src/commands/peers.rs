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
//! | the list of known devices | [`peers`] |
//! | forget one | [`unpair`] |
//! | sync now, with per-device results | [`sync_now`] |
//! | the devices on this network | [`discovered`] / [`rescan`] |
//!
//! Pair-create and pair-accept are intentionally **not** Tauri commands. The
//! current wire cannot bind a peer-visible SAS decision before persistence;
//! exposing a credential-returning command would invite a non-compliant UI.
//! ADR-0007 records the re-entry requirements.

use tauri::State;

use crate::backend::{Backend, BackendError, SelectedBackend};
use crate::model::{UiDiscovered, UiPeer, UiSyncResult};

type Result<T> = std::result::Result<T, BackendError>;

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

//! QR-based and direct bootstrap pairing FFI exports.
//!
//! Covers: `PairingQrPayload`, `ScannedPairing`, `DeviceCert`, `BootstrapResult`,
//! `SyncProvisioning`, `build_pairing_qr`, `parse_pairing_qr`,
//! `generate_device_cert`, `bootstrap_pair_initiator`, the discovery + SAS
//! pairing FFI (`start_discovery`, `stop_discovery`, `list_discovered`,
//! `pair_with_discovered`, `pair_get_sas`, `pair_confirm_sas`, `pair_abort`,
//! `pair_reset`), and private helpers `build_android_peer_meta`,
//! `bootstrap_result_from_pairing`, `confirmed_pairing_from`.
//!
//! # Module layout (ADR-017 split)
//!
//! Every item below is re-exported here so `crate::ffi_pairing::<name>`
//! resolves exactly as it did when this was a single file.
//! - [`qr`]: the QR-transport pairing FFI (`PairingQrPayload`, `ScannedPairing`,
//!   `build_pairing_qr`, `parse_pairing_qr`). The QR is purely a transport for
//!   the existing PAKE pairing material.
//! - [`runtime`]: the shared process-wide tokio runtime backing every blocking
//!   P2P FFI wrapper in [`bootstrap`] and [`discovery`].
//! - [`bootstrap`]: mTLS cert generation + QR-initiator bootstrap PAKE pairing
//!   FFI, plus the mapping helpers shared by every Android pairing path
//!   (`build_android_peer_meta`, `bootstrap_result_from_pairing`,
//!   `confirmed_pairing_from`).
//! - [`discovery`]: discovery + SAS pairing FFI (Android parity for LAN
//!   discovery) — the standing responder, initiator, and poll/confirm surface.

mod bootstrap;
mod discovery;
mod qr;
mod runtime;

pub(crate) use runtime::runtime;

pub use bootstrap::{
    bootstrap_pair_initiator, bootstrap_result_from_pairing, build_android_peer_meta,
    confirmed_pairing_from, generate_device_cert, BootstrapResult, DeviceCert, SyncProvisioning,
};
pub use discovery::{
    list_discovered, pair_abort, pair_confirm_sas, pair_get_sas, pair_reset, pair_with_discovered,
    start_discovery, stop_discovery,
};
pub use qr::{build_pairing_qr, parse_pairing_qr, PairingQrPayload, ScannedPairing};

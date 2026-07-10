//! Android discovery + SAS pairing (LAN/SAS Phase 4 — Android parity).
//!
//! This is the Android analog of the macOS daemon's discovery-pairing path
//! (`copypaste-daemon/src/pairing_sm.rs` + the `start_p2p` standing responder).
//! Unlike the QR path — where the high-entropy `PairingToken` carried in the QR
//! is the authenticator and PAKE alone proves both sides know it — the discovery
//! path has NO pre-shared secret. The bootstrap handshake runs with an EPHEMERAL
//! random password the initiator transmits in-clear inside the (unauthenticated)
//! bootstrap TLS channel, and authentication is provided ENTIRELY by the human
//! Short Authentication String (SAS) comparison.
//!
//! The SAS is derived from the post-PAKE, post-channel-binding `bound_key`
//! (`copypaste_p2p::pake::derive_sas`). A man-in-the-middle that substitutes its
//! own password per leg yields a DIFFERENT `bound_key` per leg → a different SAS
//! per leg → the two humans see mismatched codes and abort. Both sides MUST
//! confirm (frame 10a ACCEPT/REJECT in `run_with_confirm` /
//! `run_initiator_with_confirm`) before any key is trusted or persisted.
//!
//! # FFI shape — POLLED state machine (NOT a callback interface)
//!
//! UniFFI cannot pass an async Rust callback across the boundary, so the
//! handshake's `confirm` closure is wired to a [`PairingCoordinator`] instead:
//! the closure transitions the coordinator into `AwaitingSas` and parks on a
//! `tokio::sync::oneshot`. Kotlin POLLS [`pair_get_sas`](crate::pair_get_sas)
//! for the SAS, shows it, and calls
//! [`pair_confirm_sas`](crate::pair_confirm_sas) to fire the oneshot. There is a
//! single process-global [`AndroidPairing`] (the coordinator + the standing
//! responder + the in-flight initiator task) — exactly ONE pairing may be in
//! flight at a time (v0.6 simplicity).
//!
//! The standing responder (bound on `bport` when `start` is called)
//! makes the Android device pairable FROM macOS: it accepts an inbound bootstrap
//! connection, runs `run_with_confirm` wired to the SAME coordinator with the
//! `Responder` role, and routes the SAS through the same poll/confirm flow.
//!
//! # Security (load-bearing — mirrors macOS)
//!
//! * SAS derives from the post-channel-binding `bound_key`; both sides exchange
//!   frame 10a ACCEPT before the key is trusted.
//! * Reject / abort / timeout drops the confirmation channel → the handshake's
//!   `confirm` await resolves to a rejection → keys drop/zeroize, NOTHING is
//!   persisted (the coordinator never reaches `Confirmed`).
//! * Purely additive: the QR transcript (`run` / `run_initiator`) and
//!   fingerprint pinning are untouched.
//! * `session_key` crosses the FFI per the documented contract
//!   ([`PairStatus`]); Kotlin zeroes it after KEK-wrapping.
//! * Key / SAS bytes are NEVER logged.
//!
//! # Module layout (ADR-017 split)
//!
//! This module is split into cohesive submodules; every item below is
//! re-exported here so `crate::pairing::<name>` resolves exactly as it did when
//! this was a single file. `DiscoveredPeer` and `PairStatus` are UDL-exported
//! dictionaries (FROZEN field shape) re-exported at `lib.rs`; everything else is
//! internal, consumed only by `ffi_pairing.rs`.
//! - `state`: the state-machine domain types (`PairingRole`, `PairingState`,
//!   `ConfirmedPairing`).
//! - `coordinator`: [`PairingCoordinator`], owning the live state + the
//!   confirm oneshot channel.
//! - `global`: the process-global [`AndroidPairing`] singleton + shared
//!   constants.
//! - `dto`: the FFI DTOs (`DiscoveredPeer`, `PairStatus`).
//! - `helpers`: free helpers (`ipv4_first_addr`, `outcome_for_initiator_error`,
//!   `p2p_err`).

mod coordinator;
mod dto;
mod global;
mod helpers;
mod state;

#[cfg(test)]
mod tests;

pub use coordinator::PairingCoordinator;
pub use dto::{DiscoveredPeer, PairStatus};
pub use global::{global, AndroidPairing, DISCOVERY_PAIRING_PASSWORD, SAS_CONFIRM_TIMEOUT};
pub use helpers::{ipv4_first_addr, outcome_for_initiator_error, p2p_err};
pub use state::{ConfirmedPairing, PairingRole, PairingState};

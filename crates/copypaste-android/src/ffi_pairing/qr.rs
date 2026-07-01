//! QR device-pairing FFI: `PairingQrPayload`, `ScannedPairing`,
//! `build_pairing_qr`, `parse_pairing_qr`.
//!
//! The QR code is purely a transport for the existing PAKE pairing material.
//! `pake_password` is the base64url rendering of the single-use token; it is fed
//! into the existing password-authenticated pairing flow in place of the
//! manually-typed code, preserving every property of that handshake.

use crate::{panic_boundary, CopypasteError};

/// FFI result of [`build_pairing_qr`].
pub struct PairingQrPayload {
    pub qr: String,
    pub pake_password: String,
}

/// FFI result of [`parse_pairing_qr`].
pub struct ScannedPairing {
    pub fingerprint: String,
    pub device_id: String,
    pub device_name: String,
    pub addr_hint: String,
    pub pake_password: String,
}

/// Build a QR pairing payload (display side). Generates a fresh single-use
/// token internally and returns both the encoded QR string and the PAKE
/// password derived from that token.
pub fn build_pairing_qr(
    fingerprint: String,
    device_id: String,
    device_name: String,
    addr_hint: String,
) -> Result<PairingQrPayload, CopypasteError> {
    panic_boundary::catch_result(|| {
        let payload =
            copypaste_core::PairingPayload::new(fingerprint, device_id, device_name, addr_hint)
                // P2pError is semantically correct here: QR payload generation is
                // pairing infrastructure (token generation / encoding), not a
                // decryption step.  DecryptionFailed was a copy-paste mistake from
                // parse_pairing_qr (the scan side) and is misleading to Kotlin
                // callers trying to distinguish pairing vs. crypto failures.
                .map_err(|e| CopypasteError::P2pError {
                    reason: e.to_string(),
                })?;
        let pake_password = payload.token.to_pake_password();
        let qr = payload.encode();
        Ok(PairingQrPayload { qr, pake_password })
    })
}

/// Parse a scanned QR payload (scan side). Returns the peer pairing material,
/// including the PAKE password to drive the initiator handshake.
///
/// A malformed or unsupported-version payload yields
/// [`CopypasteError::DecryptionFailed`] (reused as the generic parse error so
/// no new FFI error variant / ABI break is needed).
pub fn parse_pairing_qr(payload: String) -> Result<ScannedPairing, CopypasteError> {
    panic_boundary::catch_result(|| {
        let parsed = copypaste_core::PairingPayload::decode(&payload).map_err(|e| {
            CopypasteError::DecryptionFailed {
                reason: e.to_string(),
            }
        })?;
        let pake_password = parsed.token.to_pake_password();
        Ok(ScannedPairing {
            fingerprint: parsed.fingerprint,
            device_id: parsed.device_id,
            device_name: parsed.device_name,
            addr_hint: parsed.addr_hint,
            pake_password,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Characterization test (ADR-017 split safety net): a QR built by
    /// `build_pairing_qr` must parse back via `parse_pairing_qr` into a
    /// `ScannedPairing` carrying the SAME fingerprint / device_id /
    /// device_name / addr_hint / pake_password — the QR is purely a transport,
    /// so nothing may be lost or altered in the round trip.
    #[test]
    fn build_then_parse_pairing_qr_roundtrips() {
        let fingerprint = "aa".repeat(32); // 64 hex chars = 32 bytes.
        let device_id = "11111111-2222-3333-4444-555555555555".to_string();
        let device_name = "Alice's MacBook".to_string();
        let addr_hint = "10.0.0.5:51515".to_string();

        let built = build_pairing_qr(
            fingerprint.clone(),
            device_id.clone(),
            device_name.clone(),
            addr_hint.clone(),
        )
        .expect("build_pairing_qr must succeed for valid inputs");

        let scanned = parse_pairing_qr(built.qr.clone())
            .expect("parse_pairing_qr must decode a QR just built by build_pairing_qr");

        assert_eq!(scanned.fingerprint, fingerprint);
        assert_eq!(scanned.device_id, device_id);
        assert_eq!(scanned.device_name, device_name);
        assert_eq!(scanned.addr_hint, addr_hint);
        assert_eq!(
            scanned.pake_password, built.pake_password,
            "the PAKE password must round-trip identically — it authenticates the handshake"
        );
    }
}

use sha2::{Digest, Sha256};

/// Compute the canonical device fingerprint from a raw public key.
///
/// Format: first 16 bytes of `SHA-256(public_key)` rendered as
/// lowercase hex pairs separated by `:` (e.g. `aa:bb:cc:...`).
/// This is the user-visible identifier shown during pairing — keep it short
/// enough for humans to compare on two screens.
///
/// # Why 16 bytes (128 bits) instead of the full 32-byte SHA-256
///
/// This function is the **human-visible** fingerprint used during pairing:
/// both screens must show the same string for a human to visually compare.
/// 16 bytes produces a 47-character colon-hex string that fits in a single
/// line of UI; the full 32 bytes would be 95 characters and effectively
/// unreadable at a glance.
///
/// 128 bits of SHA-256 prefix is more than sufficient for collision resistance
/// in this threat model: a birthday attack against a fleet of ≤10 000 paired
/// devices needs ~2^64 operations — well beyond practical reach.
///
/// **This truncation applies only to the display identifier.** The mTLS
/// allowlist uses [`copypaste_core::DeviceKeypair::fingerprint`], which
/// returns the full 64-character (256-bit) hex SHA-256 and is the binding
/// used for cryptographic device pinning. The two functions serve different
/// purposes and intentionally produce different lengths.
///
/// If you ever need to change the display length, update the constant `16`
/// below and document the new rationale here — do NOT silently change it,
/// as it will invalidate every previously-paired device's stored fingerprint
/// (CopyPaste-44rq.56 / SEC-4).
pub fn own_fingerprint(public_key: &[u8]) -> String {
    let digest = Sha256::digest(public_key);
    digest[..16]
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(":")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_fingerprint_is_sha256_prefix() {
        let pk = [0u8; 32];
        let fp = own_fingerprint(&pk);
        // SHA-256 of 32 zero bytes is known: 66687aadf862bd776c8fc18b8e9f8e20...
        assert!(fp.starts_with("66:68:7a:ad:f8:62:bd:77:6c:8f:c1:8b:8e:9f:8e:20"));
        assert_eq!(fp.matches(':').count(), 15); // 16 bytes = 15 colons
    }
}

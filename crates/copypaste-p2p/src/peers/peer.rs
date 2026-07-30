//! One paired device, and the rules that keep its pre-shared key out of every
//! place a secret should not be.

use std::fmt;
use std::net::SocketAddr;

use subtle::ConstantTimeEq;
use zeroize::Zeroize;

use super::PeerStoreError;
use crate::transport::TOKEN_LEN;

/// One paired device.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Peer {
    /// [`crate::PairingToken::pairing_id`] — the non-secret, stable key for
    /// this pairing. Safe to log.
    pub pairing_id: String,

    /// What the user calls this device. Cosmetic, peer-supplied, never trusted
    /// for anything.
    pub name: String,

    /// The Noise pre-shared key. **This is the secret.** Serialised as hex
    /// because JSON has no byte type; never rendered by `Debug`.
    #[serde(with = "psk_hex")]
    pub psk: [u8; TOKEN_LEN],

    /// Where this peer was last reached, if it ever was. A hint that saves a
    /// discovery round; always re-verified by the handshake.
    pub last_addr: Option<SocketAddr>,

    /// Unix milliseconds of the last successful contact. Written from the
    /// *local* clock at the end of a session, never from anything the peer
    /// said, so peer clock skew cannot affect it.
    ///
    /// **Zero is load-bearing.** It means this pairing has never been contacted,
    /// which is what [`super::PeerStore`] reads as "the code is still
    /// unredeemed" — the state that carries a deadline and that the first
    /// session burns. Writing a placeholder timestamp here instead of `0` would
    /// silently establish a pairing nobody has claimed.
    pub last_seen_ms: i64,
}

impl Peer {
    /// Constant-time comparison of this peer's PSK against a candidate, and the
    /// only PSK comparison this type offers: `==` on key bytes short-circuits
    /// at the first differing byte and leaks the matching-prefix length by
    /// timing (port manifest 02, I-13).
    #[must_use]
    pub fn psk_matches(&self, candidate: &[u8; TOKEN_LEN]) -> bool {
        self.psk[..].ct_eq(&candidate[..]).into()
    }

    pub(super) fn validate(&self) -> Result<(), PeerStoreError> {
        if self.pairing_id.is_empty() {
            return Err(PeerStoreError::Invalid("pairing id is empty"));
        }
        if self.pairing_id.len() > 128 {
            return Err(PeerStoreError::Invalid("pairing id is implausibly long"));
        }
        // An all-zero PSK is what an uninitialised buffer looks like, and
        // storing one would pair this device with anyone who guessed the
        // obvious. Fail closed rather than persist it. Constant-time, because
        // the comparison is against key material.
        if bool::from(self.psk[..].ct_eq(&[0u8; TOKEN_LEN][..])) {
            return Err(PeerStoreError::Invalid("pre-shared key is all zeroes"));
        }
        Ok(())
    }
}

/// Wipes the pre-shared key when the record goes out of scope (port manifest
/// 02, I-12). Consequence: fields cannot be moved out individually
/// (`let Peer { psk, .. } = peer;` will not compile) — clone instead. That
/// friction is intended; every copy of a PSK should be deliberate.
impl Drop for Peer {
    fn drop(&mut self) {
        self.psk.zeroize();
    }
}

/// Everything except the key.
impl fmt::Debug for Peer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Peer")
            .field("pairing_id", &self.pairing_id)
            .field("name", &self.name)
            .field("last_addr", &self.last_addr)
            .field("last_seen_ms", &self.last_seen_ms)
            .finish_non_exhaustive()
    }
}

/// Hex for the PSK, because JSON has no byte type.
///
/// `hex` is already a workspace dependency; port manifest 02 records ~6 sites
/// where v1 hand-rolled this while depending on the same crate.
mod psk_hex {
    use super::TOKEN_LEN;
    use serde::de::Error as _;
    use serde::{Deserialize as _, Deserializer, Serializer};

    pub(super) fn serialize<S: Serializer>(
        value: &[u8; TOKEN_LEN],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&hex::encode(value))
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<[u8; TOKEN_LEN], D::Error> {
        let text = zeroize::Zeroizing::new(String::deserialize(deserializer)?);
        let mut bytes = zeroize::Zeroizing::new(
            hex::decode(text.as_bytes()).map_err(|_| D::Error::custom("psk is not hex"))?,
        );
        if bytes.len() != TOKEN_LEN {
            bytes.clear();
            return Err(D::Error::custom("psk is not 32 bytes"));
        }
        let mut out = [0u8; TOKEN_LEN];
        out.copy_from_slice(&bytes);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peers::testutil::{peer, store_path};
    use crate::peers::PeerStore;

    #[test]
    fn peer_debug_shows_no_key_material() {
        let p = peer("Laptop");
        let rendered = format!("{p:?}");
        assert!(!rendered.contains(&hex::encode(p.psk)), "leaked hex psk");
        assert!(
            !rendered.contains(&hex::encode_upper(p.psk)),
            "leaked upper-hex psk"
        );
        assert!(
            !rendered.contains(&format!("{:?}", &p.psk[..])),
            "leaked bytes"
        );
        assert!(
            !rendered.contains("psk"),
            "psk field must not appear at all"
        );
        // The non-secret fields are still useful.
        assert!(rendered.contains("Laptop"));
        assert!(rendered.contains(&p.pairing_id));

        // And the store's own Debug prints neither keys nor the path.
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);
        let store = PeerStore::open(&path).expect("open");
        let rendered = format!("{store:?}");
        assert!(!rendered.contains(&path.to_string_lossy().to_string()));
        assert!(!rendered.contains("peers-v2.json"));
    }

    #[test]
    fn psk_comparison_is_available_and_correct() {
        let p = peer("Laptop");
        assert!(p.psk_matches(&p.psk));
        let mut other = p.psk;
        other[0] ^= 1;
        assert!(!p.psk_matches(&other));
        assert!(!p.psk_matches(&[0u8; TOKEN_LEN]));
    }
}

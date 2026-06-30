//! Device identity and paired-peer allowlist.
//!
//! [`DeviceFingerprint`] is a newtype over the hex-encoded SHA-256 of the device TLS cert.
//! [`PairedPeers`] maps known fingerprints to display names and handles cert rotation races.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Opaque device identity — the SHA-256 fingerprint of the device's TLS cert
/// encoded as lowercase hex.
///
/// CopyPaste-crh3.87: a newtype (not `= String`) so a device *name* / UUID can no
/// longer be passed where a fingerprint is expected — that previously compiled
/// silently and made the constant-time identity comparison always-false.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeviceFingerprint(pub String);

impl DeviceFingerprint {
    /// View as the underlying lowercase-hex string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
    /// Consume into the owned `String`.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl std::ops::Deref for DeviceFingerprint {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for DeviceFingerprint {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for DeviceFingerprint {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DeviceFingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for DeviceFingerprint {
    fn from(s: String) -> Self {
        DeviceFingerprint(s)
    }
}

impl From<&str> for DeviceFingerprint {
    fn from(s: &str) -> Self {
        DeviceFingerprint(s.to_owned())
    }
}

// CopyPaste-crh3.87: ergonomic cross-comparison with the raw hex string on both
// sides, so `fingerprint == some_str` call sites keep working without allocating.
impl PartialEq<str> for DeviceFingerprint {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<String> for DeviceFingerprint {
    fn eq(&self, other: &String) -> bool {
        &self.0 == other
    }
}

impl PartialEq<DeviceFingerprint> for str {
    fn eq(&self, other: &DeviceFingerprint) -> bool {
        self == other.0
    }
}

impl PartialEq<DeviceFingerprint> for String {
    fn eq(&self, other: &DeviceFingerprint) -> bool {
        self == &other.0
    }
}

/// Default window during which a peer's *previous* certificate fingerprint is
/// still accepted after a rotation (S10 — cert rotation race). Sized to
/// comfortably cover an in-flight handshake plus `connect_with_retry`'s full
/// retry budget; short enough that a revoked/rotated cert is not honoured for
/// long. See [`PairedPeers::rotate_peer`].
pub const CERT_ROTATION_GRACE: Duration = Duration::from_secs(60);

/// A peer fingerprint that has been superseded by a rotation but is still
/// accepted until `expires_at` to avoid the cert-rotation race (S10).
#[derive(Clone, Debug)]
struct SupersededFingerprint {
    display_name: String,
    expires_at: Instant,
}

/// Inner, lock-guarded state of [`PairedPeers`].
#[derive(Default, Debug)]
struct PairedPeersInner {
    /// Current (active) fingerprints → display name.
    inner: HashMap<DeviceFingerprint, String>,
    /// Recently-rotated-away fingerprints, accepted until their grace expiry.
    superseded: HashMap<DeviceFingerprint, SupersededFingerprint>,
}

/// Map of known paired peers: their fingerprint → optional display name.
///
/// Before the TLS handshake, the transport checks that the peer's certificate
/// fingerprint is in this map. Connections from unknown fingerprints are
/// rejected.
///
/// # Interior mutability (fix/p2p-c-review #2)
///
/// The allowlist is wrapped in an `Arc<RwLock<…>>` so a single `PairedPeers`
/// handle can be shared (via `clone()`) between the long-running mTLS transport
/// (which only reads, via [`is_known`](Self::is_known)) and the IPC pairing
/// handlers (which mutate it via [`add`](Self::add) /
/// [`rotate_peer`](Self::rotate_peer) when a PAKE handshake finishes). All
/// mutators therefore take `&self`; clones observe one another's updates.
///
/// # Cert rotation (S10)
///
/// When a peer rotates its certificate, the new fingerprint is unknown to us
/// until we learn it out-of-band. Meanwhile any TLS handshake already in flight
/// (or retried by [`crate::PeerTransport::connect_with_retry`]) still presents the
/// *old* cert. To close that race, [`rotate_peer`](Self::rotate_peer) installs
/// the new fingerprint as current while keeping the previous one valid for a
/// bounded grace window ([`CERT_ROTATION_GRACE`]). [`is_known`](Self::is_known)
/// accepts either, transparently expiring stale superseded fingerprints.
#[derive(Clone, Default, Debug)]
pub struct PairedPeers {
    state: Arc<RwLock<PairedPeersInner>>,
}

impl PairedPeers {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a paired peer. `fingerprint` is hex(SHA-256(cert_der)).
    pub fn add(&self, fingerprint: impl Into<String>, display_name: impl Into<String>) {
        // A poisoned lock means another thread panicked mid-mutation; recover
        // the guard and continue — the allowlist is plain data, not an
        // invariant-bearing structure, so reading through poison is safe.
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        state
            .inner
            .insert(DeviceFingerprint(fingerprint.into()), display_name.into());
    }

    /// Atomically rotate a peer from `old_fingerprint` to `new_fingerprint`.
    ///
    /// The new fingerprint becomes the active identity immediately, while the
    /// old fingerprint stays accepted for [`CERT_ROTATION_GRACE`] so an
    /// in-flight handshake (or a `connect_with_retry` attempt) that still
    /// presents the previous certificate does not fail spuriously (S10).
    ///
    /// The display name is carried over from the old entry when present, else
    /// from `display_name`. If `old_fingerprint` is not currently known this is
    /// equivalent to [`add`](Self::add) for the new fingerprint (no superseded
    /// entry is created — there is nothing to grace).
    pub fn rotate_peer(
        &self,
        old_fingerprint: &str,
        new_fingerprint: impl Into<String>,
        display_name: impl Into<String>,
    ) {
        self.rotate_peer_at(
            old_fingerprint,
            new_fingerprint,
            display_name,
            Instant::now(),
        )
    }

    /// Test/seam variant of [`rotate_peer`](Self::rotate_peer) that takes an
    /// explicit `now` so grace-window expiry can be exercised deterministically.
    pub(super) fn rotate_peer_at(
        &self,
        old_fingerprint: &str,
        new_fingerprint: impl Into<String>,
        display_name: impl Into<String>,
        now: Instant,
    ) {
        let new_fp = new_fingerprint.into();
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        // Remember whether we actually knew the old fingerprint: only a
        // previously-known fingerprint is worth gracing (there is nothing to
        // race against if we never accepted it in the first place).
        let previous_name = state.inner.remove(old_fingerprint);
        let name = previous_name.clone().unwrap_or_else(|| display_name.into());

        // Grace the old fingerprint only when (a) we actually knew it, (b) it is
        // non-empty, and (c) it is not the same as the new active fingerprint.
        if previous_name.is_some() && !old_fingerprint.is_empty() && old_fingerprint != new_fp {
            state.superseded.insert(
                DeviceFingerprint(old_fingerprint.to_owned()),
                SupersededFingerprint {
                    display_name: name.clone(),
                    expires_at: now + CERT_ROTATION_GRACE,
                },
            );
        }

        state.inner.insert(DeviceFingerprint(new_fp), name);
    }

    /// Returns `true` if `fingerprint` belongs to a known paired peer.
    ///
    /// Accepts both active fingerprints and superseded ones still within their
    /// rotation grace window (S10). Expired superseded fingerprints are treated
    /// as unknown (and lazily pruned via [`prune_expired`](Self::prune_expired)).
    pub fn is_known(&self, fingerprint: &str) -> bool {
        self.is_known_at(fingerprint, Instant::now())
    }

    /// Test/seam variant of [`is_known`](Self::is_known) with an explicit clock.
    pub(super) fn is_known_at(&self, fingerprint: &str, now: Instant) -> bool {
        let state = self.state.read().unwrap_or_else(|e| e.into_inner());
        if state.inner.contains_key(fingerprint) {
            return true;
        }
        state
            .superseded
            .get(fingerprint)
            .is_some_and(|s| s.expires_at > now)
    }

    /// Drop any superseded fingerprints whose grace window has elapsed.
    ///
    /// Called opportunistically; correctness does not depend on it because
    /// [`is_known`](Self::is_known) already enforces expiry, but pruning keeps
    /// the map from growing across many rotations.
    pub fn prune_expired(&self) {
        let now = Instant::now();
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        state.superseded.retain(|_, s| s.expires_at > now);
    }

    /// Number of fingerprints currently in the rotation grace window.
    /// Exposed for tests and diagnostics.
    pub fn superseded_count(&self) -> usize {
        let state = self.state.read().unwrap_or_else(|e| e.into_inner());
        state.superseded.len()
    }

    /// Number of active (non-superseded) paired fingerprints.
    /// Exposed for tests and diagnostics (e.g. confirming `peers.json` loaded).
    pub fn active_count(&self) -> usize {
        let state = self.state.read().unwrap_or_else(|e| e.into_inner());
        state.inner.len()
    }

    /// Immediately remove a peer from the live allowlist (both active and
    /// superseded slots), effective for all future handshakes on this handle.
    ///
    /// Used by the revoke handlers so a revoked peer's mTLS session is no longer
    /// accepted on the next handshake — without waiting for a daemon restart.
    /// The `fingerprint` is normalised to lowercase before compare so callers
    /// may pass either the user-facing colon-hex form (after stripping colons) or
    /// the canonical lowercase hex the verifier uses.
    pub fn remove(&self, fingerprint: &str) {
        let canonical = fingerprint.to_ascii_lowercase();
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        state.inner.remove(canonical.as_str());
        state.superseded.remove(canonical.as_str());
    }

    /// Display name associated with a fingerprint, whether it is an active or a
    /// still-graced superseded fingerprint. Returns `None` for unknown/expired
    /// fingerprints. Used by diagnostics/UI that surface in-flight rotations.
    pub fn display_name_for(&self, fingerprint: &str) -> Option<String> {
        let state = self.state.read().unwrap_or_else(|e| e.into_inner());
        if let Some(name) = state.inner.get(fingerprint) {
            return Some(name.clone());
        }
        state
            .superseded
            .get(fingerprint)
            .filter(|s| s.expires_at > Instant::now())
            .map(|s| s.display_name.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── S10: cert rotation race — grace-period dual-fingerprint acceptance ───

    #[test]
    fn rotate_peer_accepts_both_old_and_new_during_grace() {
        let peers = PairedPeers::new();
        peers.add("old_fp", "Alice's Mac");
        assert!(peers.is_known("old_fp"));

        peers.rotate_peer("old_fp", "new_fp", "Alice's Mac");

        // New fingerprint is active; old one is still graced.
        assert!(peers.is_known("new_fp"), "rotated-to fp must be active");
        assert!(
            peers.is_known("old_fp"),
            "previous fp must stay valid during the grace window (S10)"
        );
        assert_eq!(peers.superseded_count(), 1);
    }

    // ── C-P0-4: revoke cuts off P2P (verifier rejects the removed peer) ──────

    /// `PairedPeers::remove` is what the daemon's revoke handlers call to evict
    /// a revoked peer from the live mTLS allowlist. After removal the peer's
    /// fingerprint must be unknown — `is_known` is the exact predicate the
    /// `PeerCertVerifier` consults to accept/reject a presented client/server
    /// certificate, so an unknown fingerprint means the mTLS handshake is
    /// rejected on the next attempt (P2P sync is cut off without a restart).
    #[test]
    fn remove_revokes_peer_so_verifier_rejects_it() {
        let peers = PairedPeers::new();
        peers.add("aabbccdd", "Bob's Phone");
        assert!(peers.is_known("aabbccdd"), "freshly paired peer is known");
        assert_eq!(peers.active_count(), 1);

        peers.remove("aabbccdd");

        assert!(
            !peers.is_known("aabbccdd"),
            "revoked peer must be unknown → verifier rejects its mTLS handshake"
        );
        assert_eq!(
            peers.active_count(),
            0,
            "revoked peer removed from allowlist"
        );
    }

    /// `remove` must also evict a peer that is currently in the cert-rotation
    /// grace window (superseded slot), so revoking during an in-flight rotation
    /// cannot leave a still-accepted fingerprint behind.
    #[test]
    fn remove_evicts_superseded_fingerprint_too() {
        let peers = PairedPeers::new();
        peers.add("oldfp", "Carol's Mac");
        peers.rotate_peer("oldfp", "newfp", "Carol's Mac");
        assert!(peers.is_known("oldfp"), "old fp graced before revoke");

        // Revoke both the active and the still-graced fingerprint.
        peers.remove("newfp");
        peers.remove("oldfp");

        assert!(!peers.is_known("newfp"), "active fp revoked");
        assert!(
            !peers.is_known("oldfp"),
            "superseded fp must also be evicted by remove"
        );
    }

    #[test]
    fn rotate_peer_old_fingerprint_rejected_after_grace_expires() {
        let peers = PairedPeers::new();
        peers.add("old_fp", "Alice's Mac");

        // Rotate at a fixed instant in the past so the grace window is already
        // over by `now`.
        let past = Instant::now() - (CERT_ROTATION_GRACE + Duration::from_secs(1));
        peers.rotate_peer_at("old_fp", "new_fp", "Alice's Mac", past);

        assert!(peers.is_known("new_fp"), "new fp always valid");
        assert!(
            !peers.is_known("old_fp"),
            "old fp must be rejected once the grace window elapses (S10)"
        );
    }

    #[test]
    fn is_known_at_honours_explicit_clock() {
        let peers = PairedPeers::new();
        peers.add("old_fp", "dev");
        let t0 = Instant::now();
        peers.rotate_peer_at("old_fp", "new_fp", "dev", t0);

        // Just inside the window: old fp accepted.
        let inside = t0 + CERT_ROTATION_GRACE - Duration::from_secs(1);
        assert!(peers.is_known_at("old_fp", inside));

        // Just past the window: old fp rejected.
        let outside = t0 + CERT_ROTATION_GRACE + Duration::from_secs(1);
        assert!(!peers.is_known_at("old_fp", outside));
        assert!(peers.is_known_at("new_fp", outside));
    }

    #[test]
    fn rotate_peer_carries_over_display_name() {
        let peers = PairedPeers::new();
        peers.add("old_fp", "Bob's Laptop");
        peers.rotate_peer("old_fp", "new_fp", "ignored-when-old-known");
        assert_eq!(
            peers.display_name_for("new_fp").as_deref(),
            Some("Bob's Laptop")
        );
        assert_eq!(
            peers.display_name_for("old_fp").as_deref(),
            Some("Bob's Laptop")
        );
    }

    #[test]
    fn rotate_peer_with_unknown_old_fp_just_adds_new() {
        let peers = PairedPeers::new();
        // No prior `add` for "old_fp".
        peers.rotate_peer("old_fp", "new_fp", "Carol");
        assert!(peers.is_known("new_fp"));
        assert!(
            !peers.is_known("old_fp"),
            "an unknown old fp must not be graced — nothing to grace"
        );
        assert_eq!(peers.superseded_count(), 0);
    }

    #[test]
    fn prune_expired_drops_only_stale_superseded() {
        let peers = PairedPeers::new();
        peers.add("a", "dev");
        // Expired rotation.
        peers.rotate_peer_at(
            "a",
            "b",
            "dev",
            Instant::now() - (CERT_ROTATION_GRACE + Duration::from_secs(5)),
        );
        // Fresh rotation away from the now-current "b".
        peers.rotate_peer("b", "c", "dev");

        assert_eq!(peers.superseded_count(), 2, "two superseded before prune");
        peers.prune_expired();
        assert_eq!(
            peers.superseded_count(),
            1,
            "stale entry pruned, fresh kept"
        );
        assert!(peers.is_known("c"));
        assert!(peers.is_known("b"), "freshly-superseded still graced");
        assert!(!peers.is_known("a"), "long-expired stays rejected");
    }

    #[test]
    fn rotation_into_same_fingerprint_creates_no_superseded() {
        let peers = PairedPeers::new();
        peers.add("fp", "dev");
        peers.rotate_peer("fp", "fp", "dev");
        assert!(peers.is_known("fp"));
        assert_eq!(
            peers.superseded_count(),
            0,
            "rotating to the same fp must not grace it against itself"
        );
    }
}

//! Durable `pending_unpair.json` queue (Gap A: best-effort `Unpair` delivery
//! to a peer that is offline at unpair time).
//!
//! Split out of the former flat `peers.rs` (ADR-017, CopyPaste-vp63.4) —
//! moved verbatim, no behavior change.

use std::path::Path;

use super::canonical_fp;
use super::store::save_json_atomic_0600;

/// A peer whose pairing was locally removed while it was offline, queued for a
/// best-effort `ControlMsg::Unpair` delivery on the next outbound connection.
///
/// Gap A (durable unpair): the live `try_send(Unpair)` is fire-and-forget and is
/// silently dropped when the peer is not connected at unpair time. To make the
/// signal durable we persist the peer's fingerprint + last-known dial address to
/// a SEPARATE `pending_unpair.json` file. That file is NEVER loaded into the live
/// `PairedPeers` allowlist (so the peer cannot sync), but the connector reads it
/// each tick, temporarily allow-lists the fingerprint, dials, sends `Unpair`,
/// then removes the entry. Records without an address cannot be dialed and are
/// retained until an address is learned (future improvement).
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct PendingUnpair {
    /// Canonical (or colon-hex) cert fingerprint of the unpaired peer.
    pub fingerprint: String,
    /// Last-known dial address (`host:port`), or `None` if never learned.
    #[serde(default)]
    pub address: Option<String>,
    /// Display name carried over from the removed `peers.json` record, used only
    /// for the transient `PairedPeers::add` during delivery.
    #[serde(default)]
    pub name: String,
}

/// Resolve the `pending_unpair.json` path sitting alongside a given
/// `peers.json` path (same parent directory). Keeps the two stores co-located so
/// the connector and the IPC handlers agree on the location.
pub fn pending_unpair_path_for(peers_path: &Path) -> std::path::PathBuf {
    match peers_path.parent() {
        Some(parent) => parent.join("pending_unpair.json"),
        None => std::path::PathBuf::from("pending_unpair.json"),
    }
}

/// Append a `PendingUnpair` record to `path` (the `pending_unpair.json` file),
/// de-duplicating by canonical fingerprint (a re-queue refreshes the address).
///
/// Called by the IPC unpair / revoke handlers after the peer has already been
/// removed from `peers.json` and the live `PairedPeers` allowlist. Best-effort
/// durability: a write failure is returned so the caller can log it, but the
/// local unpair has already committed regardless.
pub fn queue_pending_unpair(
    path: &Path,
    fingerprint: &str,
    address: Option<&str>,
    name: &str,
) -> anyhow::Result<()> {
    let target = canonical_fp(fingerprint);
    let mut pending = load_pending_unpairs(path);
    // Drop any stale entry for the same peer first (idempotent re-queue).
    pending.retain(|p| canonical_fp(&p.fingerprint) != target);
    pending.push(PendingUnpair {
        fingerprint: fingerprint.to_string(),
        address: address.map(|s| s.to_string()),
        name: name.to_string(),
    });
    save_pending_unpairs(path, &pending)
}

/// Load all queued `PendingUnpair` records from `path`. Returns an empty `Vec`
/// for a missing or unparseable file (same lenient contract as
/// [`super::store::load_peers`]).
pub fn load_pending_unpairs(path: &Path) -> Vec<PendingUnpair> {
    match std::fs::read_to_string(path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_else(|e| {
            tracing::warn!(
                "Failed to parse pending_unpair file {}: {e}",
                path.display()
            );
            Vec::new()
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => {
            tracing::warn!("Could not read pending_unpair file {}: {e}", path.display());
            Vec::new()
        }
    }
}

/// Persist `pending` to `path` (atomic 0600 write, same as
/// [`super::store::save_peers`]). An empty slice is written as `[]` so a
/// fully-drained queue leaves a valid (empty) file rather than a stale one.
pub fn save_pending_unpairs(path: &Path, pending: &[PendingUnpair]) -> anyhow::Result<()> {
    save_json_atomic_0600(path, pending)
}

/// Remove the `PendingUnpair` record for `fingerprint` from `path` after its
/// `Unpair` frame has been delivered (or determined undeliverable and dropped).
/// No-op when no matching record exists.
pub fn remove_pending_unpair(path: &Path, fingerprint: &str) -> anyhow::Result<()> {
    let target = canonical_fp(fingerprint);
    let mut pending = load_pending_unpairs(path);
    let before = pending.len();
    pending.retain(|p| canonical_fp(&p.fingerprint) != target);
    if pending.len() == before {
        return Ok(()); // nothing to remove
    }
    save_pending_unpairs(path, &pending)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// Gap A: `queue_pending_unpair` writes a record, `load_pending_unpairs`
    /// reads it back, and `remove_pending_unpair` drains it. A re-queue for the
    /// same fingerprint replaces (does not duplicate) the prior record.
    #[test]
    fn pending_unpair_queue_roundtrip_and_remove() {
        let dir = tempdir().unwrap();
        let peers_path = dir.path().join("peers.json");
        let pending_path = pending_unpair_path_for(&peers_path);
        assert_eq!(pending_path, dir.path().join("pending_unpair.json"));

        // Empty / missing file → empty vec.
        assert!(load_pending_unpairs(&pending_path).is_empty());

        // Queue one peer.
        queue_pending_unpair(&pending_path, "aa:bb:cc", Some("10.0.0.1:4242"), "Alice").unwrap();
        let loaded = load_pending_unpairs(&pending_path);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].fingerprint, "aa:bb:cc");
        assert_eq!(loaded[0].address.as_deref(), Some("10.0.0.1:4242"));
        assert_eq!(loaded[0].name, "Alice");

        // Re-queue the SAME peer (canonical match across colon-hex vs bare hex)
        // with a fresher address → replaces, never duplicates.
        queue_pending_unpair(&pending_path, "aabbcc", Some("10.0.0.2:5555"), "Alice2").unwrap();
        let loaded = load_pending_unpairs(&pending_path);
        assert_eq!(
            loaded.len(),
            1,
            "re-queue must dedupe by canonical fingerprint"
        );
        assert_eq!(loaded[0].address.as_deref(), Some("10.0.0.2:5555"));

        // Queue a second, distinct peer.
        queue_pending_unpair(&pending_path, "dd:ee:ff", None, "Bob").unwrap();
        assert_eq!(load_pending_unpairs(&pending_path).len(), 2);

        // Remove the first by canonical fingerprint.
        remove_pending_unpair(&pending_path, "AABBCC").unwrap();
        let loaded = load_pending_unpairs(&pending_path);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].fingerprint, "dd:ee:ff");
        assert_eq!(loaded[0].address, None);

        // Removing a non-present fingerprint is a no-op.
        remove_pending_unpair(&pending_path, "deadbeef").unwrap();
        assert_eq!(load_pending_unpairs(&pending_path).len(), 1);
    }

    /// Gap A: a pending_unpair.json store is written 0600 (it co-locates with
    /// the secret-bearing peers.json, so it inherits the same owner-only mode).
    #[cfg(unix)]
    #[test]
    fn pending_unpair_file_is_mode_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let pending_path = dir.path().join("pending_unpair.json");
        queue_pending_unpair(&pending_path, "aabbcc", Some("127.0.0.1:1"), "X").unwrap();
        let mode = std::fs::metadata(&pending_path)
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "pending_unpair.json must be 0600");
    }
}

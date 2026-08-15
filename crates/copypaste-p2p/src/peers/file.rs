//! Reading and replacing the file on disk.
//!
//! **This file is a key store.** Anything that can read it can impersonate
//! every paired device and decrypt every future sync session, so the file mode
//! is not hygiene, it is the access control:
//!
//! * Replaced through [`copypaste_fs::write_atomically`] at
//!   [`Visibility::OwnerOnly`], so a crash mid-write leaves the previous file
//!   intact and the keys never exist at a mode wider than `0600`. Losing a
//!   pairing means re-pairing every device by hand, and `AGENTS.md` rule 4 ranks
//!   data loss as the worst outcome.
//! * An existing file whose mode is wider than `0600` is reported at `warn`.
//!
//! v1 shipped this file with the OPAQUE `PasswordFile` in plaintext inside it,
//! recorded by its own ADR-008 as an unresolved Medium finding
//! (**CopyPaste-5lm**, port manifest 02 §3.7 and §6.3). v2 needs only the
//! 32-byte PSK, but the file is no less sensitive for being smaller.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use copypaste_fs::Visibility;
use zeroize::Zeroizing;

use super::peer::validate_pairing_id;
use super::{Peer, PeerStoreError, MAX_REVOCATIONS};

/// Everything the store holds, in memory and on disk.
///
/// `pending` and `revoked` are keyed by pairing id and hold no key material, so
/// they are ordinary maps beside the peers rather than fields on [`Peer`] —
/// which also keeps `Peer`'s literal shape, and therefore its `Drop`-guarded
/// PSK handling, unchanged.
#[derive(Default)]
pub(super) struct State {
    pub(super) peers: HashMap<String, Peer>,
    /// Pairings whose code has never been redeemed, and the instant each stops
    /// being redeemable.
    pub(super) pending: BTreeMap<String, i64>,
    /// Pairing ids that were cut off, and when. Kept after the peer is gone: it
    /// is the audit trail, and it is what stops a revoked pairing being
    /// re-added by a code someone still has.
    pub(super) revoked: BTreeMap<String, i64>,
    /// How many times each pairing's slot has been written in this process.
    /// In memory only: it exists so a pairing ceremony can tell its own
    /// tentative write from somebody else's (see [`super::tentative`]), and
    /// across a restart there is no ceremony left to tell apart.
    pub(super) generations: HashMap<String, u64>,
}

/// On-disk envelope.
#[derive(serde::Serialize, serde::Deserialize)]
struct StoreFile {
    peers: Vec<Peer>,
    #[serde(default)]
    pending: BTreeMap<String, i64>,
    #[serde(default)]
    revoked: BTreeMap<String, i64>,
}

/// Parse the persisted state.
pub(super) fn parse(bytes: &[u8]) -> Result<State, PeerStoreError> {
    let file: StoreFile = serde_json::from_slice(bytes).map_err(|_| PeerStoreError::Corrupt)?;
    let mut peers = HashMap::with_capacity(file.peers.len());
    for peer in file.peers {
        peer.validate().map_err(|_| PeerStoreError::Corrupt)?;
        peers.insert(peer.pairing_id.clone(), peer);
    }
    // A deadline or a revocation for a pairing that is not here decides nothing
    // about `pending`, but a stale `revoked` entry is load-bearing and is kept.
    let pending = file
        .pending
        .into_iter()
        .filter(|(id, _)| peers.contains_key(id))
        .collect();
    // Refused rather than trimmed. Dropping a revocation is what lets a device
    // someone still holds a code for be re-added, so the two safe answers to an
    // implausible list are to keep all of it or to open none of it.
    if file.revoked.len() > MAX_REVOCATIONS {
        return Err(PeerStoreError::Corrupt);
    }
    for id in file.revoked.keys() {
        validate_pairing_id(id).map_err(|_| PeerStoreError::Corrupt)?;
    }
    Ok(State {
        peers,
        pending,
        revoked: file.revoked,
        generations: HashMap::new(),
    })
}

pub(super) fn write_atomically(path: &Path, state: &State) -> Result<(), PeerStoreError> {
    blocking(|| write_now(path, state))
}

fn blocking<T>(f: impl FnOnce() -> T) -> T {
    use tokio::runtime::{Handle, RuntimeFlavor};
    match Handle::try_current() {
        Ok(handle) if matches!(handle.runtime_flavor(), RuntimeFlavor::MultiThread) => {
            tokio::task::block_in_place(f)
        }
        _ => f(),
    }
}

fn write_now(path: &Path, state: &State) -> Result<(), PeerStoreError> {
    let mut records: Vec<Peer> = state.peers.values().cloned().collect();
    records.sort_by(|a, b| a.pairing_id.cmp(&b.pairing_id));
    let file = StoreFile {
        peers: records,
        pending: state.pending.clone(),
        revoked: state.revoked.clone(),
    };
    // The serialised form contains every PSK in hex. Held in `Zeroizing` so the
    // buffer is wiped once it has been handed to the kernel.
    let json =
        Zeroizing::new(serde_json::to_vec_pretty(&file).map_err(|_| PeerStoreError::Corrupt)?);

    copypaste_fs::write_atomically(path, &json, Visibility::OwnerOnly).map_err(PeerStoreError::Io)
}

pub(super) fn only_last_seen_moved(state: &State, incoming: &Peer) -> bool {
    let Some(stored) = state.peers.get(&incoming.pairing_id) else {
        return false;
    };
    stored.last_seen_ms > 0
        && incoming.last_seen_ms > 0
        && !state.pending.contains_key(&incoming.pairing_id)
        && stored.name == incoming.name
        && stored.last_addr == incoming.last_addr
        && stored.psk_matches(&incoming.psk)
}

/// Warn — do not fail — if the file on disk is readable by anyone else. It
/// holds every pre-shared key.
#[cfg(unix)]
pub(super) fn warn_if_permissive(path: &Path) {
    use std::os::unix::fs::PermissionsExt as _;
    if let Ok(meta) = std::fs::metadata(path) {
        let mode = meta.permissions().mode() & 0o777;
        if mode & 0o077 != 0 {
            // The path itself is deliberately not in the message (rule 4).
            tracing::warn!(
                mode = format!("{mode:o}"),
                "the paired-devices file is readable beyond its owner; it contains pre-shared keys"
            );
        }
    }
}

#[cfg(not(unix))]
pub(super) fn warn_if_permissive(_path: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peers::testutil::{peer, store_path};
    use crate::peers::{PeerStore, DEFAULT_FILE_NAME};
    use crate::transport::PairingToken;

    fn file_with_revocations(revoked: &str) -> Vec<u8> {
        format!(r#"{{"peers":[],"pending":{{}},"revoked":{{{revoked}}}}}"#).into_bytes()
    }

    /// The list is the thing that stops a revoked device coming back, so an
    /// implausible one is refused whole rather than trimmed to fit: trimming
    /// re-admits whichever device the dropped entry was cutting off.
    #[test]
    fn a_revocation_list_past_the_cap_is_refused_not_trimmed() {
        let entries: Vec<String> = (0..=MAX_REVOCATIONS)
            .map(|i| format!(r#""pairing-{i}":1"#))
            .collect();
        let bytes = file_with_revocations(&entries.join(","));
        assert!(
            matches!(parse(&bytes), Err(PeerStoreError::Corrupt)),
            "an oversized revocation list was accepted"
        );

        // One under the cap still opens: the bound refuses absurdity, not use.
        let entries: Vec<String> = (0..MAX_REVOCATIONS)
            .map(|i| format!(r#""pairing-{i}":1"#))
            .collect();
        let state = parse(&file_with_revocations(&entries.join(","))).expect("at the cap");
        assert_eq!(state.revoked.len(), MAX_REVOCATIONS);
    }

    #[test]
    fn a_revoked_id_no_peer_could_carry_is_refused() {
        let long = "a".repeat(129);
        for entry in [r#""":1"#.to_string(), format!(r#""{long}":1"#)] {
            assert!(
                matches!(
                    parse(&file_with_revocations(&entry)),
                    Err(PeerStoreError::Corrupt)
                ),
                "an invalid revoked id was accepted: {entry}"
            );
        }
    }

    #[test]
    fn a_torn_write_cannot_be_observed() {
        // Atomicity is a rename, so the only two states the file can be in are
        // "previous contents" and "new contents". Pin the property that matters
        // for recovery: after any number of writes, the file on disk always
        // parses.
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);
        let store = PeerStore::open(&path).expect("open");
        for i in 0..12 {
            store.upsert(peer(&format!("device-{i}"))).expect("upsert");
            let reopened = PeerStore::open(&path).expect("file must always parse");
            assert_eq!(reopened.usable_count(), i + 1);
        }
    }

    #[test]
    fn a_write_from_a_reactor_worker_completes_off_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .build()
            .expect("runtime");
        runtime.block_on(async {
            let store = PeerStore::open(&path).expect("open");
            store.upsert(peer("Laptop")).expect("upsert");
            store.upsert(peer("Phone")).expect("upsert");
        });
        assert_eq!(PeerStore::open(&path).expect("reopen").usable_count(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn the_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);
        let store = PeerStore::open(&path).expect("open");
        store.upsert(peer("Laptop")).expect("upsert");

        let mode = std::fs::metadata(&path).expect("meta").permissions().mode() & 0o777;
        assert_eq!(
            mode,
            copypaste_fs::OWNER_ONLY_MODE,
            "file holds PSKs; mode must be 0600"
        );

        // And a rewrite must not widen it.
        store.upsert(peer("Phone")).expect("second upsert");
        let mode = std::fs::metadata(&path).expect("meta").permissions().mode() & 0o777;
        assert_eq!(mode, copypaste_fs::OWNER_ONLY_MODE);
    }

    #[test]
    fn a_damaged_file_is_reported_and_never_discarded() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);

        for bad in [
            &b"not json at all"[..],
            &b""[..],
            br#"{"peers":"nope"}"#,
            br#"{"peers":[{"pairing_id":"a","name":"n","psk":"ab","last_addr":null,"last_seen_ms":0}]}"#,
            br#"{"peers":[{"pairing_id":"a","name":"n","psk":"zzzz","last_addr":null,"last_seen_ms":0}]}"#,
        ] {
            std::fs::write(&path, bad).expect("write");
            assert!(
                matches!(PeerStore::open(&path), Err(PeerStoreError::Corrupt)),
                "must reject: {}",
                String::from_utf8_lossy(bad)
            );
            // Never repaired by overwriting: the file is the only copy of the
            // pairings, and losing it costs a manual re-pair of every device.
            assert_eq!(std::fs::read(&path).expect("still there"), bad);
        }
    }

    #[test]
    fn the_serialised_file_holds_the_psk_as_hex_and_nothing_else_secret() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);
        let store = PeerStore::open(&path).expect("open");
        let p = peer("Laptop");
        let psk = p.psk;
        let pairing_id = p.pairing_id.clone();
        store.upsert(p).expect("upsert");

        let text = std::fs::read_to_string(&path).expect("read");
        assert!(text.contains(&pairing_id));
        assert!(
            text.contains(&hex::encode(psk)),
            "psk must round-trip as hex"
        );
        // The pairing code form of the secret must never be written anywhere.
        let token = PairingToken::from_bytes(&psk);
        assert!(!text.contains(&token.to_code()));
    }

    #[test]
    fn a_store_in_a_directory_that_does_not_exist_yet_creates_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir
            .path()
            .join("nested")
            .join("deeper")
            .join(DEFAULT_FILE_NAME);
        let store = PeerStore::open(&path).expect("open");
        store.upsert(peer("Laptop")).expect("upsert");
        assert!(path.exists());
        assert_eq!(PeerStore::open(&path).expect("reopen").usable_count(), 1);
    }
}

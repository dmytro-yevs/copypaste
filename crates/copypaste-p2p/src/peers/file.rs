//! Reading and replacing the file on disk.
//!
//! State is encrypted before owner-only atomic replacement. Plaintext exists
//! only in zeroized memory; migration replaces the old file without a backup.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use copypaste_fs::Visibility;
use zeroize::Zeroizing;

use super::crypto;
use super::peer::validate_pairing_id;
use super::{Peer, PeerStoreError, MAX_REVOCATIONS};

/// Everything the store holds, in memory and on disk.
///
/// `revoked` is keyed by pairing id and holds no key material, so it is an
/// ordinary map beside the peers rather than a field on [`Peer`].
#[derive(Default)]
pub(super) struct State {
    pub(super) peers: HashMap<String, Peer>,
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
#[serde(deny_unknown_fields)]
struct StoreFile {
    peers: Vec<Peer>,
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
        revoked: file.revoked,
        generations: HashMap::new(),
    })
}

pub(super) fn write_atomically(
    path: &Path,
    state: &State,
    key: &[u8; 32],
) -> Result<(), PeerStoreError> {
    blocking(|| write_now(path, state, key))
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

fn write_now(path: &Path, state: &State, key: &[u8; 32]) -> Result<(), PeerStoreError> {
    let mut records: Vec<Peer> = state.peers.values().cloned().collect();
    records.sort_by(|a, b| a.pairing_id.cmp(&b.pairing_id));
    let file = StoreFile {
        peers: records,
        revoked: state.revoked.clone(),
    };
    let json =
        Zeroizing::new(serde_json::to_vec_pretty(&file).map_err(|_| PeerStoreError::Corrupt)?);
    let envelope = crypto::seal(&json, key)?;
    copypaste_fs::write_atomically(path, &envelope, Visibility::OwnerOnly)
        .map_err(PeerStoreError::Io)
}

pub(super) fn migrate_plaintext(path: &Path, key: &[u8; 32]) -> Result<(), PeerStoreError> {
    migrate_with(path, key, write_atomically)
}

fn migrate_with(
    path: &Path,
    key: &[u8; 32],
    persist: impl FnOnce(&Path, &State, &[u8; 32]) -> Result<(), PeerStoreError>,
) -> Result<(), PeerStoreError> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => Zeroizing::new(bytes),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(PeerStoreError::Io(err)),
    };
    if bytes.starts_with(crypto::MAGIC) {
        // A recognized encrypted file must authenticate; never retry it as JSON.
        parse(&crypto::open(&bytes, key)?)?;
        return Ok(());
    }
    let state = parse(&bytes)?;
    persist(path, &state, key)
}

pub(super) fn only_last_seen_moved(state: &State, incoming: &Peer) -> bool {
    let Some(stored) = state.peers.get(&incoming.pairing_id) else {
        return false;
    };
    stored.name == incoming.name
        && stored.device_id == incoming.device_id
        && stored.last_addr == incoming.last_addr
        && stored.psk_matches(&incoming.psk)
        && stored.profile == incoming.profile
        && stored.profile_observed_at_ms == incoming.profile_observed_at_ms
}

/// Retain owner-only permissions in addition to encryption.
#[cfg(unix)]
pub(super) fn warn_if_permissive(path: &Path) {
    use std::os::unix::fs::PermissionsExt as _;
    if let Ok(meta) = std::fs::metadata(path) {
        let mode = meta.permissions().mode() & 0o777;
        if mode & 0o077 != 0 {
            // The path itself is deliberately not in the message (rule 4).
            tracing::warn!(
                mode = format!("{mode:o}"),
                "the paired-devices file is readable beyond its owner"
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
        format!(r#"{{"peers":[],"revoked":{{{revoked}}}}}"#).into_bytes()
    }

    #[test]
    fn a_retired_pending_field_is_refused() {
        let file = br#"{"peers":[],"pending":{},"revoked":{}}"#;
        assert!(matches!(parse(file), Err(PeerStoreError::Corrupt)));
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
        let store = PeerStore::open(&path, &crate::peers::testutil::KEY).expect("open");
        for i in 0..12 {
            store.upsert(peer(&format!("device-{i}"))).expect("upsert");
            let reopened = PeerStore::open(&path, &crate::peers::testutil::KEY)
                .expect("file must always parse");
            assert_eq!(reopened.len(), i + 1);
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
            let store = PeerStore::open(&path, &crate::peers::testutil::KEY).expect("open");
            store.upsert(peer("Laptop")).expect("upsert");
            store.upsert(peer("Phone")).expect("upsert");
        });
        assert_eq!(
            PeerStore::open(&path, &crate::peers::testutil::KEY)
                .expect("reopen")
                .len(),
            2
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);
        let store = PeerStore::open(&path, &crate::peers::testutil::KEY).expect("open");
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
            br#"{"peers":[{"pairing_id":"a","name":"n","psk":"ab","last_addr":null,"last_seen_ms":1}],"revoked":{}}"#,
            br#"{"peers":[{"pairing_id":"a","name":"n","psk":"zzzz","last_addr":null,"last_seen_ms":1}],"revoked":{}}"#,
        ] {
            std::fs::write(&path, bad).expect("write");
            assert!(
                matches!(PeerStore::open(&path, &crate::peers::testutil::KEY), Err(PeerStoreError::Corrupt)),
                "must reject: {}",
                String::from_utf8_lossy(bad)
            );
            // Never repaired by overwriting: the file is the only copy of the
            // pairings, and losing it costs a manual re-pair of every device.
            assert_eq!(std::fs::read(&path).expect("still there"), bad);
        }
    }

    #[test]
    fn the_file_exposes_neither_pairing_keys_nor_peer_metadata() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);
        let store = PeerStore::open(&path, &crate::peers::testutil::KEY).expect("open");
        let p = peer("Laptop");
        let psk = p.psk;
        let pairing_id = p.pairing_id.clone();
        store.upsert(p).expect("upsert");

        let bytes = std::fs::read(&path).expect("read");
        let text = String::from_utf8_lossy(&bytes);
        assert!(bytes.starts_with(crypto::MAGIC));
        assert!(!text.contains(&pairing_id));
        assert!(!text.contains("Laptop"));
        assert!(!text.contains(&hex::encode(psk)));
        assert!(!bytes.windows(psk.len()).any(|window| window == psk));
        let token = PairingToken::from_bytes(&psk);
        assert!(!text.contains(&token.to_code()));
        let reopened = PeerStore::open(&path, &crate::peers::testutil::KEY).expect("reopen");
        assert!(reopened.get(&pairing_id).unwrap().psk_matches(&psk));
    }

    #[test]
    fn wrong_keys_and_tampered_envelopes_are_refused_without_rewriting() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);
        let store = PeerStore::open(&path, &crate::peers::testutil::KEY).unwrap();
        store.upsert(peer("Laptop")).unwrap();
        let original = std::fs::read(&path).unwrap();
        assert!(matches!(
            PeerStore::open(&path, &[18; 32]),
            Err(PeerStoreError::Corrupt)
        ));
        assert_eq!(std::fs::read(&path).unwrap(), original);

        for offset in [0, 3, 4, original.len() - 1] {
            let mut tampered = original.clone();
            tampered[offset] ^= 1;
            std::fs::write(&path, &tampered).unwrap();
            assert!(matches!(
                PeerStore::open(&path, &crate::peers::testutil::KEY),
                Err(PeerStoreError::Corrupt)
            ));
            assert_eq!(std::fs::read(&path).unwrap(), tampered);
        }
        for len in [0, 4, 27, original.len() - 1] {
            std::fs::write(&path, &original[..len]).unwrap();
            assert!(matches!(
                PeerStore::open(&path, &crate::peers::testutil::KEY),
                Err(PeerStoreError::Corrupt)
            ));
        }
    }

    #[test]
    fn each_rewrite_uses_a_fresh_nonce_and_leaves_no_plaintext_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);
        let mut state = State::default();
        let record = peer("Laptop");
        state.peers.insert(record.pairing_id.clone(), record);
        write_atomically(&path, &state, &crate::peers::testutil::KEY).unwrap();
        let first = std::fs::read(&path).unwrap();
        write_atomically(&path, &state, &crate::peers::testutil::KEY).unwrap();
        let second = std::fs::read(&path).unwrap();
        assert_ne!(&first[4..28], &second[4..28]);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        assert_eq!(
            PeerStore::open(&path, &crate::peers::testutil::KEY)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn migration_write_failure_preserves_the_original_and_refuses_to_open() {
        let dir = tempfile::tempdir().unwrap();
        let path = store_path(&dir);
        let legacy = file_with_revocations(r#""revoked-device":42"#);
        std::fs::write(&path, &legacy).unwrap();
        let result = migrate_with(&path, &crate::peers::testutil::KEY, |_, state, _| {
            assert_eq!(state.revoked.get("revoked-device"), Some(&42));
            Err(PeerStoreError::Io(std::io::Error::other("write failure")))
        });
        assert!(matches!(result, Err(PeerStoreError::Io(_))));
        assert_eq!(std::fs::read(&path).unwrap(), legacy);
        assert!(matches!(
            PeerStore::open(&path, &crate::peers::testutil::KEY),
            Err(PeerStoreError::Corrupt)
        ));
    }

    #[test]
    fn damaged_encrypted_migration_never_falls_back_or_rewrites() {
        let dir = tempfile::tempdir().unwrap();
        let path = store_path(&dir);
        let mut envelope =
            crypto::seal(&file_with_revocations(""), &crate::peers::testutil::KEY).unwrap();
        *envelope.last_mut().unwrap() ^= 1;
        std::fs::write(&path, &envelope).unwrap();
        let result = migrate_with(&path, &crate::peers::testutil::KEY, |_, _, _| {
            panic!("a damaged encrypted file must never be rewritten")
        });
        assert!(matches!(result, Err(PeerStoreError::Corrupt)));
        assert_eq!(std::fs::read(&path).unwrap(), envelope);
    }

    #[test]
    fn a_store_in_a_directory_that_does_not_exist_yet_creates_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir
            .path()
            .join("nested")
            .join("deeper")
            .join(DEFAULT_FILE_NAME);
        let store = PeerStore::open(&path, &crate::peers::testutil::KEY).expect("open");
        store.upsert(peer("Laptop")).expect("upsert");
        assert!(path.exists());
        assert_eq!(
            PeerStore::open(&path, &crate::peers::testutil::KEY)
                .expect("reopen")
                .len(),
            1
        );
    }
}

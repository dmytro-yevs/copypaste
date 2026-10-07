//! One-way pairing-store migration, guarded by the encrypted history database.

use std::path::Path;

use copypaste_p2p::{PeerStore, PeerStoreError};

use crate::{Keyring, Store, StoreError};

const MIGRATED: &str = "lan_peer_store_encrypted";

#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error("the paired-device migration state is unavailable")]
    State(#[from] StoreError),
    #[error(transparent)]
    Peers(#[from] PeerStoreError),
}

/// Preserve existing pairings once, then refuse plaintext on every later start.
pub fn open(store: &Store, keyring: &Keyring, path: &Path) -> Result<PeerStore, OpenError> {
    let key = keyring.peer_store_key();
    let migrated = store.state(MIGRATED)?.is_some();
    if !migrated {
        PeerStore::migrate_plaintext(path, &key)?;
    }
    let peers = PeerStore::open(path, &key)?;
    if !migrated {
        // Commit only after encrypted state is durable and readable. A crash
        // before this point safely reopens the encrypted file next time.
        store.set_state(MIGRATED, "1")?;
    }
    Ok(peers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_p2p::{PairingToken, Peer};

    fn fixture() -> (Store, Keyring, tempfile::TempDir) {
        let keyring = Keyring::from_secret(&[7; 32]);
        let store = Store::open_in_memory(&keyring.db_key()).unwrap();
        (store, keyring, tempfile::tempdir().unwrap())
    }

    fn peer() -> Peer {
        let token = PairingToken::generate();
        Peer {
            pairing_id: token.pairing_id(),
            device_id: Some("stable-device".into()),
            name: "Laptop".into(),
            psk: token.psk(),
            last_addr: None,
            last_seen_ms: 42,
            profile: None,
            profile_observed_at_ms: 0,
        }
    }

    fn legacy(record: &Peer) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "peers": [record],
            "revoked": {"retired-device": 41}
        }))
        .unwrap()
    }

    #[test]
    fn migration_preserves_pairings_and_revocations_without_a_plaintext_copy() {
        let (store, keyring, dir) = fixture();
        let path = dir.path().join("peers.json");
        let record = peer();
        let plaintext = legacy(&record);
        std::fs::write(&path, &plaintext).unwrap();

        let migrated = open(&store, &keyring, &path).unwrap();
        let actual = migrated.get(&record.pairing_id).unwrap();
        assert!(actual.psk_matches(&record.psk));
        assert_eq!(actual.device_id, record.device_id);
        assert_eq!(actual.name, record.name);
        assert_eq!(actual.last_seen_ms, record.last_seen_ms);
        assert_eq!(migrated.revoked()[0].pairing_id, "retired-device");
        assert_eq!(migrated.revoked()[0].revoked_at_ms, 41);
        assert_eq!(store.state(MIGRATED).unwrap().as_deref(), Some("1"));
        let ciphertext = std::fs::read(&path).unwrap();
        assert_ne!(ciphertext, plaintext);
        assert!(!String::from_utf8_lossy(&ciphertext).contains(&hex::encode(record.psk)));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        assert_eq!(open(&store, &keyring, &path).unwrap().len(), 1);
    }

    #[test]
    fn a_plaintext_replacement_after_migration_is_refused_and_preserved() {
        let (store, keyring, dir) = fixture();
        let path = dir.path().join("peers.json");
        let record = peer();
        let plaintext = legacy(&record);
        std::fs::write(&path, &plaintext).unwrap();
        open(&store, &keyring, &path).unwrap();

        std::fs::write(&path, &plaintext).unwrap();
        assert!(matches!(
            open(&store, &keyring, &path),
            Err(OpenError::Peers(PeerStoreError::Corrupt))
        ));
        assert_eq!(std::fs::read(&path).unwrap(), plaintext);
    }

    #[test]
    fn first_run_also_disables_later_plaintext_import() {
        let (store, keyring, dir) = fixture();
        let path = dir.path().join("peers.json");
        assert!(open(&store, &keyring, &path).unwrap().is_empty());
        assert!(!path.exists());
        assert_eq!(store.state(MIGRATED).unwrap().as_deref(), Some("1"));

        std::fs::write(&path, legacy(&peer())).unwrap();
        assert!(matches!(
            open(&store, &keyring, &path),
            Err(OpenError::Peers(PeerStoreError::Corrupt))
        ));
    }

    #[test]
    fn restart_between_encrypted_replacement_and_marker_commit_is_safe() {
        let (store, keyring, dir) = fixture();
        let path = dir.path().join("peers.json");
        let record = peer();
        std::fs::write(&path, legacy(&record)).unwrap();
        PeerStore::migrate_plaintext(&path, &keyring.peer_store_key()).unwrap();
        let ciphertext = std::fs::read(&path).unwrap();
        assert_eq!(store.state(MIGRATED).unwrap(), None);

        let peers = open(&store, &keyring, &path).unwrap();
        assert!(peers
            .get(&record.pairing_id)
            .unwrap()
            .psk_matches(&record.psk));
        assert_eq!(std::fs::read(&path).unwrap(), ciphertext);
        assert_eq!(store.state(MIGRATED).unwrap().as_deref(), Some("1"));
    }

    #[test]
    fn damaged_state_cannot_complete_migration_or_reset_trust() {
        let (store, keyring, dir) = fixture();
        let path = dir.path().join("peers.json");
        let peers = PeerStore::open(&path, &keyring.peer_store_key()).unwrap();
        peers.upsert(peer()).unwrap();
        let mut ciphertext = std::fs::read(&path).unwrap();
        *ciphertext.last_mut().unwrap() ^= 1;
        std::fs::write(&path, &ciphertext).unwrap();
        assert!(matches!(
            open(&store, &keyring, &path),
            Err(OpenError::Peers(PeerStoreError::Corrupt))
        ));
        assert_eq!(store.state(MIGRATED).unwrap(), None);
        assert_eq!(std::fs::read(&path).unwrap(), ciphertext);
    }
}

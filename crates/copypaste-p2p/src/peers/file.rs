//! Reading and replacing the file on disk.
//!
//! **This file is a key store.** Anything that can read it can impersonate
//! every paired device and decrypt every future sync session, so the file mode
//! is not hygiene, it is the access control:
//!
//! * Written to a temporary file in the same directory and `rename`d over the
//!   target, so a crash mid-write leaves the previous file intact rather than a
//!   truncated one. Losing a pairing means re-pairing every device by hand, and
//!   `CLAUDE.md` rule 4 ranks data loss as the worst outcome.
//! * `0600` on unix, set on the temporary file *before* any key material goes
//!   into it, so the keys never exist at a wider mode. `tempfile` already
//!   creates at `0600`; setting it explicitly lets a test pin the property.
//! * An existing file whose mode is wider than `0600` is reported at `warn`.
//!
//! v1 shipped this file with the OPAQUE `PasswordFile` in plaintext inside it,
//! recorded by its own ADR-008 as an unresolved Medium finding
//! (**CopyPaste-5lm**, port manifest 02 §3.7 and §6.3). v2 needs only the
//! 32-byte PSK, but the file is no less sensitive for being smaller.

use std::collections::{BTreeMap, HashMap};
use std::io::Write as _;
use std::path::Path;

use zeroize::Zeroizing;

use super::{Peer, PeerStoreError};

/// File mode for the store on unix.
#[cfg(unix)]
pub(super) const STORE_MODE: u32 = 0o600;

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
    Ok(State {
        peers,
        pending,
        revoked: file.revoked,
    })
}

/// Replace the store file in one step: temporary file in the same directory (a
/// cross-filesystem `rename` is not atomic), `0600` before any bytes go in,
/// `fsync` the file so the contents are durable before the rename publishes
/// them, then `fsync` the directory so the rename survives a power loss.
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

    let dir = path.parent().filter(|p| !p.as_os_str().is_empty());
    if let Some(dir) = dir {
        std::fs::create_dir_all(dir).map_err(PeerStoreError::Io)?;
    }
    let dir = dir.unwrap_or_else(|| Path::new("."));

    let mut tmp = tempfile::NamedTempFile::new_in(dir).map_err(PeerStoreError::Io)?;
    restrict(tmp.as_file())?;
    tmp.write_all(&json).map_err(PeerStoreError::Io)?;
    tmp.as_file().sync_all().map_err(PeerStoreError::Io)?;
    // `persist` is `rename(2)`: the target either has the old contents or the
    // new ones, never a mixture and never nothing.
    tmp.persist(path)
        .map_err(|err| PeerStoreError::Io(err.error))?;
    sync_dir(dir);
    Ok(())
}

/// `0600`, applied before the file has any content.
#[cfg(unix)]
fn restrict(file: &std::fs::File) -> Result<(), PeerStoreError> {
    use std::os::unix::fs::PermissionsExt as _;
    file.set_permissions(std::fs::Permissions::from_mode(STORE_MODE))
        .map_err(PeerStoreError::Io)
}

/// No-op off unix. Scope is macOS and Android (`CLAUDE.md` rule 5), both of
/// which are unix; this exists so the crate still builds on a Windows CI
/// runner rather than as a supported configuration.
#[cfg(not(unix))]
fn restrict(_file: &std::fs::File) -> Result<(), PeerStoreError> {
    Ok(())
}

/// Make the rename durable. Best-effort: some filesystems refuse to open a
/// directory for sync, and failing the whole write for that would be worse than
/// the residual risk of losing the rename to a power cut.
fn sync_dir(dir: &Path) {
    if let Ok(handle) = std::fs::File::open(dir) {
        let _ = handle.sync_all();
    }
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
            let store = PeerStore::open(&path).expect("open");
            store.upsert(peer("Laptop")).expect("upsert");
            store.upsert(peer("Phone")).expect("upsert");
        });
        assert_eq!(PeerStore::open(&path).expect("reopen").len(), 2);
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
        assert_eq!(mode, STORE_MODE, "file holds PSKs; mode must be 0600");

        // And a rewrite must not widen it.
        store.upsert(peer("Phone")).expect("second upsert");
        let mode = std::fs::metadata(&path).expect("meta").permissions().mode() & 0o777;
        assert_eq!(mode, STORE_MODE);
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
        assert_eq!(PeerStore::open(&path).expect("reopen").len(), 1);
    }
}

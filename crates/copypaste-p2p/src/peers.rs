//! The list of devices this one is paired with, and their pre-shared keys.
//!
//! # What this module is
//!
//! A small, file-backed, thread-safe map from [`crate::PairingToken::pairing_id`] to
//! [`Peer`]. The daemon reads it to decide who may connect ([`PeerStore::psks`]
//! feeds [`crate::Session::accept_any`]) and to remember where a peer was last
//! seen so a reconnect does not have to wait for mDNS.
//!
//! ```text
//! peers-v2.json  (0600)
//! {
//!   "format": "copypaste-peers-v2",
//!   "peers": [
//!     { "pairing_id": "…32 hex…", "name": "Laptop",
//!       "psk": "…64 hex…", "last_addr": "192.168.1.7:47654",
//!       "last_seen_ms": 1753900000000 }
//!   ]
//! }
//! ```
//!
//! # This file is a key store
//!
//! It holds the pre-shared keys. Anything that can read it can impersonate
//! every paired device and decrypt every future sync session, so the file mode
//! is not hygiene, it is the access control:
//!
//! * Written through a temporary file in the same directory and `rename`d over
//!   the target, so a crash mid-write leaves the previous file intact rather
//!   than a truncated one. Losing a pairing means re-pairing every device by
//!   hand; `CLAUDE.md` rule 4 ranks data loss as the worst outcome.
//! * `0600` on unix, set on the temporary file *before* any key material is
//!   written to it, so there is no window in which the keys exist at a wider
//!   mode. `tempfile` already creates at `0600`; setting it explicitly means a
//!   test can pin the property instead of trusting a dependency's default.
//! * An existing file whose mode is wider than `0600` is reported at `warn`.
//!
//! v1 shipped this file with the OPAQUE `PasswordFile` in plaintext inside it
//! and its own ADR-008 recorded that as an unresolved Medium finding
//! (**CopyPaste-5lm**, port manifest 02 §3.7 and §6.3). v2 has no
//! `PasswordFile` — `NNpsk0` needs only the 32-byte PSK — but the file is no
//! less sensitive for being smaller, so the handling above is the answer to
//! that finding rather than a repeat of it.
//!
//! # No backward compatibility
//!
//! `CLAUDE.md` rule 3: v2 must not open, or appear to open, anything v1 wrote.
//! Two guards, both cheap:
//!
//! * [`DEFAULT_FILE_NAME`] is `peers-v2.json`, so v1's `peers.json` is never
//!   touched and a user who downgrades finds it intact.
//! * The envelope carries a `format` tag. A file that is valid JSON but not
//!   tagged [`FORMAT_TAG`] is [`PeerStoreError::Legacy`], a plain "this was
//!   written by a different version" — not a decryption error, and not silently
//!   overwritten.

use std::collections::HashMap;
use std::fmt;
use std::io::Write as _;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use subtle::ConstantTimeEq;
use zeroize::{Zeroize, Zeroizing};

use crate::transport::TOKEN_LEN;

/// Filename the daemon should use.
///
/// Deliberately not `peers.json`: that is v1's name, and v2 must never open a
/// v1 file (`CLAUDE.md` rule 3).
pub const DEFAULT_FILE_NAME: &str = "peers-v2.json";

/// Envelope tag. Its only job is to make "written by another version" a
/// distinguishable answer from "corrupt".
pub const FORMAT_TAG: &str = "copypaste-peers-v2";

/// File mode for the store on unix.
#[cfg(unix)]
const STORE_MODE: u32 = 0o600;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Every failure this module can produce.
///
/// No variant can render a filesystem path (`CLAUDE.md` rule 4 — the daemon's
/// data directory discloses the local username). The `io::Error` is a
/// `#[source]`, so it is available to a log sink but never to this type's own
/// `Display`; `std`'s file errors do not embed the path either, but relying on
/// that would be relying on an implementation detail.
#[derive(Debug, thiserror::Error)]
pub enum PeerStoreError {
    /// The store could not be read, written or replaced.
    #[error("the paired-devices file could not be read or written")]
    Io(#[source] std::io::Error),

    /// The file is not valid JSON, or is tagged as this format but does not
    /// match its shape.
    ///
    /// Never repaired by overwriting. The file holds the only copy of every
    /// pairing; discarding it silently would cost the user a manual re-pair of
    /// every device.
    #[error("the paired-devices file is damaged")]
    Corrupt,

    /// The file was written by a different version of CopyPaste.
    ///
    /// v2 shares no formats with v0.4.x. The daemon should say exactly that:
    /// the old pairings are still on disk and still readable by the old build,
    /// and v2 needs the devices paired again.
    #[error("the paired-devices file was written by a different version of CopyPaste")]
    Legacy,

    /// A peer record failed validation.
    #[error("peer record is invalid: {0}")]
    Invalid(&'static str),

    /// A thread panicked while holding the store's lock.
    ///
    /// Surfaced rather than swallowed: the in-memory map may have been observed
    /// mid-update, and this store's contents decide who is allowed to connect.
    #[error("the paired-devices store is no longer usable in this process")]
    Poisoned,
}

// ---------------------------------------------------------------------------
// Peer
// ---------------------------------------------------------------------------

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

    /// Unix milliseconds of the last successful contact. Advisory — this
    /// module never compares it against a local clock, so peer clock skew
    /// cannot affect anything here.
    pub last_seen_ms: i64,
}

impl Peer {
    /// Constant-time comparison of this peer's PSK against a candidate.
    ///
    /// `==` on key bytes short-circuits at the first differing byte and leaks
    /// the matching-prefix length by timing (port manifest 02, I-13). This is
    /// the only PSK comparison this type offers.
    #[must_use]
    pub fn psk_matches(&self, candidate: &[u8; TOKEN_LEN]) -> bool {
        self.psk[..].ct_eq(&candidate[..]).into()
    }

    fn validate(&self) -> Result<(), PeerStoreError> {
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
/// 02, I-12).
///
/// Note the consequence: `Peer` has a `Drop`, so its fields cannot be moved out
/// individually (`let Peer { psk, .. } = peer;` will not compile). Clone the
/// field instead. That is the intended friction — every copy of a PSK should be
/// a deliberate act.
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

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

/// On-disk envelope. Separate from the in-memory map so the `format` tag is
/// checked before anything else is believed.
#[derive(serde::Serialize, serde::Deserialize)]
struct StoreFile {
    format: String,
    peers: Vec<Peer>,
}

/// The paired-device list.
///
/// Backed by a file, cached in memory, safe to share across the daemon's tasks
/// behind an `Arc`. Every mutation writes the whole file — there are a handful
/// of peers, not a database — and every write is atomic.
///
/// The read methods never touch the disk, so the hot path (a listener checking
/// PSKs on every inbound connection) costs a read lock and nothing else.
pub struct PeerStore {
    path: PathBuf,
    peers: RwLock<HashMap<String, Peer>>,
}

impl PeerStore {
    /// Open the store at `path`, creating an empty one in memory if the file
    /// does not exist yet.
    ///
    /// Nothing is written until the first mutation, so opening a store for a
    /// device that has never paired leaves the filesystem untouched.
    ///
    /// # Errors
    ///
    /// [`PeerStoreError::Legacy`] if the file was written by another version —
    /// distinguishable on purpose, so the daemon can explain rather than report
    /// corruption. [`PeerStoreError::Corrupt`] if it is damaged;
    /// [`PeerStoreError::Io`] if it could not be read.
    pub fn open(path: &Path) -> Result<Self, PeerStoreError> {
        let peers = match std::fs::read(path) {
            Ok(bytes) => {
                warn_if_permissive(path);
                let text = Zeroizing::new(bytes);
                parse(&text)?
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
            Err(err) => return Err(PeerStoreError::Io(err)),
        };
        Ok(Self {
            path: path.to_path_buf(),
            peers: RwLock::new(peers),
        })
    }

    /// Every known peer, ordered by pairing id so callers and tests see a
    /// stable sequence.
    #[must_use]
    pub fn list(&self) -> Vec<Peer> {
        let Ok(guard) = self.peers.read() else {
            // A poisoned lock cannot be reported through this signature. An
            // empty list is the fail-closed answer: it pairs nobody and trusts
            // nobody. The mutating methods do report it.
            tracing::error!("paired-devices store lock is poisoned");
            return Vec::new();
        };
        let mut peers: Vec<Peer> = guard.values().cloned().collect();
        peers.sort_by(|a, b| a.pairing_id.cmp(&b.pairing_id));
        peers
    }

    /// One peer by pairing id.
    #[must_use]
    pub fn get(&self, pairing_id: &str) -> Option<Peer> {
        let guard = self.peers.read().ok()?;
        guard.get(pairing_id).cloned()
    }

    /// Insert a peer, or replace the one with the same pairing id.
    ///
    /// The file is rewritten atomically before this returns. If the write
    /// fails, the in-memory map is rolled back to what is actually on disk, so
    /// a caller can never be told a pairing was saved when it was not.
    ///
    /// # Errors
    ///
    /// [`PeerStoreError::Invalid`] if the record does not validate — nothing is
    /// written; [`PeerStoreError::Io`] if the file could not be replaced;
    /// [`PeerStoreError::Poisoned`] if another thread panicked holding the lock.
    pub fn upsert(&self, peer: Peer) -> Result<(), PeerStoreError> {
        peer.validate()?;
        let mut guard = self.peers.write().map_err(|_| PeerStoreError::Poisoned)?;
        let pairing_id = peer.pairing_id.clone();
        let previous = guard.insert(pairing_id.clone(), peer);
        match write_atomically(&self.path, &guard) {
            Ok(()) => {
                tracing::debug!(%pairing_id, "paired device saved");
                Ok(())
            }
            Err(err) => {
                match previous {
                    Some(old) => guard.insert(pairing_id, old),
                    None => guard.remove(&pairing_id),
                };
                Err(err)
            }
        }
    }

    /// Forget a peer. Returns whether one was there to forget.
    ///
    /// # Errors
    ///
    /// [`PeerStoreError::Io`] if the file could not be replaced — the peer stays
    /// in memory in that case, matching what is still on disk;
    /// [`PeerStoreError::Poisoned`] if another thread panicked holding the lock.
    pub fn remove(&self, pairing_id: &str) -> Result<bool, PeerStoreError> {
        let mut guard = self.peers.write().map_err(|_| PeerStoreError::Poisoned)?;
        let Some(removed) = guard.remove(pairing_id) else {
            return Ok(false);
        };
        match write_atomically(&self.path, &guard) {
            Ok(()) => {
                tracing::debug!(%pairing_id, "paired device removed");
                Ok(true)
            }
            Err(err) => {
                guard.insert(pairing_id.to_string(), removed);
                Err(err)
            }
        }
    }

    /// Every known pairing id and PSK, for a responder to try against an
    /// inbound handshake ([`crate::Session::accept_any`]).
    ///
    /// Ordered by pairing id, so the trial order does not depend on hash-map
    /// iteration and is therefore reproducible.
    ///
    /// These are copies of live key material and are **not** zeroized for you.
    /// Bind the result to a `zeroize::Zeroizing` if it outlives the handshake.
    #[must_use]
    pub fn psks(&self) -> Vec<(String, [u8; TOKEN_LEN])> {
        self.list()
            .iter()
            .map(|peer| (peer.pairing_id.clone(), peer.psk))
            .collect()
    }

    /// How many peers are known. Cheaper than [`PeerStore::list`] for a status
    /// line.
    #[must_use]
    pub fn len(&self) -> usize {
        self.peers.read().map(|g| g.len()).unwrap_or(0)
    }

    /// Whether this device is paired with anything.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Peer count only. The path is not printed — `CLAUDE.md` rule 4 keeps paths
/// away from users, and a `Debug` line ends up in the same log as everything
/// else.
impl fmt::Debug for PeerStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PeerStore")
            .field("peers", &self.len())
            .finish_non_exhaustive()
    }
}

/// Parse the envelope, separating "another version wrote this" from "this is
/// damaged".
fn parse(bytes: &[u8]) -> Result<HashMap<String, Peer>, PeerStoreError> {
    // Two passes rather than one: the first tells us whether the bytes are JSON
    // at all and whether the format tag is ours, which is what makes `Legacy`
    // and `Corrupt` distinguishable.
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| PeerStoreError::Corrupt)?;
    let tagged = value
        .get("format")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|tag| tag == FORMAT_TAG);
    if !tagged {
        return Err(PeerStoreError::Legacy);
    }

    let file: StoreFile = serde_json::from_value(value).map_err(|_| PeerStoreError::Corrupt)?;
    let mut peers = HashMap::with_capacity(file.peers.len());
    for peer in file.peers {
        peer.validate().map_err(|_| PeerStoreError::Corrupt)?;
        peers.insert(peer.pairing_id.clone(), peer);
    }
    Ok(peers)
}

/// Replace the store file in one step.
///
/// Temporary file in the same directory (a `rename` across filesystems is not
/// atomic and would fail here), `0600` before any bytes go in, `fsync` the file
/// so the contents are durable before the rename publishes them, then `fsync`
/// the directory so the rename itself survives a power loss.
fn write_atomically(path: &Path, peers: &HashMap<String, Peer>) -> Result<(), PeerStoreError> {
    let mut records: Vec<Peer> = peers.values().cloned().collect();
    records.sort_by(|a, b| a.pairing_id.cmp(&b.pairing_id));
    let file = StoreFile {
        format: FORMAT_TAG.to_string(),
        peers: records,
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
fn warn_if_permissive(path: &Path) {
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
fn warn_if_permissive(_path: &Path) {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::PairingToken;

    fn peer(name: &str) -> Peer {
        let token = PairingToken::generate();
        Peer {
            pairing_id: token.pairing_id(),
            name: name.to_string(),
            psk: token.psk(),
            last_addr: Some("192.168.1.7:47654".parse().expect("addr")),
            last_seen_ms: 1_753_900_000_000,
        }
    }

    fn store_path(dir: &tempfile::TempDir) -> PathBuf {
        dir.path().join(DEFAULT_FILE_NAME)
    }

    #[test]
    fn default_file_name_is_not_v1s() {
        // `CLAUDE.md` rule 3: a v1 install's file must never be opened, or
        // appear to be opened, by v2.
        assert_eq!(DEFAULT_FILE_NAME, "peers-v2.json");
        assert_ne!(DEFAULT_FILE_NAME, "peers.json");
    }

    #[test]
    fn opening_a_missing_file_yields_an_empty_store_and_writes_nothing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);
        let store = PeerStore::open(&path).expect("open");
        assert!(store.is_empty());
        assert!(store.list().is_empty());
        assert!(store.psks().is_empty());
        assert!(!path.exists(), "opening must not create the file");
    }

    #[test]
    fn round_trips_through_the_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);

        let laptop = peer("Laptop");
        let phone = peer("Phone");
        let (laptop_id, laptop_psk) = (laptop.pairing_id.clone(), laptop.psk);
        let phone_id = phone.pairing_id.clone();

        let store = PeerStore::open(&path).expect("open");
        store.upsert(laptop).expect("upsert laptop");
        store.upsert(phone).expect("upsert phone");
        assert_eq!(store.len(), 2);

        // Reopened from disk, everything survives byte for byte.
        let reopened = PeerStore::open(&path).expect("reopen");
        assert_eq!(reopened.len(), 2);
        let got = reopened.get(&laptop_id).expect("laptop present");
        assert_eq!(got.name, "Laptop");
        assert!(got.psk_matches(&laptop_psk));
        assert_eq!(
            got.last_addr,
            Some("192.168.1.7:47654".parse().expect("addr"))
        );
        assert_eq!(got.last_seen_ms, 1_753_900_000_000);
        assert!(reopened.get("nobody").is_none());

        // `list` and `psks` are ordered and agree with each other.
        let listed: Vec<String> = reopened
            .list()
            .iter()
            .map(|p| p.pairing_id.clone())
            .collect();
        let mut expected = vec![laptop_id.clone(), phone_id.clone()];
        expected.sort();
        assert_eq!(listed, expected);
        let psks = reopened.psks();
        assert_eq!(psks.len(), 2);
        assert_eq!(
            psks.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>(),
            expected
        );
        let stored = psks
            .iter()
            .find(|(id, _)| *id == laptop_id)
            .expect("laptop psk");
        assert_eq!(stored.1, laptop_psk);
    }

    #[test]
    fn upsert_replaces_by_pairing_id_and_overwrites_atomically() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);
        let store = PeerStore::open(&path).expect("open");

        let original = peer("Old name");
        let id = original.pairing_id.clone();
        let psk = original.psk;
        store.upsert(original).expect("first write");
        let first_len = std::fs::metadata(&path).expect("meta").len();

        let renamed = Peer {
            pairing_id: id.clone(),
            name: "New name".to_string(),
            psk,
            last_addr: None,
            last_seen_ms: 1_753_900_999_999,
        };
        store.upsert(renamed).expect("second write");

        // One record, not two, and the file was replaced rather than appended.
        assert_eq!(store.len(), 1);
        let reopened = PeerStore::open(&path).expect("reopen");
        assert_eq!(reopened.len(), 1);
        let got = reopened.get(&id).expect("present");
        assert_eq!(got.name, "New name");
        assert_eq!(got.last_addr, None);
        assert_eq!(got.last_seen_ms, 1_753_900_999_999);
        let second_len = std::fs::metadata(&path).expect("meta").len();
        assert!(second_len < first_len + 16, "file grew as if appended");

        // No temporary file was left behind.
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .expect("read dir")
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n != DEFAULT_FILE_NAME)
            .collect();
        assert!(
            leftovers.is_empty(),
            "temp files left behind: {leftovers:?}"
        );
    }

    #[test]
    fn remove_reports_whether_anything_went() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);
        let store = PeerStore::open(&path).expect("open");
        let p = peer("Tablet");
        let id = p.pairing_id.clone();
        store.upsert(p).expect("upsert");

        assert!(store.remove(&id).expect("remove"));
        assert!(!store.remove(&id).expect("second remove"), "already gone");
        assert!(store.is_empty());
        assert!(PeerStore::open(&path).expect("reopen").is_empty());
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
            assert_eq!(reopened.len(), i + 1);
        }
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
    fn a_file_from_another_version_is_reported_as_such() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);
        // Shaped like v1's peers.json: valid JSON, no v2 format tag.
        std::fs::write(
            &path,
            br#"{"peers":[{"device_id":"abc","password_file_b64":"AQID"}]}"#,
        )
        .expect("write");
        assert!(matches!(
            PeerStore::open(&path),
            Err(PeerStoreError::Legacy)
        ));
        // The file is still there, untouched — a downgrade must find its data.
        assert!(path.exists());

        // A bare array (another plausible older shape) is also Legacy, not
        // Corrupt.
        std::fs::write(&path, b"[]").expect("write");
        assert!(matches!(
            PeerStore::open(&path),
            Err(PeerStoreError::Legacy)
        ));
    }

    #[test]
    fn a_damaged_file_is_reported_and_never_discarded() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);

        for bad in [
            &b"not json at all"[..],
            &b""[..],
            // Right tag, wrong shape.
            br#"{"format":"copypaste-peers-v2","peers":"nope"}"#,
            // Right tag, a peer whose psk is not 32 bytes.
            br#"{"format":"copypaste-peers-v2","peers":[{"pairing_id":"a","name":"n","psk":"ab","last_addr":null,"last_seen_ms":0}]}"#,
            // Right tag, a peer whose psk is not hex.
            br#"{"format":"copypaste-peers-v2","peers":[{"pairing_id":"a","name":"n","psk":"zzzz","last_addr":null,"last_seen_ms":0}]}"#,
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
    fn an_all_zero_psk_is_refused() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = PeerStore::open(&store_path(&dir)).expect("open");
        let bad = Peer {
            pairing_id: "id".to_string(),
            name: "Uninitialised".to_string(),
            psk: [0u8; TOKEN_LEN],
            last_addr: None,
            last_seen_ms: 0,
        };
        assert!(matches!(store.upsert(bad), Err(PeerStoreError::Invalid(_))));
        assert!(store.is_empty());

        // Note: `Peer` has a `Drop`, so `..peer("x")` (functional record update)
        // does not compile — every field is written out. That friction is the
        // point: a PSK should never be moved out of a record by accident.
        let empty_id = Peer {
            pairing_id: String::new(),
            name: "Nameless".to_string(),
            psk: PairingToken::generate().psk(),
            last_addr: None,
            last_seen_ms: 0,
        };
        assert!(matches!(
            store.upsert(empty_id),
            Err(PeerStoreError::Invalid(_))
        ));
    }

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
    fn the_serialised_file_holds_the_psk_as_hex_and_nothing_else_secret() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);
        let store = PeerStore::open(&path).expect("open");
        let p = peer("Laptop");
        let psk = p.psk;
        let pairing_id = p.pairing_id.clone();
        store.upsert(p).expect("upsert");

        let text = std::fs::read_to_string(&path).expect("read");
        assert!(text.contains(FORMAT_TAG));
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
    fn concurrent_upserts_all_land() {
        use std::sync::Arc;

        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);
        let store = Arc::new(PeerStore::open(&path).expect("open"));

        let mut handles = Vec::new();
        for i in 0..8 {
            let store = Arc::clone(&store);
            handles.push(std::thread::spawn(move || {
                let mut ids = Vec::new();
                for j in 0..4 {
                    let p = peer(&format!("device-{i}-{j}"));
                    ids.push(p.pairing_id.clone());
                    store.upsert(p).expect("concurrent upsert");
                }
                ids
            }));
        }
        let expected: Vec<String> = handles
            .into_iter()
            .flat_map(|h| h.join().expect("thread"))
            .collect();

        assert_eq!(store.len(), expected.len());
        // Every write is serialised through the lock, so the final file has all
        // of them — no thread's rewrite dropped another's record.
        let reopened = PeerStore::open(&path).expect("reopen");
        assert_eq!(reopened.len(), expected.len());
        for id in expected {
            assert!(reopened.get(&id).is_some(), "lost record {id}");
        }
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

    #[test]
    fn psk_comparison_is_available_and_correct() {
        let p = peer("Laptop");
        assert!(p.psk_matches(&p.psk));
        let mut other = p.psk;
        other[0] ^= 1;
        assert!(!p.psk_matches(&other));
        assert!(!p.psk_matches(&[0u8; TOKEN_LEN]));
    }

    #[test]
    fn error_messages_contain_no_paths() {
        let errors = [
            PeerStoreError::Io(std::io::Error::other("/home/someone/peers-v2.json")),
            PeerStoreError::Corrupt,
            PeerStoreError::Legacy,
            PeerStoreError::Invalid("pairing id is empty"),
            PeerStoreError::Poisoned,
        ];
        for err in &errors {
            let text = err.to_string();
            assert!(!text.contains('/'), "path-like error text: {text}");
            assert!(!text.contains('\\'), "path-like error text: {text}");
            assert!(!text.contains("home"), "path-like error text: {text}");
        }
    }
}

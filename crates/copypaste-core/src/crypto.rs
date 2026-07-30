//! Crypto: one device secret, one derivation path, one AEAD path.
//!
//! # What this module is
//!
//! A single 32-byte device secret lives in the OS keystore. Two keys are
//! derived from it with HKDF-SHA256, domain-separated by `info` string:
//!
//! ```text
//! device secret (32 B, OS keystore)
//!   └─ HKDF-SHA256(salt = HKDF_SALT, ikm = device secret)
//!        ├─ expand(info = b"copypaste/v2/sqlcipher-db-key")   -> db_key   (32 B)
//!        └─ expand(info = b"copypaste/v2/item-content-key")   -> item_key (32 B)
//! ```
//!
//! Item content is sealed with XChaCha20-Poly1305. The AAD binds the item's
//! logical id, so a ciphertext lifted out of one row and pasted into another
//! fails authentication instead of decrypting.
//!
//! # What this module deliberately is not
//!
//! v2 drops backward compatibility with v0.4.x (`CLAUDE.md` rule 3 and
//! `docs/rewrite/port-manifest/README.md`). The v1 manifest's HKDF strings, AAD
//! byte layouts, `CHUNK_FORMAT_V1` framing and `key_version` dispatch are
//! **reference only** and are not reproduced here. Concretely, this module has:
//!
//! * no `key_version` column, no version dispatch, no trial decryption;
//! * no rotation sweep and no repair sweep;
//! * no second hash function, no second salt, no second AAD schema.
//!
//! That is the simplification dropping compatibility bought. v1 carried two key
//! generations forever because it could not stop reading old rows; v2 can, and
//! every branch that existed only to choose between them is gone. If a future
//! version needs a second generation, add it then — a version field that only
//! ever holds one value is the beginning of the v1 problem, not a defence
//! against it.
//!
//! # Security properties carried over from v1 (port manifest 02, §2)
//!
//! * **Fail closed.** A wrong key, a wrong AAD, or a flipped bit anywhere in
//!   nonce/ciphertext/tag produces [`CryptoError::AuthFailed`]. There is no
//!   fallback read path and no way to ask *why* authentication failed — that
//!   distinction is an oracle (I-15).
//! * **AAD binds item identity.** `item_id` is the cross-device logical id, not
//!   a row primary key (I-6 / §3.3).
//! * **Fresh OS-CSPRNG nonce per message.** `OsRng`, never a counter, never
//!   reused (I-11).
//! * **Zeroize on drop** for every value holding key material (I-12).
//! * **Constant-time comparison** of secrets via `subtle` (I-13).
//! * **No panics on caller-supplied data.** Every failure is a typed `Result`
//!   (I-14).
//! * **Errors never contain a filesystem path** (`CLAUDE.md` rule 4). This is
//!   enforced structurally: [`CryptoError`]'s payloads are `&'static str`, so
//!   there is no way to interpolate a path into one even by accident.
//! * **Keystore naming is frozen** at `com.copypaste.daemon` /
//!   `device-secret-key` (I-10).
//!
//! # Dependencies
//!
//! Per `CLAUDE.md` rule 1, nothing here is hand-rolled: `chacha20poly1305` for
//! the AEAD, `hkdf` + `sha2` for derivation, `rand` for nonces, `zeroize` for
//! erasure, `subtle` for comparison, `directories` for the data dir. The only
//! code in this file is glue and policy.

// The macOS Keychain backend is behind the `macos-keychain` cargo feature,
// which `crates/copypaste-core/Cargo.toml` does not declare yet (it needs
// `security-framework = { workspace = true, optional = true }`). Until it does,
// `cfg(feature = "macos-keychain")` is an unknown feature name; allow the
// check-cfg lint so the rest of the crate compiles cleanly meanwhile.
#![allow(unexpected_cfgs)]

use std::sync::OnceLock;

use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    Key, XChaCha20Poly1305, XNonce,
};
use hkdf::Hkdf;
use rand::{rngs::OsRng, RngCore};
use sha2::Sha256;
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// XChaCha20-Poly1305 nonce length. 192 bits is the whole reason for choosing
/// XChaCha over ChaCha: random nonces have no practical birthday bound, so no
/// counter state has to be persisted anywhere (ADR-001).
pub const NONCE_LEN: usize = 24;

/// Poly1305 authentication tag length, appended to the ciphertext by the AEAD.
pub const TAG_LEN: usize = 16;

/// Length of the device secret and of every key derived from it.
pub const KEY_LEN: usize = 32;

/// HKDF extract salt. One salt for the whole v2 tree — domain separation
/// between the derived keys comes from the `info` strings below, which is what
/// HKDF's `info` parameter is for. Using distinct salts *as well* would be
/// belt-and-braces with no additional property.
const HKDF_SALT: &[u8] = b"copypaste/v2/device-secret/hkdf-salt";

/// HKDF `info` for the SQLCipher database key.
const INFO_DB_KEY: &[u8] = b"copypaste/v2/sqlcipher-db-key";

/// HKDF `info` for the per-item content AEAD key.
const INFO_ITEM_KEY: &[u8] = b"copypaste/v2/item-content-key";

/// Fixed prefix of the item AEAD's associated data.
const AAD_PREFIX: &[u8] = b"copypaste/v2/item-aead|";

/// macOS Keychain service name. Frozen (port manifest 02, I-10) — renaming it
/// makes an existing install's database unopenable.
///
/// Read only by the macOS backend below, so it is unused on other targets;
/// the name is still defined unconditionally because a test pins it and
/// because moving a frozen identifier behind a `cfg` is how it drifts.
#[allow(dead_code)]
const KEYCHAIN_SERVICE: &str = "com.copypaste.daemon";

/// macOS Keychain account name for the device secret. Frozen (I-10).
#[allow(dead_code)]
const KEYCHAIN_ACCOUNT: &str = "device-secret-key";

/// Filename of the development-only file-backed secret store.
const SECRET_FILE_NAME: &str = "device_secret.key";

/// Dev/test bypass. When set to any value, no keystore is touched at all and a
/// fresh random secret is minted for the process lifetime. Read exactly once
/// (see [`ephemeral_requested`]) so that mutating the environment of a running
/// process cannot flip an already-keyed daemon into ephemeral mode
/// (port manifest 02, I-23).
const ENV_EPHEMERAL: &str = "COPYPASTE_EPHEMERAL_KEY";

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Every failure this module can produce.
///
/// Payloads are `&'static str` on purpose. `CLAUDE.md` rule 4 forbids showing
/// users a filesystem path (the daemon's data directory discloses the local
/// username), and a `String` payload is an open invitation to `format!` one in.
/// A closed set of literals cannot leak.
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    /// Wrong key, wrong AAD (wrong `item_id`), or tampered
    /// nonce/ciphertext/tag.
    ///
    /// These are **deliberately indistinguishable**: telling a caller which one
    /// happened turns decryption into an oracle (port manifest 02, I-15). The
    /// only correct handling is skip-and-count; never treat it as success.
    #[error("authentication failed")]
    AuthFailed,

    /// The nonce handed to [`decrypt`] is not [`NONCE_LEN`] bytes.
    ///
    /// Structural, not secret-bearing: it says nothing about the key or the
    /// plaintext, so unlike [`CryptoError::AuthFailed`] it is safe to
    /// distinguish. It means the storage row is corrupt or a caller passed the
    /// wrong column.
    #[error("nonce must be {NONCE_LEN} bytes")]
    InvalidNonce,

    /// The OS keystore could not be read or written, and its contents are
    /// therefore *unknown*.
    ///
    /// This is never a licence to mint a fresh secret: doing so would open the
    /// existing SQLCipher database with the wrong key. Only an unambiguous
    /// "no entry exists" answer authorises creation, and that path returns
    /// `Ok` rather than this error (port manifest 02, I-20). Callers should
    /// degrade — report locked, leave the encrypted database untouched.
    #[error("key store unavailable: {0}")]
    KeystoreUnavailable(&'static str),

    /// A cryptographic primitive failed for a reason that is not attacker
    /// controlled — e.g. an HKDF output length the implementation rejects, or
    /// an AEAD input past the ~256 GiB per-message limit.
    #[error("internal crypto error: {0}")]
    Internal(&'static str),
}

// ---------------------------------------------------------------------------
// Keys
// ---------------------------------------------------------------------------

/// The device secret plus the keys derived from it.
///
/// Construct with [`Keyring::load_or_create`] in production or
/// [`Keyring::from_secret`] in tests. The secret is zeroized when the `Keyring`
/// is dropped.
pub struct Keyring {
    secret: Zeroizing<[u8; KEY_LEN]>,
}

impl Keyring {
    /// Load the device secret from the OS keystore, creating one on first run.
    ///
    /// * **macOS** — a Keychain generic-password item under service
    ///   `com.copypaste.daemon`, account `device-secret-key`. Requires the
    ///   `macos-keychain` cargo feature (see the note at the top of this file).
    /// * **Every other platform** — a `0600` file named `device_secret.key`
    ///   under the platform data directory.
    ///
    /// **The file backend is for development only.** It is not a keystore: the
    /// secret sits in the user's home directory in plaintext, protected by
    /// nothing but Unix file permissions, and is readable by any process
    /// running as that user and by anything that backs the directory up. It
    /// exists so the daemon can be built and tested on a Linux workstation.
    /// Android must use the Android Keystore before shipping — a
    /// platform-`cfg`'d backend added here, alongside the macOS one.
    ///
    /// Setting `COPYPASTE_EPHEMERAL_KEY` short-circuits both backends before
    /// any keystore call and mints a throwaway secret. Data written under it is
    /// unrecoverable after the process exits, which is the point.
    ///
    /// # Errors
    ///
    /// [`CryptoError::KeystoreUnavailable`] when the keystore's state could not
    /// be determined. That is *not* the same as "no secret exists yet": a first
    /// run creates a secret and returns `Ok`. See the variant's docs.
    pub fn load_or_create() -> Result<Self, CryptoError> {
        if ephemeral_requested() {
            tracing::warn!(
                "{ENV_EPHEMERAL} is set: using a throwaway device secret. \
                 Anything written now becomes unreadable when this process exits."
            );
            return Ok(Self::from_secret(&random_secret()));
        }
        let secret = backend::load_or_create_secret()?;
        Ok(Self { secret })
    }

    /// Deterministic construction from a known secret.
    ///
    /// For tests and for callers that obtained the secret some other way. Does
    /// not touch any keystore.
    pub fn from_secret(secret: &[u8; KEY_LEN]) -> Self {
        Self {
            secret: Zeroizing::new(*secret),
        }
    }

    /// Key for SQLCipher, as raw bytes.
    ///
    /// The caller formats the PRAGMA — this module does not build SQL. Note
    /// that SQLCipher wants `PRAGMA key = "x'<64 lowercase hex chars>'"`
    /// applied *before any other statement*, and that both the hex string and
    /// this array should be wrapped in `zeroize::Zeroizing` at the call site;
    /// the signature is fixed by the shared API contract, so it cannot do that
    /// for you.
    pub fn db_key(&self) -> [u8; KEY_LEN] {
        derive(&self.secret, INFO_DB_KEY)
    }

    /// Key for item content AEAD.
    pub fn item_key(&self) -> ItemKey {
        ItemKey(Zeroizing::new(derive(&self.secret, INFO_ITEM_KEY)))
    }
}

/// Redacted: a `Debug` that prints key bytes ends up in a log file.
impl std::fmt::Debug for Keyring {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Keyring").finish_non_exhaustive()
    }
}

/// The per-item content key. Zeroized on drop; never printed.
pub struct ItemKey(Zeroizing<[u8; KEY_LEN]>);

impl ItemKey {
    /// Constant-time equality.
    ///
    /// `==` on key bytes short-circuits on the first differing byte and leaks
    /// a prefix-match length by timing (port manifest 02, I-13). This is the
    /// only equality this type has.
    pub fn ct_eq(&self, other: &Self) -> bool {
        self.0.as_ref().ct_eq(other.0.as_ref()).into()
    }
}

/// Deliberately routed through [`ItemKey::ct_eq`] so no caller can reach a
/// short-circuiting comparison of key bytes.
impl PartialEq for ItemKey {
    fn eq(&self, other: &Self) -> bool {
        self.ct_eq(other)
    }
}

impl Eq for ItemKey {}

/// Redacted, same reason as [`Keyring`]'s.
impl std::fmt::Debug for ItemKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ItemKey").finish_non_exhaustive()
    }
}

/// HKDF-SHA256 over the device secret. One extract, one expand per `info`.
fn derive(secret: &[u8; KEY_LEN], info: &[u8]) -> [u8; KEY_LEN] {
    let hk = Hkdf::<Sha256>::new(Some(HKDF_SALT), secret);
    let mut okm = [0u8; KEY_LEN];
    // Infallible for any length <= 255 * 32 bytes, and KEY_LEN is 32. The
    // `expect` documents that; it is not reachable from caller input.
    hk.expand(info, &mut okm)
        .expect("HKDF expand of 32 bytes cannot fail");
    okm
}

/// 32 bytes from the OS CSPRNG.
fn random_secret() -> [u8; KEY_LEN] {
    let mut secret = [0u8; KEY_LEN];
    OsRng.fill_bytes(&mut secret);
    secret
}

/// Read the dev bypass exactly once per process.
fn ephemeral_requested() -> bool {
    static EPHEMERAL: OnceLock<bool> = OnceLock::new();
    *EPHEMERAL.get_or_init(|| std::env::var_os(ENV_EPHEMERAL).is_some())
}

// ---------------------------------------------------------------------------
// AEAD
// ---------------------------------------------------------------------------

/// Associated data for the item AEAD: `copypaste/v2/item-aead|<len>:<item_id>`.
///
/// `item_id` is the cross-device logical item id, not a storage row id. Binding
/// it means a ciphertext copied into a different row fails to authenticate; the
/// row-swap detection is the entire reason per-item AEAD exists on top of an
/// already-encrypted SQLCipher file.
///
/// The `<len>:` prefix is not decoration. Port manifest 02, §3.2.2 records bug
/// **CopyPaste-lkmy**: an info string that concatenated two caller-controlled
/// ids with `|` collided when an id contained `|`, deriving identical keys for
/// different inputs. Only one caller-controlled field appears here and it is
/// terminal, so the length prefix is not strictly required today — it is here
/// so that adding a second field later cannot silently reintroduce the bug.
fn item_aad(item_id: &str) -> Vec<u8> {
    let id = item_id.as_bytes();
    let mut aad = Vec::with_capacity(AAD_PREFIX.len() + 24 + id.len());
    aad.extend_from_slice(AAD_PREFIX);
    aad.extend_from_slice(id.len().to_string().as_bytes());
    aad.push(b':');
    aad.extend_from_slice(id);
    aad
}

/// Seal `plaintext` under `key`, binding `item_id` into the AAD.
///
/// Returns `(nonce, ciphertext)`. The nonce is [`NONCE_LEN`] bytes, freshly
/// drawn from the OS CSPRNG for every call. The ciphertext is
/// `body || poly1305_tag`, so it is [`TAG_LEN`] bytes longer than the
/// plaintext — an empty plaintext yields a 16-byte ciphertext, not an empty
/// one. Both are opaque to the caller; store them as written.
///
/// # Errors
///
/// [`CryptoError::Internal`] if the AEAD rejects the input, which for
/// XChaCha20-Poly1305 means a plaintext past `(2^32 - 1) * 64` bytes. Never
/// panics on caller input (port manifest 02, I-14).
pub fn encrypt(
    plaintext: &[u8],
    key: &ItemKey,
    item_id: &str,
) -> Result<(Vec<u8>, Vec<u8>), CryptoError> {
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key.0.as_ref()));

    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = XNonce::from_slice(&nonce_bytes);

    let aad = item_aad(item_id);
    let ciphertext = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad: &aad,
            },
        )
        .map_err(|_| CryptoError::Internal("AEAD rejected the plaintext (too large)"))?;

    Ok((nonce_bytes.to_vec(), ciphertext))
}

/// Open a ciphertext produced by [`encrypt`] with the same key and `item_id`.
///
/// # Errors
///
/// [`CryptoError::InvalidNonce`] if `nonce` is not [`NONCE_LEN`] bytes.
///
/// [`CryptoError::AuthFailed`] for everything else — wrong key, wrong
/// `item_id`, a modified nonce, a modified or truncated ciphertext, or a
/// modified tag. These are not distinguished from each other by design; see
/// the variant's documentation.
pub fn decrypt(
    ciphertext: &[u8],
    nonce: &[u8],
    key: &ItemKey,
    item_id: &str,
) -> Result<Vec<u8>, CryptoError> {
    if nonce.len() != NONCE_LEN {
        return Err(CryptoError::InvalidNonce);
    }
    // Shorter than a bare tag: cannot be a valid sealed message. The AEAD
    // returns an error here too; checking first keeps the branch explicit.
    if ciphertext.len() < TAG_LEN {
        return Err(CryptoError::AuthFailed);
    }

    let cipher = XChaCha20Poly1305::new(Key::from_slice(key.0.as_ref()));
    let aad = item_aad(item_id);

    cipher
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad: &aad,
            },
        )
        .map_err(|_| CryptoError::AuthFailed)
}

// ---------------------------------------------------------------------------
// Keystore backends
// ---------------------------------------------------------------------------

/// macOS Keychain backend.
///
/// Gated on the `macos-keychain` cargo feature because `security-framework` is
/// declared in `[workspace.dependencies]` but is not yet a dependency of this
/// crate, and this module may not edit `Cargo.toml`. Enabling it needs:
///
/// ```toml
/// [dependencies]
/// security-framework = { workspace = true, optional = true }
///
/// [features]
/// macos-keychain = ["dep:security-framework"]
/// ```
///
/// Until then macOS falls through to the file backend, which is a development
/// posture and not a shipping one.
#[cfg(all(target_os = "macos", feature = "macos-keychain"))]
mod backend {
    use super::{
        random_secret, CryptoError, Zeroizing, KEYCHAIN_ACCOUNT, KEYCHAIN_SERVICE, KEY_LEN,
    };

    /// `errSecItemNotFound`. The *only* status that authorises minting a fresh
    /// device secret (port manifest 02, I-20). v1 matched `Err(_)` here, minted
    /// an ephemeral key against an existing database, and produced
    /// `SQLITE_NOTADB` plus a dead daemon.
    const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;

    pub(super) fn load_or_create_secret() -> Result<Zeroizing<[u8; KEY_LEN]>, CryptoError> {
        match security_framework::passwords::get_generic_password(
            KEYCHAIN_SERVICE,
            KEYCHAIN_ACCOUNT,
        ) {
            Ok(bytes) => {
                // Zeroize the Keychain-returned buffer even on the error path
                // (I-12, audit MED #4).
                let bytes = Zeroizing::new(bytes);
                let secret: [u8; KEY_LEN] = bytes.as_slice().try_into().map_err(|_| {
                    CryptoError::KeystoreUnavailable("stored secret has wrong length")
                })?;
                Ok(Zeroizing::new(secret))
            }
            Err(e) if e.code() == ERR_SEC_ITEM_NOT_FOUND => {
                let secret = Zeroizing::new(random_secret());
                security_framework::passwords::set_generic_password(
                    KEYCHAIN_SERVICE,
                    KEYCHAIN_ACCOUNT,
                    secret.as_ref(),
                )
                .map_err(|_| CryptoError::KeystoreUnavailable("could not write to the keychain"))?;
                Ok(secret)
            }
            // Locked (-25308), auth failed (-25293), missing entitlement
            // (-34018), timeout, I/O: the entry's state is UNKNOWN. Report it
            // and leave the encrypted database alone.
            Err(_) => Err(CryptoError::KeystoreUnavailable(
                "the keychain is locked or access was denied",
            )),
        }
    }
}

/// Development-only file backend: a `0600` file under the platform data dir.
///
/// See [`super::Keyring::load_or_create`] for why this is not a keystore.
#[cfg(not(all(target_os = "macos", feature = "macos-keychain")))]
mod backend {
    use std::fs;
    use std::io::{ErrorKind, Read, Write};
    use std::path::PathBuf;

    use super::{random_secret, CryptoError, Zeroizing, KEY_LEN, SECRET_FILE_NAME};

    /// Resolve the secret's path. Never surfaced in an error (`CLAUDE.md`
    /// rule 4) — the data dir contains the local username.
    fn secret_path() -> Result<PathBuf, CryptoError> {
        let dirs = directories::ProjectDirs::from("com", "copypaste", "copypaste").ok_or(
            CryptoError::KeystoreUnavailable("no platform data directory is available"),
        )?;
        Ok(dirs.data_dir().join(SECRET_FILE_NAME))
    }

    pub(super) fn load_or_create_secret() -> Result<Zeroizing<[u8; KEY_LEN]>, CryptoError> {
        let path = secret_path()?;

        // Try to create it first, atomically. `create_new` fails if the file
        // already exists, so two daemons racing on first run cannot clobber
        // each other's secret — the loser reads the winner's. Note there is no
        // temp-file-and-rename dance: rename replaces, and replacing this file
        // is exactly the operation that orphans a database.
        match create_secret_file(&path) {
            Ok(secret) => return Ok(secret),
            Err(CreateOutcome::Exists) => {}
            Err(CreateOutcome::Failed(e)) => return Err(e),
        }

        read_secret_file(&path)
    }

    enum CreateOutcome {
        Exists,
        Failed(CryptoError),
    }

    fn create_secret_file(
        path: &std::path::Path,
    ) -> Result<Zeroizing<[u8; KEY_LEN]>, CreateOutcome> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|_| {
                CreateOutcome::Failed(CryptoError::KeystoreUnavailable(
                    "could not create the data directory",
                ))
            })?;
        }

        let mut opts = fs::OpenOptions::new();
        opts.write(true).create_new(true);
        // 0600 is set *at create time*, not chmod'd afterwards, so the secret
        // is never momentarily group- or world-readable.
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }

        let mut file = match opts.open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == ErrorKind::AlreadyExists => return Err(CreateOutcome::Exists),
            Err(_) => {
                return Err(CreateOutcome::Failed(CryptoError::KeystoreUnavailable(
                    "could not create the device secret file",
                )))
            }
        };

        #[cfg(not(unix))]
        tracing::warn!(
            "this platform has no file-permission enforcement here; the device \
             secret is stored without restricted permissions"
        );

        let secret = Zeroizing::new(random_secret());
        let write = file
            .write_all(secret.as_ref())
            .and_then(|()| file.sync_all());
        if write.is_err() {
            // A partial file would later read back as the wrong length and
            // fail closed rather than silently minting a new key, but leaving
            // it behind would wedge every subsequent start. Remove it.
            let _ = fs::remove_file(path);
            return Err(CreateOutcome::Failed(CryptoError::KeystoreUnavailable(
                "could not write the device secret",
            )));
        }

        tracing::warn!(
            "created a file-backed device secret. This is a development \
             fallback, not an OS keystore."
        );
        Ok(secret)
    }

    fn read_secret_file(path: &std::path::Path) -> Result<Zeroizing<[u8; KEY_LEN]>, CryptoError> {
        let mut file = fs::File::open(path)
            .map_err(|_| CryptoError::KeystoreUnavailable("could not open the device secret"))?;

        // Read one byte more than expected so a too-long file is rejected
        // rather than silently truncated to its first 32 bytes.
        let mut buf = Zeroizing::new(Vec::with_capacity(KEY_LEN + 1));
        file.read_to_end(&mut buf)
            .map_err(|_| CryptoError::KeystoreUnavailable("could not read the device secret"))?;

        let secret: [u8; KEY_LEN] = buf
            .as_slice()
            .try_into()
            .map_err(|_| CryptoError::KeystoreUnavailable("stored secret has wrong length"))?;

        // Re-assert 0600 if something loosened it. Refusing to start would be
        // the stricter choice, but it locks the user out of their own history
        // over a permission bit; tightening and continuing keeps the secret
        // protected without risking data loss.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = file.metadata() {
                let mode = meta.permissions().mode() & 0o777;
                if mode != 0o600 {
                    tracing::warn!(
                        found_mode = format!("{mode:o}"),
                        "device secret file had permissive modes; tightening to 0600"
                    );
                    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
                }
            }
        }

        Ok(Zeroizing::new(secret))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET_A: [u8; KEY_LEN] = [7u8; KEY_LEN];
    const SECRET_B: [u8; KEY_LEN] = [9u8; KEY_LEN];
    const ITEM: &str = "1f3c9a4e-0000-4000-8000-000000000001";

    fn key_a() -> ItemKey {
        Keyring::from_secret(&SECRET_A).item_key()
    }

    // --- round trip ------------------------------------------------------

    #[test]
    fn round_trip_recovers_the_plaintext() {
        let key = key_a();
        let msg = b"ssh-rsa AAAAB3NzaC1yc2E... not really a key";

        let (nonce, ct) = encrypt(msg, &key, ITEM).unwrap();
        assert_eq!(nonce.len(), NONCE_LEN);
        assert_eq!(ct.len(), msg.len() + TAG_LEN);
        assert_ne!(
            &ct[..msg.len()],
            &msg[..],
            "plaintext appears in ciphertext"
        );

        let out = decrypt(&ct, &nonce, &key, ITEM).unwrap();
        assert_eq!(out, msg);
    }

    #[test]
    fn round_trip_survives_a_rebuilt_key() {
        // A restarted daemon re-derives the key from the same device secret.
        let (nonce, ct) = encrypt(b"persisted", &key_a(), ITEM).unwrap();
        let reloaded = Keyring::from_secret(&SECRET_A).item_key();
        assert_eq!(decrypt(&ct, &nonce, &reloaded, ITEM).unwrap(), b"persisted");
    }

    // --- fail closed -----------------------------------------------------

    #[test]
    fn wrong_key_fails() {
        let (nonce, ct) = encrypt(b"secret", &key_a(), ITEM).unwrap();
        let other = Keyring::from_secret(&SECRET_B).item_key();

        assert!(matches!(
            decrypt(&ct, &nonce, &other, ITEM),
            Err(CryptoError::AuthFailed)
        ));
    }

    #[test]
    fn wrong_item_id_in_aad_fails() {
        // The row-swap case: same key, same bytes, different row.
        let key = key_a();
        let (nonce, ct) = encrypt(b"secret", &key, ITEM).unwrap();

        assert!(matches!(
            decrypt(&ct, &nonce, &key, "1f3c9a4e-0000-4000-8000-000000000002"),
            Err(CryptoError::AuthFailed)
        ));
    }

    #[test]
    fn item_id_prefix_is_not_accepted_for_the_full_id() {
        // Guards the AAD's length prefix: "item" must not authenticate a
        // ciphertext bound to "item-2", however the framing is concatenated.
        let key = key_a();
        let (nonce, ct) = encrypt(b"secret", &key, "item").unwrap();

        assert!(decrypt(&ct, &nonce, &key, "item-2").is_err());
        assert!(decrypt(&ct, &nonce, &key, "").is_err());
    }

    #[test]
    fn empty_item_id_is_bound_too() {
        let key = key_a();
        let (nonce, ct) = encrypt(b"secret", &key, "").unwrap();
        assert_eq!(decrypt(&ct, &nonce, &key, "").unwrap(), b"secret");
        assert!(decrypt(&ct, &nonce, &key, ITEM).is_err());
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let key = key_a();
        let (nonce, ct) = encrypt(b"the quick brown fox", &key, ITEM).unwrap();

        // Every byte matters: body and all 16 tag bytes.
        for i in 0..ct.len() {
            let mut bad = ct.clone();
            bad[i] ^= 0x01;
            assert!(
                matches!(
                    decrypt(&bad, &nonce, &key, ITEM),
                    Err(CryptoError::AuthFailed)
                ),
                "flipping a bit in ciphertext byte {i} was not detected"
            );
        }
    }

    #[test]
    fn tampered_nonce_fails() {
        let key = key_a();
        let (nonce, ct) = encrypt(b"the quick brown fox", &key, ITEM).unwrap();

        for i in 0..nonce.len() {
            let mut bad = nonce.clone();
            bad[i] ^= 0x80;
            assert!(matches!(
                decrypt(&ct, &bad, &key, ITEM),
                Err(CryptoError::AuthFailed)
            ));
        }
    }

    #[test]
    fn truncated_ciphertext_fails_without_panicking() {
        let key = key_a();
        let (nonce, ct) = encrypt(b"the quick brown fox", &key, ITEM).unwrap();

        for len in 0..ct.len() {
            assert!(matches!(
                decrypt(&ct[..len], &nonce, &key, ITEM),
                Err(CryptoError::AuthFailed)
            ));
        }
    }

    #[test]
    fn swapped_nonces_both_fail() {
        let key = key_a();
        let (n1, c1) = encrypt(b"first", &key, ITEM).unwrap();
        let (n2, c2) = encrypt(b"second", &key, ITEM).unwrap();

        assert!(decrypt(&c1, &n2, &key, ITEM).is_err());
        assert!(decrypt(&c2, &n1, &key, ITEM).is_err());
    }

    #[test]
    fn wrong_nonce_length_is_a_structural_error() {
        let key = key_a();
        let (nonce, ct) = encrypt(b"x", &key, ITEM).unwrap();

        assert!(matches!(
            decrypt(&ct, &nonce[..NONCE_LEN - 1], &key, ITEM),
            Err(CryptoError::InvalidNonce)
        ));
        assert!(matches!(
            decrypt(&ct, &[], &key, ITEM),
            Err(CryptoError::InvalidNonce)
        ));

        let mut long = nonce.clone();
        long.push(0);
        assert!(matches!(
            decrypt(&ct, &long, &key, ITEM),
            Err(CryptoError::InvalidNonce)
        ));
    }

    // --- shapes of plaintext ---------------------------------------------

    #[test]
    fn empty_plaintext_round_trips() {
        let key = key_a();
        let (nonce, ct) = encrypt(b"", &key, ITEM).unwrap();

        // An empty plaintext still produces a tag, so the ciphertext is not
        // empty and "no content" is not distinguishable by length alone.
        assert_eq!(ct.len(), TAG_LEN);
        assert_eq!(decrypt(&ct, &nonce, &key, ITEM).unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn large_plaintext_round_trips() {
        // 4 MiB: larger than any realistic clipboard text item and past every
        // internal buffer size in the AEAD.
        let big: Vec<u8> = (0..4 * 1024 * 1024).map(|i| (i % 251) as u8).collect();
        let key = key_a();

        let (nonce, ct) = encrypt(&big, &key, ITEM).unwrap();
        assert_eq!(ct.len(), big.len() + TAG_LEN);
        assert_eq!(decrypt(&ct, &nonce, &key, ITEM).unwrap(), big);
    }

    #[test]
    fn non_utf8_and_unicode_payloads_round_trip() {
        let key = key_a();
        for msg in [&[0xff, 0xfe, 0x00, 0x01][..], "🔐 ключ".as_bytes()] {
            let (nonce, ct) = encrypt(msg, &key, ITEM).unwrap();
            assert_eq!(decrypt(&ct, &nonce, &key, ITEM).unwrap(), msg);
        }
    }

    // --- key derivation ---------------------------------------------------

    #[test]
    fn key_derivation_is_deterministic_for_a_fixed_secret() {
        let a = Keyring::from_secret(&SECRET_A);
        let b = Keyring::from_secret(&SECRET_A);

        assert_eq!(a.db_key(), b.db_key());
        // Compared in constant time via ItemKey's PartialEq.
        assert!(a.item_key() == b.item_key());

        // And it is stable across separately built rings, which is what a
        // daemon restart does.
        let (nonce, ct) = encrypt(b"stable", &a.item_key(), ITEM).unwrap();
        assert_eq!(
            decrypt(&ct, &nonce, &b.item_key(), ITEM).unwrap(),
            b"stable"
        );
    }

    #[test]
    fn different_secrets_derive_different_keys() {
        let a = Keyring::from_secret(&SECRET_A);
        let b = Keyring::from_secret(&SECRET_B);

        assert_ne!(a.db_key(), b.db_key());
        assert!(a.item_key() != b.item_key());
    }

    #[test]
    fn db_key_and_item_key_are_domain_separated() {
        // Same IKM, same salt, different `info`. If these ever collide, a
        // SQLCipher header disclosure would also disclose the item key.
        let ring = Keyring::from_secret(&SECRET_A);
        let item = ring.item_key();
        assert_ne!(ring.db_key(), *item.0.as_ref());
    }

    #[test]
    fn derived_keys_are_not_the_secret_itself() {
        // v1's "the seed is the item key" quirk (port manifest 02, §3.2.3) was
        // the single most easily-broken thing in that design. v2 derives both
        // keys; neither is the stored secret.
        let ring = Keyring::from_secret(&SECRET_A);
        assert_ne!(ring.db_key(), SECRET_A);
        assert_ne!(*ring.item_key().0.as_ref(), SECRET_A);
    }

    #[test]
    fn hkdf_info_strings_are_the_documented_ones() {
        // Pins the derivation labels. Changing either of these silently
        // re-keys every install, so a change should be a visible, deliberate
        // edit of both the constant and this test.
        assert_eq!(INFO_DB_KEY, b"copypaste/v2/sqlcipher-db-key");
        assert_eq!(INFO_ITEM_KEY, b"copypaste/v2/item-content-key");
        assert_eq!(HKDF_SALT, b"copypaste/v2/device-secret/hkdf-salt");
        assert_ne!(INFO_DB_KEY, INFO_ITEM_KEY);
    }

    #[test]
    fn keychain_identifiers_are_frozen() {
        // Port manifest 02, I-10.
        assert_eq!(KEYCHAIN_SERVICE, "com.copypaste.daemon");
        assert_eq!(KEYCHAIN_ACCOUNT, "device-secret-key");
    }

    // --- nonces -----------------------------------------------------------

    #[test]
    fn two_encryptions_of_the_same_plaintext_use_different_nonces() {
        let key = key_a();
        let (n1, c1) = encrypt(b"identical", &key, ITEM).unwrap();
        let (n2, c2) = encrypt(b"identical", &key, ITEM).unwrap();

        assert_ne!(n1, n2, "nonce reuse");
        assert_ne!(c1, c2, "deterministic ciphertext leaks equality of items");
    }

    #[test]
    fn nonces_are_unique_and_never_all_zero() {
        use std::collections::HashSet;

        let key = key_a();
        let mut seen = HashSet::new();
        for _ in 0..2_000 {
            let (nonce, _) = encrypt(b"x", &key, ITEM).unwrap();
            assert_ne!(
                nonce,
                vec![0u8; NONCE_LEN],
                "all-zero nonce from the CSPRNG"
            );
            assert!(seen.insert(nonce), "nonce repeated");
        }
    }

    // --- AAD --------------------------------------------------------------

    #[test]
    fn item_aad_has_the_documented_layout() {
        assert_eq!(item_aad("item-abc"), b"copypaste/v2/item-aead|8:item-abc");
        assert_eq!(item_aad(""), b"copypaste/v2/item-aead|0:");
        // Length is in bytes, not chars.
        assert_eq!(item_aad("é"), b"copypaste/v2/item-aead|2:\xc3\xa9");
    }

    #[test]
    fn item_aad_is_injective_across_delimiter_abuse() {
        // CopyPaste-lkmy in miniature: ids containing the delimiter must not
        // collide with each other.
        let ids = ["a|b", "a", "b", "3:a|b", "|", "1:a", ""];
        let mut seen = std::collections::HashSet::new();
        for id in ids {
            assert!(seen.insert(item_aad(id)), "AAD collision for {id:?}");
        }
    }

    // --- hygiene ----------------------------------------------------------

    #[test]
    fn debug_output_never_contains_key_material() {
        let ring = Keyring::from_secret(&SECRET_A);
        let item = ring.item_key();

        for rendered in [format!("{ring:?}"), format!("{item:?}")] {
            assert!(
                !rendered.contains('7'),
                "secret byte value leaked: {rendered}"
            );
            for byte in ring.db_key() {
                assert!(!rendered.contains(&byte.to_string()));
            }
        }
        assert_eq!(format!("{ring:?}"), "Keyring { .. }");
        assert_eq!(format!("{item:?}"), "ItemKey { .. }");
    }

    #[test]
    fn error_messages_contain_no_paths() {
        // CLAUDE.md rule 4. The type makes this structural — payloads are
        // &'static str — but pin the rendered strings anyway.
        let errors = [
            CryptoError::AuthFailed,
            CryptoError::InvalidNonce,
            CryptoError::KeystoreUnavailable("the keychain is locked or access was denied"),
            CryptoError::Internal("AEAD rejected the plaintext (too large)"),
        ];
        for e in errors {
            let msg = e.to_string();
            assert!(!msg.contains('/'), "path-like separator in {msg:?}");
            assert!(!msg.contains('\\'), "path-like separator in {msg:?}");
            assert!(!msg.contains("home"), "path-like fragment in {msg:?}");
        }
    }

    #[test]
    fn ct_eq_agrees_with_byte_equality() {
        let a = Keyring::from_secret(&SECRET_A).item_key();
        let a2 = Keyring::from_secret(&SECRET_A).item_key();
        let b = Keyring::from_secret(&SECRET_B).item_key();

        assert!(a.ct_eq(&a2));
        assert!(!a.ct_eq(&b));

        // Also for a one-bit difference, where a short-circuiting compare
        // would be indistinguishable in result but not in timing.
        let mut near = SECRET_A;
        near[KEY_LEN - 1] ^= 0x01;
        assert!(!a.ct_eq(&Keyring::from_secret(&near).item_key()));
    }
}

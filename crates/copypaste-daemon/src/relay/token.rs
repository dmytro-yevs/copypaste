//! Relay auth-token cache: encrypt/decrypt at rest (XChaCha20-Poly1305), atomic
//! 0600 file write, load/store helpers.

use std::path::PathBuf;

use base64::Engine as _;
use copypaste_core::{decrypt_item_with_aad, encrypt_item_with_aad, NONCE_SIZE};

/// Filename of the cached relay auth token inside the app data dir.
pub(super) const RELAY_TOKEN_FILE: &str = "relay_token";

// ── Token cache (0600 file) ─────────────────────────────────────────────────

/// Static prefix of the relay token AAD.
///
/// The full AAD is `"{RELAY_TOKEN_AAD_PREFIX}|{device_id}"` — binding both the
/// purpose (relay token, version 1) AND the daemon's own stable device UUID so
/// a token file encrypted by device A cannot silently decrypt as a valid token
/// under device B's identity even when both share the same `local_key`.
///
/// **CopyPaste-qvtg.4:** do NOT bind to `derive_relay_inbox_id(sync_key)`
/// here. At startup the push/receive loops call `load_initial_token` with
/// `sync_key` possibly `None`; using the inbox_id would make the cached token
/// undecryptable at boot (bricks relay sync silently). The daemon's own device
/// UUID is stable, available before any sync passphrase is set, and unique per
/// device — exactly the right anchor.
pub(super) const RELAY_TOKEN_AAD_PREFIX: &str = "copypaste-relay-token-v1";

/// Path to the cached relay token file (sibling of the device-key files).
pub(super) fn token_path() -> Option<PathBuf> {
    crate::paths::try_app_support_dir()
        .ok()
        .map(|d| d.join(RELAY_TOKEN_FILE))
}

/// Encrypt `token` bytes under `local_key` with XChaCha20-Poly1305.
///
/// `device_id` is the daemon's own stable device UUID; it is bound into the
/// AEAD AAD as `"{RELAY_TOKEN_AAD_PREFIX}|{device_id}"` so a ciphertext
/// produced by device A cannot silently authenticate under device B's id.
///
/// Returns `base64(nonce[24] || ciphertext_with_tag)`.
///
/// # Errors
/// Propagates `EncryptError` from the underlying AEAD layer (e.g. if the
/// plaintext somehow exceeds the per-message size limit — unlikely for a
/// short token but handled explicitly rather than unwrapped).
pub(super) fn encrypt_relay_token(
    token: &str,
    local_key: &zeroize::Zeroizing<[u8; 32]>,
    device_id: &str,
) -> Result<String, copypaste_core::EncryptError> {
    let aad = format!("{RELAY_TOKEN_AAD_PREFIX}|{device_id}");
    let (nonce, ct) = encrypt_item_with_aad(token.as_bytes(), local_key, aad.as_bytes())?;
    // Concatenate nonce || ciphertext into a single blob for storage.
    let mut blob = Vec::with_capacity(NONCE_SIZE + ct.len());
    blob.extend_from_slice(&nonce);
    blob.extend_from_slice(&ct);
    Ok(base64::engine::general_purpose::STANDARD.encode(&blob))
}

/// Decrypt a relay token that was written by [`encrypt_relay_token`].
///
/// `device_id` must be the SAME daemon device UUID that was passed to
/// `encrypt_relay_token`; the AEAD tag covers `"copypaste-relay-token-v1|{device_id}"`.
/// A token encrypted for a different device ID (or with the old static AAD)
/// will fail authentication and return `None` — the caller re-registers.
///
/// Returns `Some(token)` on success, `None` if the blob is malformed or the
/// AEAD tag does not verify (caller should treat the file as absent).
pub(super) fn decrypt_relay_token(
    encoded: &str,
    local_key: &zeroize::Zeroizing<[u8; 32]>,
    device_id: &str,
) -> Option<String> {
    let blob = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .ok()?;
    if blob.len() < NONCE_SIZE + 1 {
        // Too short to be a valid nonce || ciphertext blob.
        return None;
    }
    let nonce: [u8; NONCE_SIZE] = blob[..NONCE_SIZE]
        .try_into()
        // SAFETY: we just checked blob.len() >= NONCE_SIZE; infallible.
        .expect("slice is exactly NONCE_SIZE bytes");
    let ct = &blob[NONCE_SIZE..];
    let aad = format!("{RELAY_TOKEN_AAD_PREFIX}|{device_id}");
    let plaintext = decrypt_item_with_aad(ct, &nonce, local_key, aad.as_bytes()).ok()?;
    String::from_utf8(plaintext).ok()
}

/// Load a previously-cached relay auth token, if any. Never errors hard — a
/// missing/unreadable token just means "re-register".
///
/// `device_id` is the daemon's own stable device UUID. The token file is bound
/// to this id via the AEAD AAD; a file written for a different device (or with
/// the old static AAD from before CopyPaste-qvtg.4) will fail authentication
/// and trigger re-registration — a one-time refetch, not a hard error.
///
/// **Security (CopyPaste-qvtg.2):** the token file MUST authenticate under
/// XChaCha20-Poly1305 (AEAD-at-rest). If decryption fails — legacy plaintext,
/// wrong key, wrong device_id, truncated/corrupt, or **a token planted by a
/// local attacker with write access to the data dir** — this returns `None`
/// (the daemon re-registers and writes a fresh encrypted token). It NEVER
/// returns the raw file bytes.
///
/// The earlier "best-effort migration" path returned undecryptable file contents
/// verbatim as the bearer token, with no deadline. That permanently degraded the
/// at-rest protection to advisory and enabled a write-then-use TOCTOU: an
/// attacker could plant a controlled token and the daemon would use it. The
/// migration period is now over; re-registration is cheap and the only cost of
/// rejecting a genuine legacy plaintext token.
pub(super) fn load_cached_token(
    local_key: &zeroize::Zeroizing<[u8; 32]>,
    device_id: &str,
) -> Option<String> {
    let path = token_path()?;
    let raw = std::fs::read_to_string(&path).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    // ONLY accept a token that authenticates under AEAD with the correct
    // device_id. Anything else (legacy plaintext, corrupt, wrong key, wrong
    // device, or attacker-planted) is rejected: warn and return None so the
    // caller re-registers and overwrites the file with a fresh encrypted token.
    match decrypt_relay_token(trimmed, local_key, device_id) {
        Some(token) => Some(token),
        None => {
            tracing::warn!(
                "relay-sync: cached relay token failed AEAD decryption (legacy plaintext, \
                 corrupt, wrong device, or tampered) — ignoring it and re-registering"
            );
            None
        }
    }
}

/// Persist the relay auth token encrypted to a `0600` file. Best-effort: a
/// failure is logged (without the token) and the token is still used in-memory
/// for this run.
///
/// `device_id` is bound into the AEAD AAD (see [`encrypt_relay_token`]).
pub(super) fn store_cached_token(
    token: &str,
    local_key: &zeroize::Zeroizing<[u8; 32]>,
    device_id: &str,
) {
    let Some(path) = token_path() else {
        tracing::warn!("relay-sync: cannot resolve data dir to cache token");
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let encoded = match encrypt_relay_token(token, local_key, device_id) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "relay-sync: failed to encrypt relay token (continuing in-memory)");
            return;
        }
    };
    if let Err(e) = write_token_0600(&path, &encoded) {
        tracing::warn!(error = %e, "relay-sync: failed to cache relay token (continuing in-memory)");
    }
}

/// Write `content` to `path` with `0600` perms via a temp-file + rename so a
/// reader never sees a partial or world-readable file.
///
/// CopyPaste-2yuo: the temp file is now opened with `OpenOptionsExt::mode(0o600)`
/// so the file is **never** world-readable — not even for the brief window between
/// `File::create` (which inherits the process umask, typically giving 0644) and a
/// subsequent `set_permissions(0o600)` call. The explicit mode argument passed to
/// `open(2)` is `0o600 & ~umask`; since `0600` has no group/other bits, any umask
/// leaves it at `0600`, eliminating the race window atomically.
pub(super) fn write_token_0600(path: &std::path::Path, content: &str) -> std::io::Result<()> {
    use std::io::Write as _;
    let dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let tmp = dir.join(format!(".{RELAY_TOKEN_FILE}.tmp"));
    // CopyPaste-2yuo fix: open with mode 0o600 on the first syscall so no
    // world-readable window exists between create and chmod. The `#[cfg(unix)]`
    // block uses OpenOptionsExt; on non-Unix (Windows) we fall back to the
    // simple `File::create` (Windows has no Unix mode bits).
    #[cfg(unix)]
    let mut f = {
        use std::os::unix::fs::OpenOptionsExt as _;
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)?
    };
    #[cfg(not(unix))]
    let mut f = std::fs::File::create(&tmp)?;
    f.write_all(content.as_bytes())?;
    f.sync_all()?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Token encryption tests ────────────────────────────────────────────────

    /// Round-trip: encrypt then decrypt recovers the original token.
    #[test]
    fn token_encrypt_decrypt_roundtrip() {
        let key = zeroize::Zeroizing::new([0xABu8; 32]);
        let token = "test-auth-token-abc123-deadbeef";
        let device_id = "device-roundtrip-uuid";
        let encoded = encrypt_relay_token(token, &key, device_id).expect("encrypt");
        let recovered =
            decrypt_relay_token(&encoded, &key, device_id).expect("decrypt returned None");
        assert_eq!(recovered, token);
    }

    /// Two encryptions of the same token produce DIFFERENT base64 blobs (nonce
    /// uniqueness via OsRng) so the file content changes on every re-store.
    #[test]
    fn token_encrypt_nonce_is_unique_across_writes() {
        let key = zeroize::Zeroizing::new([0xCDu8; 32]);
        let token = "same-token-every-time";
        let device_id = "device-nonce-uuid";
        let enc1 = encrypt_relay_token(token, &key, device_id).expect("enc1");
        let enc2 = encrypt_relay_token(token, &key, device_id).expect("enc2");
        // The blobs must differ (nonce changes, so the entire base64 string differs).
        assert_ne!(enc1, enc2, "each encryption must use a fresh random nonce");
    }

    /// Wrong key → decrypt returns None (AEAD auth tag failure, not a panic).
    #[test]
    fn token_decrypt_wrong_key_returns_none() {
        let key_a = zeroize::Zeroizing::new([0x11u8; 32]);
        let key_b = zeroize::Zeroizing::new([0x22u8; 32]);
        let device_id = "device-wrongkey-uuid";
        let encoded = encrypt_relay_token("secret-token", &key_a, device_id).expect("encrypt");
        let result = decrypt_relay_token(&encoded, &key_b, device_id);
        assert!(
            result.is_none(),
            "wrong key must yield None, not a recovered token"
        );
    }

    /// Tampered ciphertext → decrypt returns None (not a panic).
    #[test]
    fn token_decrypt_tampered_ciphertext_returns_none() {
        let key = zeroize::Zeroizing::new([0x33u8; 32]);
        let device_id = "device-tamper-uuid";
        let mut blob = base64::engine::general_purpose::STANDARD
            .decode(encrypt_relay_token("my-token", &key, device_id).expect("enc"))
            .expect("b64");
        // Flip a bit in the ciphertext portion (after the 24-byte nonce).
        if let Some(b) = blob.get_mut(NONCE_SIZE) {
            *b ^= 0xFF;
        }
        let tampered = base64::engine::general_purpose::STANDARD.encode(&blob);
        assert!(decrypt_relay_token(&tampered, &key, device_id).is_none());
    }

    /// CopyPaste-qvtg.4: a token encrypted for device_id "A" must fail to decrypt
    /// under device_id "B" with the same local_key. This is the primary regression
    /// guard for the device-id AAD binding.
    #[test]
    fn token_encrypted_for_device_a_fails_under_device_b() {
        let key = zeroize::Zeroizing::new([0x44u8; 32]);
        let device_id_a = "device-uuid-aaaa-1111-2222-333333333333";
        let device_id_b = "device-uuid-bbbb-4444-5555-666666666666";
        let token = "my-relay-auth-token-xyz";

        let encoded = encrypt_relay_token(token, &key, device_id_a).expect("encrypt for A");

        // Must succeed under device A.
        let recovered =
            decrypt_relay_token(&encoded, &key, device_id_a).expect("same device must decrypt");
        assert_eq!(recovered, token, "device A can recover its own token");

        // Must fail under device B even though the key is identical.
        let result_b = decrypt_relay_token(&encoded, &key, device_id_b);
        assert!(
            result_b.is_none(),
            "token encrypted for device A must NOT decrypt under device B's id \
             (AEAD tag covers the device_id in the AAD)"
        );
    }

    /// CopyPaste-qvtg.2: a token file that does NOT authenticate under AEAD
    /// (legacy plaintext, corrupt, or attacker-planted) must be REJECTED —
    /// `load_cached_token` returns `None` and never the raw file bytes — while a
    /// properly AEAD-encrypted token is still accepted. This closes the
    /// write-then-use TOCTOU where a local attacker plants a controlled token.
    #[test]
    fn load_cached_token_rejects_non_aead_token() {
        let _lock = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let dir = tempfile::tempdir().expect("tmpdir");
        // try_app_support_dir() resolves under HOME on macOS / XDG_DATA_HOME on
        // Linux; set both so token_path() lands inside the tempdir.
        let prev_home = std::env::var_os("HOME");
        let prev_xdg = std::env::var_os("XDG_DATA_HOME");
        unsafe {
            std::env::set_var("HOME", dir.path());
            std::env::set_var("XDG_DATA_HOME", dir.path());
        }

        let key = zeroize::Zeroizing::new([0x55u8; 32]);
        let device_id = "device-load-test-uuid";
        let token_file = token_path().expect("token path resolves");
        if let Some(parent) = token_file.parent() {
            std::fs::create_dir_all(parent).expect("mkdir");
        }

        // 1) Legacy plaintext / attacker-planted token → rejected, never returned.
        std::fs::write(&token_file, b"attacker-planted-token-xyz\n").expect("write");
        assert!(
            load_cached_token(&key, device_id).is_none(),
            "non-AEAD token must be rejected (None), never returned as the bearer"
        );

        // 2) A properly AEAD-encrypted token → accepted and round-trips.
        let enc =
            encrypt_relay_token("real-encrypted-token-123", &key, device_id).expect("encrypt");
        write_token_0600(&token_file, &enc).expect("write encrypted");
        assert_eq!(
            load_cached_token(&key, device_id).as_deref(),
            Some("real-encrypted-token-123"),
            "a valid AEAD token must still load"
        );

        // 3) Token encrypted for a DIFFERENT device_id → rejected (qvtg.4 binding).
        let enc_other =
            encrypt_relay_token("other-device-token", &key, "other-device-uuid").expect("encrypt");
        write_token_0600(&token_file, &enc_other).expect("write other");
        assert!(
            load_cached_token(&key, device_id).is_none(),
            "token written for another device_id must be rejected for this device"
        );

        // Restore env.
        unsafe {
            match prev_home {
                Some(v) => std::env::set_var("HOME", v),
                None => std::env::remove_var("HOME"),
            }
            match prev_xdg {
                Some(v) => std::env::set_var("XDG_DATA_HOME", v),
                None => std::env::remove_var("XDG_DATA_HOME"),
            }
        }
    }

    /// Empty file → load returns None (no fallback to empty token).
    #[test]
    fn load_cached_token_empty_file_returns_none() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let token_file = dir.path().join(RELAY_TOKEN_FILE);
        std::fs::write(&token_file, b"   \n").expect("write");

        let key = zeroize::Zeroizing::new([0x77u8; 32]);
        let device_id = "device-empty-test-uuid";
        let raw = std::fs::read_to_string(&token_file).expect("read");
        let trimmed = raw.trim();
        // Empty / whitespace-only file → treated as absent.
        assert!(trimmed.is_empty());
        // Simulates the `if trimmed.is_empty() { return None; }` guard.
        assert!(if trimmed.is_empty() {
            None::<String>
        } else {
            decrypt_relay_token(trimmed, &key, device_id)
        }
        .is_none());
    }

    // ── BUG 1 (CopyPaste-2yuo): write_token_0600 permissions ─────────────────

    /// write_token_0600 must produce a file with exactly mode 0600.
    ///
    /// This is the contract test: the file must be 0600 regardless of the
    /// process umask. The old `File::create()` + `set_permissions()` approach
    /// created the temp file with the umask-modified mode (typically 0644) for a
    /// brief window before chmod. The fix uses `OpenOptionsExt::mode(0o600)` so
    /// the file is 0600 from the first open(2) call.
    ///
    /// Note: a race-condition reproducer cannot be written as a pure unit test
    /// without threading primitives; this test verifies the postcondition contract.
    #[cfg(unix)]
    #[test]
    fn write_token_0600_perms_are_exactly_0600() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let path = dir.path().join("relay_token_perms_test");
        write_token_0600(&path, "test-token-for-perms-check").expect("write ok");
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&path).expect("stat").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "token file must be mode 0600, got {:o}", mode);
    }

    /// write_token_0600 must produce a 0600 file even when the process umask is
    /// 0000 (which makes File::create produce world-readable 0666 files).
    ///
    /// This is the failing test for the race: with the old implementation
    /// `File::create` creates the temp file at mode 0666 (umask=0) for a brief
    /// window. The test cannot observe that window directly, but it documents
    /// the invariant that `mode(0o600)` via OpenOptionsExt is immune to umask.
    ///
    /// The umask is process-wide; this test uses `#[serial]` to avoid
    /// interference with other tests.
    #[cfg(unix)]
    #[test]
    #[serial_test::serial]
    fn write_token_0600_immune_to_permissive_umask() {
        // Temporarily set umask to 0 so File::create would produce 0666.
        // A correct implementation using OpenOptions::mode(0o600) must still
        // produce 0600 because the explicit mode overrides umask for the
        // bits we care about (0600 ∩ 0777 = 0600, unaffected by umask~0777).
        let old_umask = unsafe { libc::umask(0) };
        let dir = tempfile::tempdir().expect("tmpdir");
        let path = dir.path().join("relay_token_umask_test");
        let result = write_token_0600(&path, "tok-umask-test");
        // Restore umask before any assertion so a panic doesn't leave it broken.
        unsafe { libc::umask(old_umask) };
        result.expect("write ok");
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&path).expect("stat").permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "token file must be 0600 even with umask=0000 (world-open), got {:o}",
            mode
        );
    }
}

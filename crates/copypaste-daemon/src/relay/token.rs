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

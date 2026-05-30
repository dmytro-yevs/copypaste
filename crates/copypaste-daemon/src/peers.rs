//! Persistent storage for paired P2P devices.
//!
//! Paired device records are stored as a JSON file alongside the database.
//! The file is read at daemon startup and written whenever the pairing list
//! changes.

use std::io::Write as _;
use std::path::Path;

/// A device that has been paired with this instance.
///
/// `name` / `added_at` are `#[serde(default)]` so this type can also parse the
/// leaner records written by the IPC PAKE pairing handlers
/// (`{"fingerprint":…, "added_at":…}`, sometimes `"password_file_b64"`), which
/// omit a display name. Unknown fields (e.g. `password_file_b64`) are ignored.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct PairedDevice {
    /// SHA-256 fingerprint of the peer's TLS certificate, lowercase hex.
    pub fingerprint: String,
    /// Human-readable display name of the device.
    #[serde(default)]
    pub name: String,
    /// Unix timestamp (seconds) when this device was paired.
    #[serde(default)]
    pub added_at: i64,
    /// The peer's P2P sync-listener address (`host:port`), learned in-band
    /// during PAKE pairing. Used by the Phase 3 outbound connector to dial an
    /// already-paired peer directly (loopback mDNS filters 127.0.0.1 and is
    /// unreliable, so the connector relies on this persisted address rather than
    /// mDNS). `#[serde(default)]` keeps backward compatibility with older
    /// `peers.json` records that predate this field (they deserialise to `None`).
    #[serde(default)]
    pub address: Option<String>,
    /// Base64 (standard) of the 32-byte shared content sync key for this peer,
    /// derived deterministically from the PAKE `SessionKey` at pairing time
    /// (P2P Phase 3, cross-device readability).
    ///
    /// Both sides converge on the same `SessionKey` after a successful PAKE
    /// handshake, so each derives — and persists — the IDENTICAL key here. The
    /// sync orchestrator uses it to re-encrypt outgoing item plaintext (so a
    /// paired peer can decrypt it) and to decrypt incoming items before
    /// re-encrypting them under this device's own local-storage key.
    ///
    /// `#[serde(default)]` keeps backward compatibility with records that
    /// predate this field. chmod 0600 on the file (see [`save_peers`]) keeps the
    /// key off world-readable storage; it never leaves this host as plaintext.
    #[serde(default)]
    pub sync_key_b64: Option<String>,
}

/// Load the list of paired devices from `path`.
///
/// Returns an empty `Vec` if the file does not exist or cannot be parsed,
/// so callers never need to treat a missing file as an error.
pub fn load_peers(path: &Path) -> Vec<PairedDevice> {
    match std::fs::read_to_string(path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_else(|e| {
            tracing::warn!("Failed to parse peers file {}: {e}", path.display());
            Vec::new()
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => {
            tracing::warn!("Could not read peers file {}: {e}", path.display());
            Vec::new()
        }
    }
}

/// Persist `peers` to `path` as pretty-printed JSON.
///
/// Creates parent directories if they do not already exist.
///
/// # Security
///
/// Uses an atomic write: the JSON is written to a temp file in the **same
/// directory** (so the final `rename` is guaranteed to be on the same
/// filesystem), the temp file is created with mode `0600` from the very first
/// byte, and only then renamed over the destination. This eliminates the
/// world-readable window that existed when using `std::fs::write` (creates at
/// the umask-derived mode, typically `0644`) followed by `set_permissions`.
/// The `sync_key_b64` field in `PairedDevice` is the shared P2P content key;
/// it must never be readable by other users even momentarily.
pub fn save_peers(path: &Path, peers: &[PairedDevice]) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("peers path has no parent directory: {}", path.display()))?;
    std::fs::create_dir_all(parent)?;

    let json = serde_json::to_string_pretty(peers)?;

    // Atomic 0600 write: create a uniquely-named temp file in the SAME
    // directory (same filesystem → rename is atomic), set mode 0600 before any
    // secret bytes are written, write + flush + sync, then rename over the
    // destination.  A crash between write and rename leaves an invisible temp
    // file that will be cleaned up on the next successful write.
    let tmp = parent.join(format!(
        ".peers.json.tmp.{}.{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));

    let write_result = (|| -> std::io::Result<()> {
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            // Create with 0600 from the outset — secret content is never
            // momentarily group/other-readable between create and chmod.
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp)?;
        // Defence-in-depth: re-assert 0600 in case a restrictive parent umask
        // or a non-honouring filesystem ignored the create mode above.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        f.write_all(json.as_bytes())?;
        f.flush()?;
        f.sync_all()?;
        Ok(())
    })();

    if let Err(e) = write_result {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }

    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_device(fp: &str, name: &str) -> PairedDevice {
        PairedDevice {
            fingerprint: fp.to_string(),
            name: name.to_string(),
            added_at: 1_700_000_000,
            address: Some("127.0.0.1:4242".to_string()),
            sync_key_b64: None,
        }
    }

    #[test]
    fn roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("peers.json");
        let devices = vec![make_device("aabbcc", "Alice"), make_device("112233", "Bob")];

        save_peers(&path, &devices).unwrap();
        let loaded = load_peers(&path);
        assert_eq!(loaded, devices);
    }

    #[test]
    fn missing_file_returns_empty() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");
        assert!(load_peers(&path).is_empty());
    }

    #[test]
    fn corrupt_file_returns_empty() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("peers.json");
        std::fs::write(&path, b"not json").unwrap();
        assert!(load_peers(&path).is_empty());
    }

    /// Fix-2 (atomic 0600 write): `save_peers` must create `peers.json` with
    /// mode 0600 so that the shared `sync_key_b64` is never world-readable.
    /// The atomic temp-rename pattern must also leave no orphaned `.tmp.*` file
    /// in the parent directory after a successful write.
    #[cfg(unix)]
    #[test]
    fn save_peers_creates_file_with_mode_0600_and_no_tmp_orphan() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let path = dir.path().join("peers.json");
        let devices = vec![make_device("aabbcc", "Alice")];

        save_peers(&path, &devices).unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(
            mode & 0o777,
            0o600,
            "peers.json must be owner-only (0600), got {:o}",
            mode & 0o777
        );

        // No orphaned temp file should remain after a successful write.
        let orphans: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with(".peers.json.tmp.")
            })
            .collect();
        assert!(
            orphans.is_empty(),
            "atomic write must not leave temp files behind: {:?}",
            orphans
        );
    }

    /// A `peers.json` written before the `address` field existed must still
    /// deserialise (the field defaults to `None`) — backward compatibility.
    #[test]
    fn legacy_record_without_address_loads_as_none() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("peers.json");
        std::fs::write(
            &path,
            br#"[{"fingerprint":"aabbcc","name":"Old","added_at":1700000000}]"#,
        )
        .unwrap();
        let loaded = load_peers(&path);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].fingerprint, "aabbcc");
        assert_eq!(loaded[0].address, None);
    }
}

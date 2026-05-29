//! Persistent storage for paired P2P devices.
//!
//! Paired device records are stored as a JSON file alongside the database.
//! The file is read at daemon startup and written whenever the pairing list
//! changes.

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
pub fn save_peers(path: &Path, peers: &[PairedDevice]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(peers)?;
    std::fs::write(path, json)?;
    // chmod 0600 — peer cert fingerprints + addresses are sensitive identifiers
    // and must never be world-readable. Mirrors the IPC peers-surface writer.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
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

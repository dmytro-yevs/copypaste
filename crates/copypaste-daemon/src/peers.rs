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

    /// Friendly hardware model of the peer (e.g. `"MacBook Air"`), learned
    /// in-band over the bootstrap channel during pairing. `#[serde(default)]`
    /// for backward compatibility with records that predate this field.
    #[serde(default)]
    pub model: Option<String>,
    /// Peer's OS name + version (e.g. `"macOS 15.5"`), learned in-band.
    #[serde(default)]
    pub os_version: Option<String>,
    /// Peer's app/daemon version string, learned in-band.
    #[serde(default)]
    pub app_version: Option<String>,
    /// Peer's best LAN-routable display IP, learned in-band. Preferred over
    /// parsing the `host:port` `address` field for UI display.
    #[serde(default)]
    pub local_ip: Option<String>,
    /// Peer's STUN-discovered public / global IP (e.g. `"203.0.113.42"`), learned
    /// in-band over the bootstrap channel during pairing (B1: full device info).
    /// Surfaced verbatim in the `list_peers` IPC response so the Devices UI can
    /// show the remote peer's global IP. `None` when the peer opted out of
    /// public-IP collection, STUN had not resolved, or the record predates this
    /// field. Informational only — never used for auth/trust. `#[serde(default)]`
    /// keeps backward compatibility with older `peers.json` records.
    #[serde(default)]
    pub public_ip: Option<String>,
    /// Unix timestamp (seconds) of the FIRST successful sync connection with
    /// this peer. Set once and never overwritten. `None` until the first sync.
    #[serde(default)]
    pub first_sync_at: Option<i64>,
    /// Unix timestamp (seconds) of the MOST RECENT successful sync connection
    /// with this peer. Updated on every established (throttled) connection.
    #[serde(default)]
    pub last_sync_at: Option<i64>,
}

// Use the canonical fingerprint normaliser from the IPC module — single
// implementation, zero drift risk. The local alias keeps call-site churn
// minimal; any future rename only touches this one line.
use crate::ipc::canonical_fingerprint as canonical_fp;

/// Update the persisted `name`, `address`, and `local_ip` fields for a paired
/// peer from a live mDNS snapshot.
///
/// Loads `peers.json`, finds the record whose fingerprint canonicalises to the
/// same value as `fingerprint`, and applies whichever of the three live fields
/// actually differ from what is stored, then atomically rewrites the file via
/// [`save_peers`].  All other fields (sync timestamps, `sync_key_b64`, `model`,
/// `os_version`, `app_version`, `public_ip`) are preserved verbatim — those
/// richer fields are NOT carried by mDNS and are out of scope for this helper.
///
/// Returns `true` when at least one field changed and the file was rewritten,
/// `false` when nothing changed (no I/O).  No-op (returns `Ok(false)`) when no
/// matching peer record exists.
///
/// # Follow-up
/// `model`, `os_version`, `app_version`, and `public_ip` are learned in-band
/// over the bootstrap channel at pairing time and are NOT carried by mDNS TXT
/// records.  Refreshing them reactively would require a separate wire-protocol
/// extension and is deferred.
/// // TODO(DeviceInfoAnnounce frame): once we add a DeviceInfoAnnounce wire frame,
/// // drive model/os_version/app_version/public_ip refresh through that path.
pub fn update_peer_meta(
    path: &Path,
    fingerprint: &str,
    new_name: &str,
    new_addr: std::net::SocketAddr,
    new_local_ip: &str,
) -> anyhow::Result<bool> {
    let target = canonical_fp(fingerprint);
    let mut peers = load_peers(path);
    let Some(peer) = peers
        .iter_mut()
        .find(|p| canonical_fp(&p.fingerprint) == target)
    else {
        // No matching record — nothing to update.  Not an error.
        return Ok(false);
    };

    let mut changed = false;

    if !new_name.is_empty() && peer.name != new_name {
        peer.name = new_name.to_string();
        changed = true;
    }

    let new_addr_str = new_addr.to_string();
    if peer.address.as_deref() != Some(new_addr_str.as_str()) {
        peer.address = Some(new_addr_str);
        changed = true;
    }

    if !new_local_ip.is_empty() && peer.local_ip.as_deref() != Some(new_local_ip) {
        peer.local_ip = Some(new_local_ip.to_string());
        changed = true;
    }

    if changed {
        save_peers(path, &peers)?;
    }
    Ok(changed)
}

/// Update the persisted `address` field for a paired peer.
///
/// Loads `peers.json`, finds the record whose fingerprint canonicalises to the
/// same value as `fingerprint` (colon-hex stored vs colon-free P2P both
/// match), updates only its `address` to `new_addr.to_string()`, then
/// atomically rewrites the file via [`save_peers`].  All other fields (name,
/// added_at, sync timestamps, etc.) are preserved verbatim.
///
/// No-op (and not an error) when no matching peer record exists.
pub fn update_peer_address(
    path: &Path,
    fingerprint: &str,
    new_addr: std::net::SocketAddr,
) -> anyhow::Result<()> {
    let target = canonical_fp(fingerprint);
    let mut peers = load_peers(path);
    let Some(peer) = peers
        .iter_mut()
        .find(|p| canonical_fp(&p.fingerprint) == target)
    else {
        // No matching record — nothing to update.  Not an error.
        return Ok(());
    };
    peer.address = Some(new_addr.to_string());
    save_peers(path, &peers)
}

/// Stamp first/last sync timestamps for the peer identified by `fingerprint`.
///
/// Loads `peers.json`, finds the record whose fingerprint canonicalises to the
/// same value as `fingerprint` (so a colon-hex stored record matches a
/// colon-free P2P fingerprint and vice versa), sets `first_sync_at` only if it
/// was previously `None`, and ALWAYS updates `last_sync_at` to `now_secs`, then
/// atomically rewrites the file via [`save_peers`].
///
/// No-op (and not an error) when no matching peer record exists — the peer may
/// not yet be persisted, or may have been unpaired between connect and stamp.
/// Callers should throttle invocations (per-connection or debounced ≥ 60 s) to
/// avoid write amplification; this function does not throttle internally.
pub fn touch_sync_times(path: &Path, fingerprint: &str, now_secs: i64) -> anyhow::Result<()> {
    let target = canonical_fp(fingerprint);
    let mut peers = load_peers(path);
    let Some(peer) = peers
        .iter_mut()
        .find(|p| canonical_fp(&p.fingerprint) == target)
    else {
        // No matching record — nothing to stamp. Not an error.
        return Ok(());
    };
    if peer.first_sync_at.is_none() {
        peer.first_sync_at = Some(now_secs);
    }
    peer.last_sync_at = Some(now_secs);
    save_peers(path, &peers)
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
            model: None,
            os_version: None,
            app_version: None,
            local_ip: None,
            public_ip: None,
            first_sync_at: None,
            last_sync_at: None,
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

    /// A `peers.json` written before the metadata / sync-time fields existed
    /// must still deserialise — all the new fields default to `None`.
    #[test]
    fn legacy_record_without_metadata_loads_as_none() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("peers.json");
        std::fs::write(
            &path,
            br#"[{"fingerprint":"aa:bb:cc","name":"Old","added_at":1700000000,"address":"127.0.0.1:4242"}]"#,
        )
        .unwrap();
        let loaded = load_peers(&path);
        assert_eq!(loaded.len(), 1);
        let p = &loaded[0];
        assert_eq!(p.model, None);
        assert_eq!(p.os_version, None);
        assert_eq!(p.app_version, None);
        assert_eq!(p.local_ip, None);
        assert_eq!(p.first_sync_at, None);
        assert_eq!(p.last_sync_at, None);
    }

    /// `touch_sync_times` sets `first_sync_at` only on the first call and
    /// always advances `last_sync_at`. Matching is canonical: the stored
    /// fingerprint is colon-hex but the lookup key is colon-free hex.
    #[test]
    fn touch_sync_times_sets_first_once_and_last_always() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("peers.json");
        // Stored in colon-hex form (the user-facing peers.json form).
        save_peers(&path, &[make_device("aa:bb:cc:dd", "Alice")]).unwrap();

        // First stamp via the colon-FREE canonical fingerprint (as the P2P
        // layer reports it) — must match the colon-hex stored record.
        touch_sync_times(&path, "aabbccdd", 1_000).unwrap();
        let after_first = load_peers(&path);
        assert_eq!(after_first[0].first_sync_at, Some(1_000));
        assert_eq!(after_first[0].last_sync_at, Some(1_000));

        // Second stamp: first_sync_at is preserved, last_sync_at advances.
        touch_sync_times(&path, "AA:BB:CC:DD", 2_000).unwrap();
        let after_second = load_peers(&path);
        assert_eq!(
            after_second[0].first_sync_at,
            Some(1_000),
            "first_sync_at must never be overwritten"
        );
        assert_eq!(
            after_second[0].last_sync_at,
            Some(2_000),
            "last_sync_at must always advance"
        );
    }

    /// `touch_sync_times` is a no-op (and not an error) when no matching peer
    /// record exists.
    #[test]
    fn touch_sync_times_no_match_is_noop() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("peers.json");
        save_peers(&path, &[make_device("aa:bb:cc:dd", "Alice")]).unwrap();
        touch_sync_times(&path, "deadbeef", 5_000).unwrap();
        let loaded = load_peers(&path);
        assert_eq!(loaded[0].first_sync_at, None);
        assert_eq!(loaded[0].last_sync_at, None);
    }

    /// `update_peer_meta` updates name, address, and local_ip when they change
    /// and returns `true`.  A second call with the SAME values returns `false`
    /// (no I/O) — idempotent.  Other fields (sync timestamps, sync_key_b64,
    /// model, os_version, app_version, public_ip) are preserved verbatim.
    #[test]
    fn update_peer_meta_updates_changed_fields_and_is_idempotent() {
        use std::net::SocketAddr;

        let dir = tempdir().unwrap();
        let path = dir.path().join("peers.json");
        // Start with a record that has a stale name and address.
        save_peers(&path, &[make_device("aa:bb:cc:dd", "Old Name")]).unwrap();
        // Manually set last_sync_at so we can verify it is preserved.
        touch_sync_times(&path, "aa:bb:cc:dd", 42_000).unwrap();

        let new_addr: SocketAddr = "192.168.1.5:9876".parse().unwrap();

        // First call — should change name, address, and local_ip → returns true.
        let changed = update_peer_meta(&path, "aabbccdd", "New Name", new_addr, "192.168.1.5")
            .expect("update_peer_meta must not error");
        assert!(changed, "expected true on first call with changed fields");

        let loaded = load_peers(&path);
        assert_eq!(loaded[0].name, "New Name");
        assert_eq!(loaded[0].address, Some("192.168.1.5:9876".to_string()));
        assert_eq!(loaded[0].local_ip, Some("192.168.1.5".to_string()));
        // last_sync_at preserved verbatim.
        assert_eq!(loaded[0].last_sync_at, Some(42_000));
        // model / os_version / app_version / public_ip remain None (out-of-scope).
        assert_eq!(loaded[0].model, None);
        assert_eq!(loaded[0].os_version, None);

        // Second call with identical values — nothing changed → returns false.
        let changed2 = update_peer_meta(&path, "aabbccdd", "New Name", new_addr, "192.168.1.5")
            .expect("update_peer_meta must not error");
        assert!(!changed2, "expected false on second call with same values");
    }

    /// `update_peer_meta` is a no-op (returns `Ok(false)`) when the fingerprint
    /// does not match any record.
    #[test]
    fn update_peer_meta_no_match_is_noop() {
        use std::net::SocketAddr;

        let dir = tempdir().unwrap();
        let path = dir.path().join("peers.json");
        save_peers(&path, &[make_device("aa:bb:cc:dd", "Alice")]).unwrap();

        let addr: SocketAddr = "10.0.0.1:1234".parse().unwrap();
        let changed = update_peer_meta(&path, "deadbeef", "Bob", addr, "10.0.0.1")
            .expect("update_peer_meta must not error on no-match");
        assert!(!changed, "no-match must return false");

        // Original record must be untouched.
        let loaded = load_peers(&path);
        assert_eq!(loaded[0].name, "Alice");
    }

    /// `update_peer_meta` preserves the name when `new_name` is empty (a peer
    /// that re-announces without a name should not blank our stored display name).
    #[test]
    fn update_peer_meta_empty_name_preserved() {
        use std::net::SocketAddr;

        let dir = tempdir().unwrap();
        let path = dir.path().join("peers.json");
        save_peers(&path, &[make_device("aabbcc", "Keeper")]).unwrap();

        let addr: SocketAddr = "10.0.0.2:5678".parse().unwrap();
        // Pass empty new_name — name must not be blanked.
        update_peer_meta(&path, "aabbcc", "", addr, "10.0.0.2").unwrap();

        let loaded = load_peers(&path);
        assert_eq!(
            loaded[0].name, "Keeper",
            "empty new_name must not overwrite stored name"
        );
        assert_eq!(loaded[0].address, Some("10.0.0.2:5678".to_string()));
    }
}

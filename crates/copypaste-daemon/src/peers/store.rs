//! Core `peers.json` load/save + the shared atomic-0600 JSON writer.
//!
//! Split out of the former flat `peers.rs` (ADR-017, CopyPaste-vp63.4) —
//! moved verbatim, no behavior change.

use std::io::Write as _;
use std::path::Path;

use super::model::PairedDevice;

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
    save_json_atomic_0600(path, peers)
}

/// Atomic 0600 write of any serializable value to `path`.
///
/// Extracted from [`save_peers`] so the `pending_unpair.json` store reuses the
/// identical write-temp-in-same-dir → chmod 0600 → fsync → rename sequence. See
/// [`save_peers`] for the full security rationale (the shared `sync_key_b64`
/// must never be world-readable, even momentarily).
///
/// `pub(super)` (CopyPaste-vp63.4): the one visibility widening in this split —
/// `pending_unpair.rs` (a sibling submodule) needs to call this same writer so
/// both stores share the identical atomic-write path.
pub(super) fn save_json_atomic_0600<T: serde::Serialize + ?Sized>(
    path: &Path,
    value: &T,
) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("path has no parent directory: {}", path.display()))?;
    std::fs::create_dir_all(parent)?;

    let json = serde_json::to_string_pretty(value)?;

    // Atomic 0600 write: create a uniquely-named temp file in the SAME
    // directory (same filesystem → rename is atomic), set mode 0600 before any
    // secret bytes are written, write + flush + sync, then rename over the
    // destination.  A crash between write and rename leaves an invisible temp
    // file that will be cleaned up on the next successful write.
    // Derive the temp-file prefix from the destination filename so each store
    // (peers.json / pending_unpair.json) cleans up only its OWN orphans and the
    // existing `.peers.json.tmp.` orphan-detection test stays valid.
    let base = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("store.json");
    let tmp = parent.join(format!(
        ".{base}.tmp.{}.{}",
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
            // Fresh test fixture, no prior device to carry a device_id from.
            device_id: None,
            public_ip: None,
            supabase_account_id: None,
            first_sync_at: None,
            last_sync_at: None,
            password_file_b64: None,
            password_file_enc: None,
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

    /// Characterization test (CopyPaste-vp63.4 gap): `save_json_atomic_0600`'s
    /// temp-file prefix is derived from `path.file_name()`, so two independent
    /// stores (`peers.json` and a hypothetical `pending_unpair.json` at the
    /// same directory) each clean up only their OWN orphaned temp files and
    /// never collide with each other's temp-file prefix.
    #[test]
    fn temp_file_prefix_is_derived_from_destination_file_name() {
        let dir = tempdir().unwrap();
        let peers_path = dir.path().join("peers.json");
        let other_path = dir.path().join("pending_unpair.json");

        save_json_atomic_0600(&peers_path, &vec![make_device("aabbcc", "Alice")]).unwrap();
        save_json_atomic_0600(&other_path, &Vec::<PairedDevice>::new()).unwrap();

        // Both files exist and no cross-contaminated temp file remains for
        // either destination filename's prefix.
        assert!(peers_path.exists());
        assert!(other_path.exists());
        let orphans: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp."))
            .collect();
        assert!(
            orphans.is_empty(),
            "no orphaned temp file for either destination: {orphans:?}"
        );
    }
}

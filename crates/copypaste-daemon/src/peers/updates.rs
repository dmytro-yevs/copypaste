//! Live-metadata updaters: mutate a single paired peer's record by canonical
//! fingerprint.
//!
//! Split out of the former flat `peers.rs` (ADR-017, CopyPaste-vp63.4) —
//! moved verbatim, no behavior change.

use std::path::Path;

use super::model::find_mut_by_fingerprint;
use super::store::{load_peers, save_peers};

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
    let mut peers = load_peers(path);
    let Some(peer) = find_mut_by_fingerprint(&mut peers, fingerprint) else {
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
    let mut peers = load_peers(path);
    let Some(peer) = find_mut_by_fingerprint(&mut peers, fingerprint) else {
        // No matching record — nothing to update.  Not an error.
        return Ok(());
    };
    peer.address = Some(new_addr.to_string());
    save_peers(path, &peers)
}

/// Refresh the volatile device metadata for a paired peer from the peer's
/// current in-band `ControlMsg::DeviceInfo` announcement.
///
/// Called on every successful authenticated session (both accept and connector
/// paths) when the peer sends a `DeviceInfo` control frame. Only `Some` arguments
/// overwrite the stored value — `None` means "no update for this field". This lets
/// a peer that did not collect a particular field (e.g. STUN not yet resolved)
/// avoid blanking an existing stored value.
///
/// Returns `true` when at least one field changed and the file was rewritten,
/// `false` when nothing changed (no I/O). No-op (returns `Ok(false)`) when no
/// matching peer record exists.
pub fn update_peer_device_info(
    path: &Path,
    fingerprint: &str,
    model: Option<&str>,
    os_version: Option<&str>,
    app_version: Option<&str>,
    public_ip: Option<&str>,
) -> anyhow::Result<bool> {
    let mut peers = load_peers(path);
    let Some(peer) = find_mut_by_fingerprint(&mut peers, fingerprint) else {
        return Ok(false);
    };

    let mut changed = false;

    // Only write a field when the caller supplied Some AND the stored value
    // differs — avoids a spurious write when the peer re-announces identical info.
    if let Some(m) = model {
        if peer.model.as_deref() != Some(m) {
            peer.model = Some(m.to_string());
            changed = true;
        }
    }
    if let Some(os) = os_version {
        if peer.os_version.as_deref() != Some(os) {
            peer.os_version = Some(os.to_string());
            changed = true;
        }
    }
    if let Some(av) = app_version {
        if peer.app_version.as_deref() != Some(av) {
            peer.app_version = Some(av.to_string());
            changed = true;
        }
    }
    if let Some(ip) = public_ip {
        if peer.public_ip.as_deref() != Some(ip) {
            peer.public_ip = Some(ip.to_string());
            changed = true;
        }
    }

    if changed {
        save_peers(path, &peers)?;
    }
    Ok(changed)
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
    let mut peers = load_peers(path);
    let Some(peer) = find_mut_by_fingerprint(&mut peers, fingerprint) else {
        // No matching record — nothing to stamp. Not an error.
        return Ok(());
    };
    if peer.first_sync_at.is_none() {
        peer.first_sync_at = Some(now_secs);
    }
    peer.last_sync_at = Some(now_secs);
    save_peers(path, &peers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peers::PairedDevice;
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

    // ─── CopyPaste-crh3.109: peer device-info refresh ───────────────────────

    /// `update_peer_device_info` with new values must update model/os/app/ip
    /// in the stored record and return `true`; a second call with the SAME
    /// values must be a no-op and return `false`.
    #[test]
    fn refresh_peer_device_info_updates_stored_metadata() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("peers.json");
        save_peers(&path, &[make_device("aa:bb:cc:dd", "Alice")]).unwrap();

        // Initial call — all fields change → returns true.
        let changed = update_peer_device_info(
            &path,
            "aa:bb:cc:dd",
            Some("MacBook Pro"),
            Some("macOS 15.6"),
            Some("1.2.3"),
            Some("203.0.113.42"),
        )
        .expect("update_peer_device_info must not error");
        assert!(changed, "first call with new metadata must return true");

        let loaded = load_peers(&path);
        assert_eq!(loaded[0].model.as_deref(), Some("MacBook Pro"));
        assert_eq!(loaded[0].os_version.as_deref(), Some("macOS 15.6"));
        assert_eq!(loaded[0].app_version.as_deref(), Some("1.2.3"));
        assert_eq!(loaded[0].public_ip.as_deref(), Some("203.0.113.42"));
        // Unrelated fields must be preserved verbatim.
        assert_eq!(loaded[0].name, "Alice");
        assert_eq!(loaded[0].address.as_deref(), Some("127.0.0.1:4242"));

        // Second call with identical values → no-op, returns false.
        let changed2 = update_peer_device_info(
            &path,
            "aa:bb:cc:dd",
            Some("MacBook Pro"),
            Some("macOS 15.6"),
            Some("1.2.3"),
            Some("203.0.113.42"),
        )
        .expect("update_peer_device_info must not error on repeat");
        assert!(
            !changed2,
            "second call with same metadata must return false"
        );
    }

    /// `update_peer_device_info` on an unknown fingerprint is a no-op (Ok(false)).
    #[test]
    fn refresh_peer_device_info_no_match_is_noop() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("peers.json");
        save_peers(&path, &[make_device("aa:bb:cc:dd", "Alice")]).unwrap();

        let changed =
            update_peer_device_info(&path, "deadbeef", Some("Mac mini"), None, None, None)
                .expect("update_peer_device_info must not error on no-match");
        assert!(
            !changed,
            "no-match must return false without modifying the file"
        );

        let loaded = load_peers(&path);
        assert_eq!(loaded[0].model, None, "unmatched peer must be unmodified");
    }

    /// `update_peer_device_info` with `None` values must not overwrite
    /// previously-stored non-None metadata (partial update — only `Some`
    /// arguments replace the stored value).
    #[test]
    fn refresh_peer_device_info_none_args_do_not_overwrite() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("peers.json");
        let mut device = make_device("aa:bb:cc:dd", "Alice");
        device.model = Some("Mac mini".to_string());
        device.os_version = Some("macOS 14.0".to_string());
        device.app_version = Some("0.9.0".to_string());
        device.public_ip = Some("1.2.3.4".to_string());
        save_peers(&path, &[device]).unwrap();

        // Pass None for model and os_version — they must be preserved.
        let changed = update_peer_device_info(
            &path,
            "aa:bb:cc:dd",
            None,          // do not overwrite model
            None,          // do not overwrite os_version
            Some("1.0.0"), // update app_version
            None,          // do not overwrite public_ip
        )
        .unwrap();
        assert!(changed, "app_version changed → must return true");

        let loaded = load_peers(&path);
        assert_eq!(
            loaded[0].model.as_deref(),
            Some("Mac mini"),
            "model must be preserved"
        );
        assert_eq!(
            loaded[0].os_version.as_deref(),
            Some("macOS 14.0"),
            "os_version must be preserved"
        );
        assert_eq!(
            loaded[0].app_version.as_deref(),
            Some("1.0.0"),
            "app_version must be updated"
        );
        assert_eq!(
            loaded[0].public_ip.as_deref(),
            Some("1.2.3.4"),
            "public_ip must be preserved"
        );
    }
}

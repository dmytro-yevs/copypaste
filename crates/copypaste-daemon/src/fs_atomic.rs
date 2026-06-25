/// Atomic 0600-mode file write helper shared across the daemon crate.
///
/// # Why this module exists
///
/// Two helpers existed independently:
/// - `ipc::config::atomic_write_0600` (bytes slice, tightens parent to 0700)
/// - `daemon::startup::write_text_atomic_0600` (str, no parent tighten)
///
/// Both implement the same durability sequence:
///   1. Create parent dirs.
///   2. Open a uniquely-named temp file in the SAME directory (same filesystem
///      → `rename` is POSIX-atomic) with mode `0600`.
///   3. Defence-in-depth: re-assert `0600` via `set_permissions` in case the
///      filesystem or umask ignored the `OpenOptions::mode` hint.
///   4. `write_all` + `flush` + `sync_all` (fsync the data).
///   5. `rename` over the destination — atomically visible to readers.
///
/// The consolidated helper preserves the STRONGER guarantee from
/// `atomic_write_0600`: it also tightens the parent directory to `0700`
/// (best-effort, error ignored) so secret files (device_id, config.json) are
/// not discoverable through a world-executable parent directory.
///
/// A crash between step 4 and step 5 leaves an invisible `.tmp.*` orphan in
/// the same directory. It will be overwritten or ignored by the next
/// successful write.
use std::path::Path;

/// Atomically write `bytes` to `path` with POSIX mode `0600`.
///
/// See the module-level documentation for the full durability and security
/// guarantees. On non-Unix platforms the `0600` / `0700` permission calls are
/// compiled out; the atomic rename guarantee still holds.
///
/// # Errors
///
/// Returns `Err` on any I/O failure. On error the temporary file is removed
/// (best-effort) and `path` is left unmodified.
pub(crate) fn write_atomic_0600(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    use std::io::Write as _;

    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("path has no parent directory: {}", path.display()))?;
    std::fs::create_dir_all(parent)?;

    // Best-effort: tighten the parent directory to user-only so secret files
    // (device_id, config.json, pairing keys) are not discoverable through a
    // world-executable parent. Intentionally ignored — failure is non-fatal
    // (e.g. on a filesystem that does not support Unix permissions).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
    }

    let stem = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "tmp".to_owned());
    let tmp = parent.join(format!(
        ".{}.tmp.{}.{}",
        stem,
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
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp)?;
        // Defence-in-depth: re-assert 0600 in case a restrictive parent umask
        // or a non-honouring filesystem ignored the create mode set above.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        f.write_all(bytes)?;
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

// ── Sentinel helpers ────────────────────────────────────────────────────────

/// Pre-stamp the self-write sentinel with the expected post-write change count.
///
/// Must be called **before** any `NSPasteboard` mutation (clearContents /
/// setString / setData). This closes the race window where the clipboard
/// monitor could observe a new `changeCount` before the sentinel is set:
///
///   pre_count + 2 = clearContents (+1) + setString/setData (+1)
///
/// If macOS increments by a different amount the sentinel will be slightly off;
/// `sentinel_post_stamp` corrects it after the write completes.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[inline]
pub(crate) fn sentinel_pre_stamp(sentinel: &std::sync::atomic::AtomicI64, pre_count: i64) -> i64 {
    let expected = pre_count + 2;
    sentinel.store(expected, std::sync::atomic::Ordering::Release);
    expected
}

/// Post-stamp the sentinel with the actual post-write change count — but ONLY
/// if no concurrent third-party write occurred between our write and this read.
///
/// If `actual == expected` our write was the only one; we confirm the exact
/// count so the monitor can skip it cleanly.
///
/// If `actual != expected` a third-party app wrote to the pasteboard after us.
/// In that case we leave the sentinel at `expected` (which the monitor has
/// already consumed or will never see again). Overwriting with `actual` would
/// mark the third-party's content as a daemon self-write, silently suppressing
/// it — see CopyPaste-8yzf.
///
/// On write failure, call [`sentinel_reset`] instead.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[inline]
pub(crate) fn sentinel_post_stamp(
    sentinel: &std::sync::atomic::AtomicI64,
    actual: i64,
    expected: i64,
) {
    if actual == expected {
        sentinel.store(actual, std::sync::atomic::Ordering::Release);
    }
    // else: third-party wrote after us; leave sentinel at `expected` (stale,
    // harmless — the monitor will not see it again).
}

/// Reset the sentinel to `-1` after a failed pasteboard write.
///
/// A value of `-1` is never a valid `NSPasteboard changeCount` (macOS starts
/// the counter at 0 and increments monotonically), so `-1` permanently
/// disables suppression until the next write stamps a real value.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[inline]
pub(crate) fn sentinel_reset(sentinel: &std::sync::atomic::AtomicI64) {
    sentinel.store(-1, std::sync::atomic::Ordering::Release);
}

// ── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicI64;

    // ── write_atomic_0600 ──────────────────────────────────────────────────

    /// The helper creates the file, writes the exact bytes, and sets mode 0600.
    #[cfg(unix)]
    #[test]
    fn write_atomic_0600_creates_file_with_correct_content_and_mode() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("test_secret.txt");
        let content = b"hello, daemon";

        write_atomic_0600(&path, content).expect("write_atomic_0600 should succeed");

        let actual = std::fs::read(&path).expect("file should exist");
        assert_eq!(actual, content, "file content must match");

        let mode = std::fs::metadata(&path)
            .expect("metadata")
            .permissions()
            .mode();
        // Check the lower 9 bits: must be 0600 (owner rw only).
        assert_eq!(mode & 0o777, 0o600, "file mode must be 0600, got {mode:#o}");
    }

    /// Writing to a nested directory that does not yet exist — the helper must
    /// create it.
    #[test]
    fn write_atomic_0600_creates_parent_dirs() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("nested").join("dir").join("file.txt");

        write_atomic_0600(&path, b"data").expect("write_atomic_0600 should create parents");
        assert!(path.exists(), "file should exist after write");
    }

    /// Overwriting an existing file produces the new content atomically.
    #[cfg(unix)]
    #[test]
    fn write_atomic_0600_overwrites_existing_content() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("overwrite_me.txt");

        write_atomic_0600(&path, b"first").expect("first write");
        write_atomic_0600(&path, b"second").expect("second write");

        let actual = std::fs::read(&path).expect("file should exist");
        assert_eq!(actual, b"second", "overwrite must replace content");

        let mode = std::fs::metadata(&path)
            .expect("metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "mode must remain 0600 after overwrite");
    }

    // ── Sentinel helpers ───────────────────────────────────────────────────

    /// sentinel_pre_stamp stores pre+2 and returns it.
    #[test]
    fn sentinel_pre_stamp_stores_expected_value() {
        let s = AtomicI64::new(-1);
        let expected = sentinel_pre_stamp(&s, 10);
        assert_eq!(expected, 12, "pre_stamp should return pre+2");
        assert_eq!(
            s.load(std::sync::atomic::Ordering::Acquire),
            12,
            "atomic must hold the expected value"
        );
    }

    /// sentinel_post_stamp only updates when actual == expected (no racing write).
    #[test]
    fn sentinel_post_stamp_updates_only_on_match() {
        let s = AtomicI64::new(12);

        // Our write produced exactly the expected count.
        sentinel_post_stamp(&s, 12, 12);
        assert_eq!(
            s.load(std::sync::atomic::Ordering::Acquire),
            12,
            "sentinel should be confirmed when actual == expected"
        );
    }

    /// sentinel_post_stamp must NOT update when a third-party write raced.
    /// Overwriting with the third-party count would suppress their content.
    #[test]
    fn sentinel_post_stamp_does_not_suppress_third_party_write() {
        // Our write expected changeCount=12, but a third party wrote → actual=13.
        let s = AtomicI64::new(12); // pre-stamped at expected

        sentinel_post_stamp(&s, 13, 12); // actual != expected → must NOT update

        assert_eq!(
            s.load(std::sync::atomic::Ordering::Acquire),
            12,
            "sentinel must stay at expected (12), not third-party count (13)"
        );
    }

    /// sentinel_reset stores -1 unconditionally.
    #[test]
    fn sentinel_reset_stores_minus_one() {
        let s = AtomicI64::new(42);
        sentinel_reset(&s);
        assert_eq!(
            s.load(std::sync::atomic::Ordering::Acquire),
            -1,
            "sentinel_reset must store -1"
        );
    }

    /// Full roundtrip: pre-stamp → no racing write → post-stamp confirms exact count.
    #[test]
    fn sentinel_roundtrip_no_race() {
        let s = AtomicI64::new(-1);

        let pre = 10_i64;
        let expected = sentinel_pre_stamp(&s, pre);
        // After our write, changeCount is exactly expected.
        sentinel_post_stamp(&s, expected, expected);

        assert_eq!(
            s.load(std::sync::atomic::Ordering::Acquire),
            expected,
            "sentinel must hold the confirmed count after a clean write"
        );
    }

    /// Full roundtrip: pre-stamp → racing write → sentinel stays at expected.
    #[test]
    fn sentinel_roundtrip_with_racing_third_party_write() {
        let s = AtomicI64::new(-1);

        let pre = 10_i64;
        let expected = sentinel_pre_stamp(&s, pre);
        let third_party_count = expected + 1; // someone else wrote after us

        sentinel_post_stamp(&s, third_party_count, expected);

        let val = s.load(std::sync::atomic::Ordering::Acquire);
        assert_eq!(
            val, expected,
            "sentinel must stay at expected ({expected}), not third-party count ({third_party_count})"
        );
        // The monitor would see `third_party_count` and sentinel != third_party_count → not suppressed.
        assert_ne!(
            val, third_party_count,
            "third-party content must not be suppressed"
        );
    }
}

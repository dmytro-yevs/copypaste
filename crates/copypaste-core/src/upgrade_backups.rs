//! Encrypted copies of the history kept before a schema migration.
//!
//! A copy is taken from the keyed connection that is about to migrate, so it
//! is as unreadable without the device secret as the live file. Restoring one
//! goes through [`crate::history_backup::restore`], which migrates the staged
//! copy forward before validating it.
use std::path::{Path, PathBuf};

use copypaste_fs::PendingFile;
use rusqlite::Connection;

/// Directory beside the history database that holds the copies.
pub const DIR_NAME: &str = "upgrade-backups";
/// Copies kept; the oldest beyond this are deleted after each new copy.
pub const KEEP: usize = 5;
const EXTENSION: &str = "copypaste-backup";

/// One copy, newest first in [`list`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradeBackup {
    pub path: PathBuf,
    pub created_at_ms: i64,
    pub from_version: u32,
    pub size_bytes: u64,
}

/// Copy `conn` into `dir` before it migrates from `from_version`.
///
/// Best effort: a failed copy is logged and the migration still runs, because
/// the migration itself is transactional and a full disk must not lock the
/// user out of their history.
pub(crate) fn create(conn: &Connection, db_key: &[u8; 32], dir: &Path, from_version: u32) {
    if let Err(error) = try_create(conn, db_key, dir, from_version) {
        tracing::warn!(?error, from_version, "could not keep a pre-upgrade backup");
    }
    prune(dir, KEEP);
}

fn try_create(
    conn: &Connection,
    db_key: &[u8; 32],
    dir: &Path,
    from_version: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(file_name(crate::now_ms(), from_version));
    let pending = PendingFile::create(&path)?;
    crate::storage::copy_keyed(conn, pending.temp_path(), db_key, |_, _| true)?;
    pending.publish()?;
    Ok(())
}

fn file_name(created_at_ms: i64, from_version: u32) -> String {
    format!("{created_at_ms:013}-v{from_version}.{EXTENSION}")
}

fn parse(path: &Path) -> Option<(i64, u32)> {
    let name = path.file_name()?.to_str()?;
    let stem = name.strip_suffix(&format!(".{EXTENSION}"))?;
    let (created, version) = stem.split_once("-v")?;
    Some((created.parse().ok()?, version.parse().ok()?))
}

/// The copies in `dir`, newest first; missing or unreadable entries are skipped.
pub fn list(dir: &Path) -> Vec<UpgradeBackup> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut backups = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let (created_at_ms, from_version) = parse(&path)?;
            let metadata = entry.metadata().ok().filter(std::fs::Metadata::is_file)?;
            Some(UpgradeBackup {
                path,
                created_at_ms,
                from_version,
                size_bytes: metadata.len(),
            })
        })
        .collect::<Vec<_>>();
    backups.sort_by_key(|backup| std::cmp::Reverse(backup.created_at_ms));
    backups
}

fn prune(dir: &Path, keep: usize) {
    for stale in list(dir).into_iter().skip(keep) {
        if let Err(error) = std::fs::remove_file(&stale.path) {
            tracing::warn!(?error, "could not remove an old pre-upgrade backup");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip_and_foreign_files_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        for (at, version) in [(3, 9), (1, 7), (2, 8)] {
            std::fs::write(dir.path().join(file_name(at, version)), b"x").unwrap();
        }
        std::fs::write(dir.path().join("notes.txt"), b"x").unwrap();

        let listed = list(dir.path());

        assert_eq!(
            listed
                .iter()
                .map(|b| (b.created_at_ms, b.from_version))
                .collect::<Vec<_>>(),
            [(3, 9), (2, 8), (1, 7)]
        );
    }

    #[test]
    fn pruning_keeps_the_newest_copies() {
        let dir = tempfile::tempdir().unwrap();
        for at in 0..(KEEP as i64 + 2) {
            std::fs::write(dir.path().join(file_name(at, 1)), b"x").unwrap();
        }

        prune(dir.path(), KEEP);

        let kept = list(dir.path());
        assert_eq!(kept.len(), KEEP);
        assert_eq!(kept.last().unwrap().created_at_ms, 2);
    }
}

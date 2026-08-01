//! Safe temporary files for native file paste-back.

use std::io::Write;
use std::path::{Path, PathBuf};

use copypaste_core::FileMetadata;

/// Materialise bytes at a private, deterministic filename.  The source path is
/// never persisted or reported; the metadata has only a validated basename.
pub(super) fn materialize(
    root: &Path,
    bytes: &[u8],
    metadata: &FileMetadata,
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(root)?;
    set_private_dir(root)?;
    // `item_id` may have crossed P2P before it reaches paste-back.  The
    // materialised filename is derived locally from the verified bytes instead
    // of trusting that remote identifier as a path component.
    let target = root.join(format!(
        "{}-{}",
        copypaste_core::binary_item_id(bytes),
        metadata.filename
    ));
    if target.exists() {
        return Ok(target);
    }
    let mut temporary = tempfile::NamedTempFile::new_in(root)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    set_private_file(temporary.path())?;
    match temporary.persist_noclobber(&target) {
        Ok(_) => Ok(target),
        Err(err) if err.error.kind() == std::io::ErrorKind::AlreadyExists => Ok(target),
        Err(err) => Err(err.error),
    }
}

#[cfg(unix)]
fn set_private_dir(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn set_private_dir(_: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_private_file(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_private_file(_: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materialized_file_is_private_and_never_uses_an_untrusted_id() {
        let root = tempfile::tempdir().unwrap();
        let metadata = FileMetadata::new("report.pdf", "application/pdf").unwrap();
        let path = materialize(root.path(), b"bytes", &metadata).unwrap();
        assert_eq!(
            path.file_name().unwrap().to_string_lossy(),
            format!("{}-report.pdf", copypaste_core::binary_item_id(b"bytes"))
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"bytes");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn a_remote_traversal_id_cannot_escape_the_materialization_root() {
        let root = tempfile::tempdir().unwrap();
        let metadata = FileMetadata::new("report.pdf", "application/pdf").unwrap();
        let remote_item_id = "../../outside";
        // P2P item ids are deliberately absent from this API: regardless of
        // what a peer supplied, only the locally-derived content id is used.
        let path = materialize(root.path(), b"bytes", &metadata).unwrap();
        assert!(path.starts_with(root.path()));
        assert_ne!(path.file_name().unwrap().to_string_lossy(), remote_item_id);
        assert!(!root.path().join("report.pdf").exists());
    }
}

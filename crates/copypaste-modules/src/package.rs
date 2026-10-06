use crate::ModuleError;
use copypaste_module_sdk::{ModuleManifest, ModuleTarget};
use minisign_verify::{PublicKey, Signature};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{Read, Write},
    path::Path,
};

pub(crate) const MAX_PACKAGE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 256 * 1024;
const MAX_SIGNATURE_BYTES: u64 = 4096;
const MANIFEST: &str = "manifest.json";
const SIGNATURE: &str = "manifest.json.sig";

pub(crate) struct PackageVerifier {
    key: PublicKey,
    host_version: String,
    target: ModuleTarget,
}

impl PackageVerifier {
    pub(crate) fn new(
        key: &str,
        host_version: &str,
        target: ModuleTarget,
    ) -> Result<Self, ModuleError> {
        Ok(Self {
            key: PublicKey::from_base64(key).map_err(|_| ModuleError::State)?,
            host_version: host_version.into(),
            target,
        })
    }

    fn manifest(&self, bytes: &[u8], signature: &[u8]) -> Result<ModuleManifest, ModuleError> {
        let signature = std::str::from_utf8(signature).map_err(|_| invalid_signature())?;
        let signature = Signature::decode(signature).map_err(|_| invalid_signature())?;
        self.key
            .verify(bytes, &signature, false)
            .map_err(|_| invalid_signature())?;
        let manifest: ModuleManifest = serde_json::from_slice(bytes)
            .map_err(|_| ModuleError::Invalid("The module manifest is invalid.".into()))?;
        manifest.validate(&self.host_version, self.target)?;
        let mut size = 0u64;
        let mut paths = BTreeSet::new();
        for file in &manifest.files {
            size = size
                .checked_add(file.size_bytes)
                .ok_or_else(package_too_large)?;
            if !paths.insert(file.path.to_lowercase()) {
                return Err(ModuleError::Invalid("Module file paths collide.".into()));
            }
        }
        if size > MAX_PACKAGE_BYTES {
            return Err(package_too_large());
        }
        Ok(manifest)
    }

    /// Extract only inventoried regular files, after authenticating the manifest.
    pub(crate) fn extract(
        &self,
        source: &Path,
        destination: &Path,
    ) -> Result<ModuleManifest, ModuleError> {
        let file = File::open(source)?;
        if file.metadata()?.len() > MAX_PACKAGE_BYTES {
            return Err(package_too_large());
        }
        let mut archive = zip::ZipArchive::new(file).map_err(|_| {
            ModuleError::Invalid("The module package is not a valid archive.".into())
        })?;
        if archive.len() > 4098 {
            return Err(package_too_large());
        }
        let mut names = BTreeSet::new();
        for index in 0..archive.len() {
            let file = archive.by_index(index).map_err(|_| invalid_archive())?;
            let kind = file.unix_mode().unwrap_or(0) & 0o170000;
            if file.is_dir()
                || (kind != 0 && kind != 0o100000)
                || !copypaste_module_sdk::valid_relative_path(file.name())
                || !names.insert(file.name().to_owned())
            {
                return Err(invalid_archive());
            }
        }
        let bytes = read_entry(&mut archive, MANIFEST, MAX_MANIFEST_BYTES)?;
        let signature = read_entry(&mut archive, SIGNATURE, MAX_SIGNATURE_BYTES)?;
        let manifest = self.manifest(&bytes, &signature)?;
        let expected: BTreeSet<_> = manifest
            .files
            .iter()
            .map(|f| f.path.as_str())
            .chain([MANIFEST, SIGNATURE])
            .collect();
        if names.iter().map(String::as_str).collect::<BTreeSet<_>>() != expected {
            return Err(invalid_archive());
        }
        for inventory in &manifest.files {
            let mut source = archive
                .by_name(&inventory.path)
                .map_err(|_| invalid_archive())?;
            if source.size() != inventory.size_bytes {
                return Err(invalid_archive());
            }
            let path = destination.join(&inventory.path);
            fs::create_dir_all(path.parent().ok_or_else(invalid_archive)?)?;
            let mut target = File::options().write(true).create_new(true).open(&path)?;
            let mut hash = Sha256::new();
            let mut total = 0u64;
            let mut buffer = [0u8; 64 * 1024];
            loop {
                let count = source.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                total += count as u64;
                if total > inventory.size_bytes {
                    return Err(invalid_archive());
                }
                hash.update(&buffer[..count]);
                target.write_all(&buffer[..count])?;
            }
            if total != inventory.size_bytes
                || hex::encode(hash.finalize()) != inventory.sha256.to_lowercase()
            {
                return Err(ModuleError::Invalid(
                    "A module file failed its integrity check.".into(),
                ));
            }
            target.sync_all()?;
        }
        for (name, content) in [(MANIFEST, bytes), (SIGNATURE, signature)] {
            let mut target = File::create(destination.join(name))?;
            target.write_all(&content)?;
            // Windows requires a writable handle for FlushFileBuffers.
            target.sync_all()?;
        }
        Ok(manifest)
    }

    /// Re-authenticate installed code before it is loaded, including after restart.
    pub(crate) fn installed(
        &self,
        directory: &Path,
        verify_files: bool,
    ) -> Result<ModuleManifest, ModuleError> {
        let bytes = read_regular(directory, MANIFEST, MAX_MANIFEST_BYTES)?;
        let signature = read_regular(directory, SIGNATURE, MAX_SIGNATURE_BYTES)?;
        let manifest = self.manifest(&bytes, &signature)?;
        if !verify_files {
            return Ok(manifest);
        }
        for inventory in &manifest.files {
            regular_path(directory, &inventory.path)?;
            let mut source = File::open(directory.join(&inventory.path))?;
            if source.metadata()?.len() != inventory.size_bytes {
                return Err(invalid_archive());
            }
            let mut hash = Sha256::new();
            let mut buffer = [0u8; 64 * 1024];
            loop {
                let count = source.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                hash.update(&buffer[..count]);
            }
            if hex::encode(hash.finalize()) != inventory.sha256.to_lowercase() {
                return Err(ModuleError::Invalid(
                    "An installed module failed its integrity check.".into(),
                ));
            }
        }
        Ok(manifest)
    }
}

fn read_entry<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
    limit: u64,
) -> Result<Vec<u8>, ModuleError> {
    let source = archive.by_name(name).map_err(|_| invalid_archive())?;
    if source.size() > limit {
        return Err(package_too_large());
    }
    let mut bytes = Vec::new();
    source.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(package_too_large());
    }
    Ok(bytes)
}

fn read_regular(directory: &Path, name: &str, limit: u64) -> Result<Vec<u8>, ModuleError> {
    regular_path(directory, name)?;
    let file = File::open(directory.join(name))?;
    if file.metadata()?.len() > limit {
        return Err(package_too_large());
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(package_too_large());
    }
    Ok(bytes)
}

pub(crate) fn regular_path(directory: &Path, relative: &str) -> Result<(), ModuleError> {
    let root = fs::symlink_metadata(directory)?;
    if !root.is_dir() || root.file_type().is_symlink() {
        return Err(invalid_archive());
    }
    let mut path = directory.to_path_buf();
    let parts: Vec<_> = relative.split('/').collect();
    for (index, part) in parts.iter().enumerate() {
        path.push(part);
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink()
            || if index + 1 == parts.len() {
                !metadata.is_file()
            } else {
                !metadata.is_dir()
            }
        {
            return Err(invalid_archive());
        }
    }
    Ok(())
}

fn invalid_signature() -> ModuleError {
    ModuleError::Invalid("The module is not signed by CopyPaste.".into())
}
fn invalid_archive() -> ModuleError {
    ModuleError::Invalid("The module package file inventory is invalid.".into())
}
fn package_too_large() -> ModuleError {
    ModuleError::Invalid("The module package exceeds its size limit.".into())
}

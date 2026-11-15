//! Prepare signed module updates without activating code in the old app process.

use super::{ModuleError, ModuleManager, PackageVerifier};
use copypaste_module_sdk::valid_id;
use semver::Version;
use std::{fs, path::Path};

impl ModuleManager {
    /// Only installed modules may accompany an application update. Authentication
    /// uses the destination app version; the current registry stays untouched.
    pub fn stage_update(
        &self,
        id: &str,
        package: &Path,
        app_version: &str,
    ) -> Result<(), ModuleError> {
        let _mutation = self.mutation.lock().map_err(|_| ModuleError::State)?;
        let version = Version::parse(app_version).map_err(|_| ModuleError::State)?;
        let current =
            Version::parse(&self.worker_identity.app_version).map_err(|_| ModuleError::State)?;
        if !valid_id(id) || version <= current {
            return Err(ModuleError::Invalid(
                "The application update is not newer.".into(),
            ));
        }
        let installed = self
            .state
            .lock()
            .map_err(|_| ModuleError::State)?
            .registry
            .modules
            .get(id)
            .filter(|record| !record.removing)
            .cloned()
            .ok_or(ModuleError::NotInstalled)?;
        let verifier = PackageVerifier::new(
            &self.worker_identity.public_key,
            app_version,
            self.worker_identity.target,
        )?;
        let pending = self.root.join("updates").join(version.to_string());
        fs::create_dir_all(&pending)?;
        let stage = tempfile::tempdir_in(&pending)?;
        let payload = stage.path().join("package");
        fs::create_dir(&payload)?;
        let manifest = verifier.extract(package, &payload)?;
        if manifest.id != id
            || Version::parse(&manifest.version).map_err(|_| ModuleError::State)?
                <= Version::parse(&installed.version).map_err(|_| ModuleError::State)?
        {
            return Err(ModuleError::Invalid(
                "The module update is not newer.".into(),
            ));
        }
        let destination = pending.join(id).join(&manifest.version);
        fs::create_dir_all(destination.parent().ok_or(ModuleError::State)?)?;
        if destination.exists() {
            if verifier.installed(&destination.join("package"), true)? != manifest {
                return Err(ModuleError::Invalid(
                    "The staged module update conflicts.".into(),
                ));
            }
        } else {
            fs::rename(stage.path(), destination)?;
        }
        Ok(())
    }

    /// Startup runs this before exposing the manager to any native consumer.
    /// A missing, corrupt, removed, or incompatible update keeps the old module.
    pub(super) fn activate_staged_updates(&self) {
        let Ok(current) = Version::parse(&self.worker_identity.app_version) else {
            return;
        };
        let Ok(batches) = fs::read_dir(self.root.join("updates")) else {
            return;
        };
        let eligible: Vec<_> = batches
            .flatten()
            .filter_map(|entry| {
                let version = Version::parse(entry.file_name().to_str()?).ok()?;
                (version <= current).then(|| entry.path())
            })
            .collect();
        let Ok(state) = self.state.lock() else { return };
        let installed = state.registry.modules.clone();
        drop(state);
        for (id, record) in installed {
            if record.removing {
                continue;
            }
            let mut candidates = Vec::new();
            for pending in &eligible {
                let Ok(entries) = fs::read_dir(pending.join(&id)) else {
                    continue;
                };
                candidates.extend(entries.flatten().filter_map(|entry| {
                    let version = Version::parse(entry.file_name().to_str()?).ok()?;
                    Some((version, entry.path()))
                }));
            }
            candidates.sort_by(|a, b| b.0.cmp(&a.0));
            for (version, path) in candidates {
                let payload = path.join("package");
                // The version directory remains as a receipt if a process exits
                // after the payload rename but before the registry commit.
                let source = if payload.exists() {
                    payload
                } else {
                    self.directory(&id, &version.to_string())
                };
                let Ok(manifest) = self.verifier.installed(&source, true) else {
                    continue;
                };
                if manifest.id == id
                    && manifest.version == version.to_string()
                    && self.activate_package(manifest, &source).is_ok()
                {
                    break;
                }
            }
        }
        // Consume eligible batches once without networking. Packages for a
        // newer application stay staged if its installer has not succeeded.
        for pending in eligible {
            let _ = fs::remove_dir_all(pending);
        }
    }
}

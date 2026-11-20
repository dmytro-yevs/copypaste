use crate::{
    bridge::{HostAccess, HostShared},
    endpoints::EndpointSession,
    native::{LoadSpec, ModuleInstance},
    package::PackageVerifier,
    ModuleError,
};
use copypaste_module_sdk::{
    resolve_fields, valid_id, ModuleCapability, ModuleDelivery, ModuleEvent, ModuleInvocation,
    ModuleManifest, ModuleOutput, ModuleTarget,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

mod updates;

/// The last locale the app reported, beside the registry.
const LOCALE_FILE: &str = "locale";

fn valid_locale(locale: &str) -> bool {
    !locale.is_empty()
        && locale.len() <= 35
        && locale
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledModule {
    pub search_language_field: Option<String>,
    pub search_models: Vec<copypaste_module_sdk::SearchModelInfo>,
    pub events: Vec<ModuleEvent>,
    #[serde(default)]
    pub supported_platforms: Vec<copypaste_module_sdk::ModulePlatform>,
    pub restart_required: bool,
    pub id: String,
    pub title: String,
    pub description: String,
    pub version: String,
    pub enabled: bool,
    pub size_bytes: u64,
    pub commands: Vec<copypaste_module_sdk::ModuleCommand>,
    #[serde(default)]
    pub capabilities: Vec<copypaste_module_sdk::ModuleCapability>,
    #[serde(default)]
    pub endpoints: Vec<copypaste_module_sdk::ModuleEndpoint>,
    /// Where the app shows the module, localized (schema 7).
    #[serde(default)]
    pub surfaces: copypaste_module_sdk::ModuleSurfaces,
    pub preference_fields: Vec<copypaste_module_sdk::ModuleField>,
    pub preferences: BTreeMap<String, Value>,
    pub error: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: String,
    enabled: bool,
    preferences: BTreeMap<String, Value>,
    #[serde(default)]
    removing: bool,
    /// Declared capabilities the user confirmed when enabling the module.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    capabilities: Vec<ModuleCapability>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Registry {
    schema_version: u32,
    modules: BTreeMap<String, Record>,
}

struct State {
    registry: Registry,
}

struct ResourceHealth {
    fingerprint: crate::resources::ResourceFingerprint,
    healthy: bool,
}

struct VerifiedManifest {
    version: String,
    fingerprint: crate::package::PackageFingerprint,
    manifest: Arc<ModuleManifest>,
}

/// One owner per application data directory. Registry mutations are serialized;
/// native execution uses a separate lock per module, outside the registry lock.
pub struct ModuleManager {
    root: PathBuf,
    verifier: PackageVerifier,
    state: Mutex<State>,
    instances: crate::instances::Instances,
    mutation: Mutex<()>,
    sync_services: Mutex<Option<Arc<crate::SyncServices>>>,
    shared: Arc<HostShared>,
    model_health: Mutex<BTreeMap<String, ResourceHealth>>,
    manifests: Mutex<BTreeMap<String, VerifiedManifest>>,
    inference: crate::inference::InferenceClient,
    worker_identity: crate::inference::WorkerIdentity,
    // Drop last, after inference and loaded native consumers have shut down.
    _staging_owner: fs::File,
}

impl ModuleManager {
    pub fn open(
        root: &Path,
        app_version: &str,
        target: ModuleTarget,
        public_key: &str,
    ) -> Result<Self, ModuleError> {
        fs::create_dir_all(root.join("packages"))?;
        fs::create_dir_all(root.join("data"))?;
        let root = fs::canonicalize(root)?;
        let mut registry = match fs::read(root.join("registry.json")) {
            Ok(bytes) if bytes.len() <= 1024 * 1024 => {
                serde_json::from_slice::<Registry>(&bytes).map_err(|_| ModuleError::State)?
            }
            Ok(_) => return Err(ModuleError::State),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Registry {
                schema_version: 1,
                ..Registry::default()
            },
            Err(error) => return Err(error.into()),
        };
        if registry.schema_version != 1 || registry.modules.len() > 128 {
            return Err(ModuleError::State);
        }
        // The id and version name package and data paths, so an invalid record
        // is never used. It must not make the other modules unavailable; its
        // files stay on disk.
        registry.modules.retain(|id, record| {
            let valid = valid_id(id) && semver::Version::parse(&record.version).is_ok();
            if !valid {
                tracing::warn!("ignoring an invalid module registry record");
            }
            valid
        });
        let verifier = PackageVerifier::new(public_key, app_version, target)?;
        let staging_owner = crate::staging_cleanup::open(&root)?;
        let manager = Self {
            root,
            verifier,
            state: Mutex::new(State { registry }),
            instances: crate::instances::Instances::new(),
            mutation: Mutex::new(()),
            sync_services: Mutex::new(None),
            shared: Arc::default(),
            model_health: Mutex::new(BTreeMap::new()),
            manifests: Mutex::new(BTreeMap::new()),
            inference: crate::inference::InferenceClient::new(),
            worker_identity: crate::inference::WorkerIdentity {
                app_version: app_version.into(),
                target,
                public_key: public_key.into(),
            },
            _staging_owner: staging_owner,
        };
        if let Some(locale) = fs::read_to_string(manager.root.join(LOCALE_FILE))
            .ok()
            .filter(|locale| valid_locale(locale))
        {
            manager.shared.set_locale(locale);
        }
        manager.activate_staged_updates();
        Ok(manager)
    }

    fn directory(&self, id: &str, version: &str) -> PathBuf {
        self.root.join("packages").join(id).join(version)
    }

    pub fn set_inference_launcher(&self, launcher: Arc<dyn crate::InferenceLauncher>) {
        self.inference.set_launcher(launcher);
    }

    pub(crate) fn stop_inference(&self) {
        self.inference.stop(None);
    }

    /// Stops a loaded provider before its host-owned credentials are cleared.
    /// This does not mutate the module registry or broaden provider access.
    pub(crate) fn stop_module(&self, id: &str) -> Result<(), ModuleError> {
        let _mutation = self.mutation.lock().map_err(|_| ModuleError::State)?;
        self.inference.stop(Some(id));
        let loaded = {
            let _state = self.state.lock().map_err(|_| ModuleError::State)?;
            self.instances.remove(id)
        };
        drop(loaded);
        Ok(())
    }

    /// Verified once per installed package state; loading code verifies again.
    fn manifest(&self, id: &str, record: &Record) -> Result<Arc<ModuleManifest>, ModuleError> {
        let directory = self.directory(id, &record.version);
        // Taken before reading, so a concurrent change can only cause a miss.
        let fingerprint = crate::package::fingerprint(&directory);
        let mut cache = self.manifests.lock().map_err(|_| ModuleError::State)?;
        if let (Some(cached), Some(fingerprint)) = (cache.get(id), &fingerprint) {
            if cached.version == record.version && &cached.fingerprint == fingerprint {
                return Ok(Arc::clone(&cached.manifest));
            }
        }
        cache.remove(id);
        let manifest = self.verifier.installed(&directory, false)?;
        if manifest.id != id || manifest.version != record.version {
            return Err(ModuleError::State);
        }
        let manifest = Arc::new(manifest);
        if let Some(fingerprint) = fingerprint {
            cache.insert(
                id.into(),
                VerifiedManifest {
                    version: record.version.clone(),
                    fingerprint,
                    manifest: Arc::clone(&manifest),
                },
            );
        }
        Ok(manifest)
    }

    fn persist(&self, registry: &Registry) -> Result<(), ModuleError> {
        let bytes = serde_json::to_vec(registry).map_err(|_| ModuleError::State)?;
        if bytes.len() > 1024 * 1024 {
            return Err(ModuleError::Invalid(
                "The module registry exceeds its size limit.".into(),
            ));
        }
        let summaries =
            serde_json::to_vec(&self.summaries(registry)).map_err(|_| ModuleError::State)?;
        if summaries.len() > copypaste_module_sdk::MAX_INVOCATION_BYTES {
            return Err(ModuleError::Invalid(
                "The installed module metadata exceeds its size limit.".into(),
            ));
        }
        copypaste_fs::write_atomically(
            &self.root.join("registry.json"),
            &bytes,
            copypaste_fs::Visibility::OwnerOnly,
        )?;
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<InstalledModule>, ModuleError> {
        let _mutation = self.mutation.lock().map_err(|_| ModuleError::State)?;
        // Cleanup debt stays persisted; a failed retry must not hide the list.
        if let Err(error) = self.finish_pending_removals() {
            tracing::warn!(%error, "module removal cleanup failed");
        }
        let state = self.state.lock().map_err(|_| ModuleError::State)?;
        Ok(self.summaries(&state.registry))
    }

    fn summaries(&self, registry: &Registry) -> Vec<InstalledModule> {
        let locale = self.shared.locale();
        registry
            .modules
            .iter()
            .map(|(id, record)| {
                if record.removing {
                    let restart_required =
                        crate::native::pinned_under(&self.root.join("packages").join(id));
                    let mut module = failed_summary(
                        id,
                        record,
                        if restart_required {
                            "Restart CopyPaste to finish removing this module."
                        } else {
                            "Module removal is finishing cleanup. Try again if it remains."
                        }
                        .into(),
                    );
                    module.restart_required = restart_required;
                    return module;
                }
                match self.manifest(id, record) {
                    Ok(manifest) => {
                        match validate_stored_preferences(&manifest, &record.preferences) {
                            Ok(_) => self.summary(manifest.localized(&locale), record),
                            Err(error) => failed_summary(id, record, error),
                        }
                    }
                    Err(error) => failed_summary(id, record, error.to_string()),
                }
            })
            .collect()
    }

    /// Installing a newer package performs an atomic version switch. Existing
    /// settings and enabled state survive updates; incompatible preferences reset.
    pub fn install(&self, package: &Path) -> Result<InstalledModule, ModuleError> {
        let _mutation = self.mutation.lock().map_err(|_| ModuleError::State)?;
        let stage = tempfile::tempdir_in(self.root.join("packages"))?;
        let manifest = self.verifier.extract(package, stage.path())?;
        self.activate_package(manifest, stage.path())
    }

    fn activate_package(
        &self,
        manifest: ModuleManifest,
        stage: &Path,
    ) -> Result<InstalledModule, ModuleError> {
        let mut state = self.state.lock().map_err(|_| ModuleError::State)?;
        let old = state.registry.modules.get(&manifest.id).cloned();
        if old.as_ref().is_some_and(|record| record.removing) {
            return Err(ModuleError::Invalid(
                "Finish removing this module before installing it again.".into(),
            ));
        }
        if let Some(old) = &old {
            let current = semver::Version::parse(&old.version).map_err(|_| ModuleError::State)?;
            let incoming =
                semver::Version::parse(&manifest.version).map_err(|_| ModuleError::State)?;
            if incoming <= current {
                return Err(ModuleError::Invalid(
                    "Install a newer version of this module.".into(),
                ));
            }
        } else if state.registry.modules.len() >= 128 {
            return Err(ModuleError::Invalid(
                "Too many modules are installed.".into(),
            ));
        }
        let preferences = manifest
            .preferences
            .iter()
            .map(|field| {
                let value = old
                    .as_ref()
                    .and_then(|r| r.preferences.get(&field.id))
                    .filter(|value| field.accepts_stored(value))
                    .cloned()
                    .unwrap_or_else(|| field.default_value());
                (field.id.clone(), value)
            })
            .collect();
        // Kept only while every declared capability stays confirmed: an
        // update asking for more waits disabled for the user to confirm again.
        let confirmed = old.as_ref().is_some_and(|old| {
            manifest
                .capabilities
                .iter()
                .all(|capability| old.capabilities.contains(capability))
        });
        let record = Record {
            version: manifest.version.clone(),
            enabled: old.as_ref().map_or(
                manifest.event_handlers.is_empty()
                    && manifest.search_provider.is_none()
                    && manifest.sync_provider.is_none()
                    && manifest.capabilities.is_empty(),
                |old| old.enabled && (manifest.capabilities.is_empty() || confirmed),
            ),
            preferences,
            removing: false,
            capabilities: old
                .as_ref()
                .map(|old| {
                    old.capabilities
                        .iter()
                        .filter(|capability| manifest.capabilities.contains(capability))
                        .copied()
                        .collect()
                })
                .unwrap_or_default(),
        };
        let destination = self.directory(&manifest.id, &manifest.version);
        fs::create_dir_all(destination.parent().ok_or(ModuleError::State)?)?;
        // A previous interrupted activation may have left this unreferenced version.
        if destination.exists() {
            if self.verifier.installed(&destination, true)? != manifest {
                return Err(ModuleError::Invalid(
                    "The staged module version conflicts with an existing package.".into(),
                ));
            }
        } else {
            fs::rename(stage, &destination)?;
        }
        let mut registry = state.registry.clone();
        registry.modules.insert(manifest.id.clone(), record.clone());
        if let Err(error) = self.persist(&registry) {
            let _ = fs::remove_dir_all(&destination);
            return Err(error);
        }
        let previous = self.instances.remove(&manifest.id);
        if manifest.search_provider.is_some() {
            self.inference.stop(Some(&manifest.id));
        }
        state.registry = registry;
        drop(state);
        drop(previous);
        // A stuck teardown leaves the old version for the next cleanup pass.
        if let Some(old) = old.filter(|_| self.instances.torn_down(&manifest.id)) {
            let _ = fs::remove_dir_all(self.directory(&manifest.id, &old.version));
        }
        Ok(self.summary(manifest, &record))
    }

    /// Enabling grants every declared capability, so `confirmed` must echo
    /// all of them as confirmed by the user; the confirmed set is persisted.
    pub fn set_enabled(
        &self,
        id: &str,
        enabled: bool,
        confirmed: &[ModuleCapability],
    ) -> Result<(), ModuleError> {
        let _mutation = self.mutation.lock().map_err(|_| ModuleError::State)?;
        let mut state = self.state.lock().map_err(|_| ModuleError::State)?;
        let mut registry = state.registry.clone();
        let record = registry
            .modules
            .get_mut(id)
            .ok_or(ModuleError::NotInstalled)?;
        if record.removing {
            return Err(ModuleError::Invalid(
                "Finish removing this module first.".into(),
            ));
        }
        if enabled {
            let manifest = self.manifest(id, record)?;
            if manifest
                .capabilities
                .iter()
                .any(|capability| !confirmed.contains(capability))
            {
                return Err(ModuleError::Invalid(
                    "Confirm the module permissions before enabling it.".into(),
                ));
            }
            if let Some(provider) = &manifest.search_provider {
                let model = selected_model(provider, &record.preferences)?;
                if !self.model_healthy(id, model) {
                    return Err(ModuleError::Invalid("Choose languages and download their model before enabling semantic search.".into()));
                }
            }
            record.capabilities = manifest.capabilities.clone();
        }
        record.enabled = enabled;
        self.persist(&registry)?;
        state.registry = registry;
        if !enabled {
            self.inference.stop(Some(id));
        }
        let previous = if !enabled {
            self.instances.remove(id)
        } else {
            None
        };
        drop(state);
        drop(previous);
        Ok(())
    }

    pub fn set_preferences(
        &self,
        id: &str,
        values: BTreeMap<String, Value>,
    ) -> Result<(), ModuleError> {
        // Downloads do not hold the registry/state locks or block other modules.
        let (manifest, preferences) = {
            let state = self.state.lock().map_err(|_| ModuleError::State)?;
            let record = state
                .registry
                .modules
                .get(id)
                .ok_or(ModuleError::NotInstalled)?;
            if record.removing {
                return Err(ModuleError::Disabled);
            }
            let manifest = self.manifest(id, record)?;
            let preferences = resolve_fields(&manifest.preferences, &values)?;
            (manifest, preferences)
        };
        let prepared = if let Some(provider) = &manifest.search_provider {
            crate::resources::prepare(
                &self.root,
                &self.root.join("data").join(id),
                selected_model(provider, &preferences)?,
                || {
                    self.state.lock().is_ok_and(|state| {
                        state.registry.modules.get(id).is_some_and(|record| {
                            !record.removing && record.version == manifest.version
                        })
                    })
                },
            )?
        } else {
            None
        };
        // Declared before the locks, so teardown runs after they are released.
        let mut retired = Vec::new();
        let _mutation = self.mutation.lock().map_err(|_| ModuleError::State)?;
        let mut state = self.state.lock().map_err(|_| ModuleError::State)?;
        let mut registry = state.registry.clone();
        let record = registry
            .modules
            .get_mut(id)
            .ok_or(ModuleError::NotInstalled)?;
        if record.removing || record.version != manifest.version {
            return Err(ModuleError::Disabled);
        }
        let same_search_model = manifest.search_provider.as_ref().is_some_and(|provider| {
            selected_model(provider, &record.preferences).ok()
                == selected_model(provider, &preferences).ok()
        });
        record.preferences = preferences;
        // Release model sessions before replacing repaired model files on Windows.
        if manifest.search_provider.is_some() && (!same_search_model || prepared.is_some()) {
            self.inference.stop(Some(id));
        }
        if !same_search_model || prepared.is_some() {
            retired.extend(self.instances.remove(id));
        }
        if let Some(prepared) = prepared {
            // The lifecycle lease prevents removal during final publication.
            prepared.publish(
                &self.root.join("data").join(id),
                selected_model(
                    manifest
                        .search_provider
                        .as_ref()
                        .ok_or(ModuleError::State)?,
                    &record.preferences,
                )?,
            )?;
        }
        crate::preferences::store(
            self.shared.services().as_deref(),
            id,
            &manifest.preferences,
            &record.preferences,
        )?;
        self.persist(&registry)?;
        state.registry = registry;
        if manifest.sync_provider.is_some() {
            retired.extend(self.instances.remove(id));
        }
        if let Some(record) = state.registry.modules.get(id) {
            self.instances
                .set_preferences(id, record.preferences.clone());
        }
        Ok(())
    }

    pub(crate) fn search_configuration(&self) -> Result<Option<SearchConfiguration>, ModuleError> {
        let state = self.state.lock().map_err(|_| ModuleError::State)?;
        self.search_configuration_in(&state.registry)
    }

    fn search_configuration_in(
        &self,
        registry: &Registry,
    ) -> Result<Option<SearchConfiguration>, ModuleError> {
        for (id, record) in &registry.modules {
            if !record.enabled || record.removing {
                continue;
            }
            // A broken unrelated package must not disable search for others.
            let Ok(manifest) = self.manifest(id, record) else {
                continue;
            };
            let Some(provider) = &manifest.search_provider else {
                continue;
            };
            let Ok(model) = selected_model(provider, &record.preferences) else {
                continue;
            };
            if !self.model_healthy(id, model) {
                continue;
            }
            use sha2::Digest;
            let scope = hex::encode(sha2::Sha256::digest(
                serde_json::to_vec(&(id, &record.version, model))
                    .map_err(|_| ModuleError::State)?,
            ));
            return Ok(Some(SearchConfiguration {
                id: id.clone(),
                command: provider.command.clone(),
                model: model.clone(),
                scope,
            }));
        }
        Ok(None)
    }

    /// Hold admission only through storage publication, not model inference.
    /// Other modules' network commands cannot block a search inference call.
    pub(crate) fn search_lease<T>(
        &self,
        configuration: &SearchConfiguration,
        run: impl FnOnce() -> Result<T, ModuleError>,
    ) -> Result<T, ModuleError> {
        let state = self.state.lock().map_err(|_| ModuleError::State)?;
        if self
            .search_configuration_in(&state.registry)?
            .as_ref()
            .map(|value| &value.scope)
            != Some(&configuration.scope)
        {
            return Err(ModuleError::Disabled);
        }
        run()
    }

    pub fn remove(&self, id: &str) -> Result<(), ModuleError> {
        let _mutation = self.mutation.lock().map_err(|_| ModuleError::State)?;
        let mut state = self.state.lock().map_err(|_| ModuleError::State)?;
        if !state.registry.modules.contains_key(id) {
            return Err(ModuleError::NotInstalled);
        }
        let mut registry = state.registry.clone();
        let record = registry
            .modules
            .get_mut(id)
            .ok_or(ModuleError::NotInstalled)?;
        record.removing = true;
        record.enabled = false;
        record.preferences.clear();
        // Persist removal before releasing code and deleting package/data. A crash
        // can leave unused files, but can never reactivate a removed module.
        self.persist(&registry)?;
        let previous = self.instances.remove(id);
        self.inference.stop(Some(id));
        state.registry = registry;
        drop(state);
        drop(previous);
        if let Ok(mut manifests) = self.manifests.lock() {
            manifests.remove(id);
        }
        // Teardown may still write; a stuck one stays persisted cleanup debt.
        if !self.instances.torn_down(id) {
            return Ok(());
        }
        let state_cleared = self.clear_host_state(id);
        let packages = self.root.join("packages").join(id);
        if crate::native::pinned_under(&packages) {
            let data = self.root.join("data").join(id);
            let data_cleared = !data.exists() || fs::remove_dir_all(data).is_ok();
            // The persisted removal state is the durable cleanup debt. The
            // module is already disabled, so a locked library or a transient
            // state-store failure must not be reported as a failed removal.
            if !state_cleared || !data_cleared {
                return Ok(());
            }
            return Ok(());
        }
        let files_cleared = [packages, self.root.join("data").join(id)]
            .iter()
            .all(|directory| !directory.exists() || fs::remove_dir_all(directory).is_ok());
        if !state_cleared || !files_cleared {
            return Ok(());
        }
        let Ok(mut state) = self.state.lock() else {
            return Ok(());
        };
        let mut registry = state.registry.clone();
        registry.modules.remove(id);
        if self.persist(&registry).is_err() {
            return Ok(());
        }
        state.registry = registry;
        Ok(())
    }

    /// Local encrypted state, the derived index and the module's records,
    /// synced preferences included.
    fn clear_host_state(&self, id: &str) -> bool {
        match self.sync_services.lock() {
            Ok(services) => services
                .as_ref()
                .is_none_or(|services| services.clear(id).is_ok()),
            Err(_) => false,
        }
    }

    fn finish_pending_removals(&self) -> Result<(), ModuleError> {
        let mut state = self.state.lock().map_err(|_| ModuleError::State)?;
        let pending: Vec<_> = state
            .registry
            .modules
            .iter()
            .filter(|(_, record)| record.removing)
            .map(|(id, _)| id.clone())
            .collect();
        let mut registry = state.registry.clone();
        for id in pending {
            if self.instances.tearing_down(&id) || !self.clear_host_state(&id) {
                continue;
            }
            let packages = self.root.join("packages").join(&id);
            if crate::native::pinned_under(&packages) {
                continue;
            }
            let cleanup = [packages, self.root.join("data").join(&id)]
                .iter()
                .try_for_each(|path| {
                    if path.exists() {
                        fs::remove_dir_all(path)
                    } else {
                        Ok(())
                    }
                });
            if cleanup.is_ok() {
                registry.modules.remove(&id);
            }
        }
        // Once process-scoped code is gone, reclaim inactive package versions
        // retained by earlier updates. Only manager-owned version directories qualify.
        for (id, record) in &registry.modules {
            if self.instances.tearing_down(id) {
                continue;
            }
            let parent = self.root.join("packages").join(id);
            if let Ok(entries) = fs::read_dir(&parent) {
                for entry in entries.flatten() {
                    let version = entry.file_name().to_string_lossy().into_owned();
                    if version != record.version
                        && semver::Version::parse(&version).is_ok()
                        && entry.file_type().is_ok_and(|kind| kind.is_dir())
                        && !crate::native::pinned_under(&entry.path())
                    {
                        let _ = fs::remove_dir_all(entry.path());
                    }
                }
            }
        }
        if registry.modules.len() != state.registry.modules.len() {
            self.persist(&registry)?;
            state.registry = registry;
        }
        Ok(())
    }

    fn summary(&self, manifest: ModuleManifest, record: &Record) -> InstalledModule {
        let models = manifest
            .search_provider
            .as_ref()
            .map(|provider| {
                provider
                    .models
                    .iter()
                    .map(|model| copypaste_module_sdk::SearchModelInfo {
                        id: model.id.clone(),
                        title: model.title.clone(),
                        languages: model.languages.clone(),
                        size_bytes: model.size_bytes(),
                        available: self.model_healthy(&manifest.id, model),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let preferences = self.preferences(&manifest.id, &manifest, &record.preferences);
        let mut result = summary(manifest, record);
        result.search_models = models;
        result.preferences = preferences;
        result
    }

    fn model_healthy(&self, id: &str, model: &copypaste_module_sdk::SearchModel) -> bool {
        let data = self.root.join("data").join(id);
        let Some(fingerprint) = crate::resources::fingerprint(&data, model) else {
            return false;
        };
        let key = format!("{id}/{}", model.resource_directory_name());
        let mut health = match self.model_health.lock() {
            Ok(health) => health,
            Err(_) => return false,
        };
        if let Some(previous) = health.get(&key) {
            if previous.fingerprint == fingerprint {
                return previous.healthy;
            }
        }
        let healthy = crate::resources::verify(&data, model).is_ok();
        health.insert(
            key,
            ResourceHealth {
                fingerprint,
                healthy,
            },
        );
        healthy
    }

    pub fn set_sync_services(&self, services: Arc<crate::SyncServices>) -> Result<(), ModuleError> {
        *self.sync_services.lock().map_err(|_| ModuleError::State)? = Some(services);
        Ok(())
    }
    pub(crate) fn sync_providers(
        &self,
        enabled_only: bool,
    ) -> Result<Vec<(String, String)>, ModuleError> {
        let state = self.state.lock().map_err(|_| ModuleError::State)?;
        let mut providers = Vec::new();
        for (id, record) in &state.registry.modules {
            if record.removing || (enabled_only && !record.enabled) {
                continue;
            }
            // One broken package must not hide or keep running the others.
            match self.manifest(id, record) {
                Ok(manifest) => {
                    if let Some(provider) = &manifest.sync_provider {
                        providers.push((id.clone(), provider.command.clone()));
                    }
                }
                Err(error) => tracing::warn!(%error, module = id, "skipping an unreadable module"),
            }
        }
        Ok(providers)
    }
    pub(crate) fn stop_sync_providers(&self) -> Result<(), ModuleError> {
        let providers = self.sync_providers(false)?;
        let instances: Vec<_> = {
            let _state = self.state.lock().map_err(|_| ModuleError::State)?;
            providers
                .iter()
                .filter_map(|(id, _)| self.instances.remove(id))
                .collect()
        };
        drop(instances);
        Ok(())
    }

    pub fn has_sms_handler(&self) -> Result<bool, ModuleError> {
        let state = self.state.lock().map_err(|_| ModuleError::State)?;
        for (id, record) in &state.registry.modules {
            if !record.enabled || record.removing {
                continue;
            }
            // One broken package must not hide other modules' handlers.
            let manifest = match self.manifest(id, record) {
                Ok(manifest) => manifest,
                Err(error) => {
                    tracing::warn!(%error, module = id, "skipping an unreadable module");
                    continue;
                }
            };
            if manifest
                .event_handlers
                .iter()
                .any(|handler| handler.event == ModuleEvent::SmsReceived)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Lifecycle mutations wait through recognition and host publication. No
    /// callback from disabled, updated or removed code can publish afterwards.
    pub fn dispatch_sms(
        &self,
        text: &str,
        mut publish: impl FnMut(&str) -> Result<(), ModuleError>,
    ) -> Result<bool, ModuleError> {
        if text.is_empty() || text.len() > 64 * 1024 {
            return Ok(false);
        }
        let _mutation = self.mutation.lock().map_err(|_| ModuleError::State)?;
        let handlers = {
            let state = self.state.lock().map_err(|_| ModuleError::State)?;
            let mut handlers = Vec::new();
            for (id, record) in &state.registry.modules {
                if !record.enabled || record.removing {
                    continue;
                }
                let manifest = match self.manifest(id, record) {
                    Ok(manifest) => manifest,
                    Err(error) => {
                        tracing::warn!(%error, module = id, "skipping an unreadable module");
                        continue;
                    }
                };
                for handler in &manifest.event_handlers {
                    if handler.event == ModuleEvent::SmsReceived {
                        handlers.push((id.clone(), handler.command.clone()));
                    }
                }
            }
            handlers
        };
        let mut codes = std::collections::BTreeSet::new();
        for (id, command) in handlers {
            // A failing handler yields no code; it must not block the others
            // or keep the message queued forever.
            let result = match self.invoke(
                &id,
                &command,
                BTreeMap::from([("text".into(), Value::String(text.into()))]),
            ) {
                Ok(result) => result,
                Err(error) => {
                    tracing::warn!(%error, module = id, "an SMS handler failed");
                    continue;
                }
            };
            if let ModuleOutput::Text { text: code } = result {
                if code.is_empty() {
                    continue;
                }
                if !(4..=10).contains(&code.len())
                    || !code.bytes().all(|byte| byte.is_ascii_alphanumeric())
                {
                    tracing::warn!(module = id, "an SMS handler returned an invalid code");
                    continue;
                }
                codes.insert(code);
            }
        }
        if codes.len() != 1 {
            return Ok(false);
        }
        publish(codes.first().ok_or(ModuleError::State)?)?;
        Ok(true)
    }

    /// Invokes on behalf of the host or the app.
    pub fn invoke(
        &self,
        id: &str,
        command: &str,
        arguments: BTreeMap<String, Value>,
    ) -> Result<ModuleOutput, ModuleError> {
        self.invoke_as(id, command, arguments, true)
    }

    /// `app` is false for local clients other than the app; they are refused
    /// `app_only` commands.
    pub fn invoke_as(
        &self,
        id: &str,
        command: &str,
        arguments: BTreeMap<String, Value>,
        app: bool,
    ) -> Result<ModuleOutput, ModuleError> {
        let state = self.state.lock().map_err(|_| ModuleError::State)?;
        let record = state
            .registry
            .modules
            .get(id)
            .cloned()
            .ok_or(ModuleError::NotInstalled)?;
        if !record.enabled || record.removing {
            return Err(ModuleError::Disabled);
        }
        let manifest = self.manifest(id, &record)?;
        let command = manifest
            .commands
            .iter()
            .find(|entry| entry.id == command)
            .ok_or_else(|| ModuleError::Invalid("The module command is not registered.".into()))?;
        if command.app_only && !app {
            return Err(ModuleError::Invalid(
                "Only the CopyPaste app can run this command.".into(),
            ));
        }
        let mut invocation = ModuleInvocation {
            command: command.id.clone(),
            arguments: resolve_fields(&command.arguments, &arguments)?,
            preferences: resolve_fields(
                &manifest.preferences,
                &self.preferences(id, &manifest, &record.preferences),
            )?,
        };
        if manifest.search_provider.is_some() || crate::inference::is_ocr_command(id, &command.id) {
            let generation = self.inference.generation();
            drop(state);
            let _inputs =
                crate::input::snapshot(&self.root, &command.arguments, &mut invocation.arguments)?;
            let request = crate::inference::InferenceRequest {
                identity: self.worker_identity.clone(),
                id: id.into(),
                package_dir: self.directory(id, &record.version),
                data_dir: self.root.join("data").join(id),
                invocation,
            };
            return self.inference.invoke(request, generation);
        }
        let lease = self.lease(id, &manifest);
        drop(state);
        let instance = lease.ready()?;
        let result = (|| {
            let _inputs =
                crate::input::snapshot(&self.root, &command.arguments, &mut invocation.arguments)?;
            let package_dir = self.directory(id, &record.version);
            let data_dir = self.root.join("data").join(id);
            instance.invoke(
                LoadSpec {
                    verifier: &self.verifier,
                    package_dir: &package_dir,
                    data_dir: &data_dir,
                    preferences: &invocation.preferences,
                    access: self.access(&manifest, &record)?,
                },
                &invocation,
            )
        })();
        drop(instance);
        self.instances.release(id);
        let output = result?;
        checked_output(&manifest, output)
    }

    /// Answers a host service request of `caller` with the first enabled
    /// module that provides `service`; `None` when no other module does.
    pub fn serve_service(
        &self,
        caller: &str,
        service: copypaste_module_sdk::ModuleService,
        request: &copypaste_module_sdk::ModuleHostRequest,
    ) -> Option<Result<Value, copypaste_module_sdk::HostError>> {
        use copypaste_module_sdk::{HostError, HostErrorCode, ServiceReply};
        let provider = {
            let state = self.state.lock().ok()?;
            state
                .registry
                .modules
                .iter()
                .filter(|(id, record)| record.enabled && !record.removing && *id != caller)
                .find_map(|(id, record)| {
                    let manifest = self.manifest(id, record).ok()?;
                    let command = manifest
                        .provides
                        .iter()
                        .find(|provider| provider.service == service)?
                        .command
                        .clone();
                    Some((id.clone(), command))
                })
        }?;
        let unavailable = |error: &dyn std::fmt::Display| {
            tracing::warn!(%error, module = provider.0, "a service provider failed");
            HostError::new(
                HostErrorCode::Unavailable,
                "The module providing this service is unavailable.",
            )
        };
        let request = match serde_json::to_string(request) {
            Ok(request) => request,
            Err(error) => return Some(Err(unavailable(&error))),
        };
        let output = match self.invoke(
            &provider.0,
            &provider.1,
            BTreeMap::from([("request".to_owned(), Value::String(request))]),
        ) {
            Ok(output) => output,
            Err(error) => return Some(Err(unavailable(&error))),
        };
        let ModuleOutput::Data { data } = output else {
            return Some(Err(unavailable(&"not data")));
        };
        Some(match serde_json::from_value::<ServiceReply>(data) {
            Ok(ServiceReply::Ok { reply }) => Ok(reply),
            Ok(ServiceReply::Error { error }) => Err(error),
            Err(error) => Err(unavailable(&error)),
        })
    }

    /// Call `ready` on the lease after releasing the state lock.
    fn lease(&self, id: &str, manifest: &ModuleManifest) -> crate::instances::Lease {
        self.instances
            .lease(id, manifest.sync_provider.is_some(), || {
                ModuleInstance::new(id, Arc::clone(&self.shared))
            })
    }

    fn access(
        &self,
        manifest: &ModuleManifest,
        record: &Record,
    ) -> Result<HostAccess, ModuleError> {
        Ok(HostAccess {
            sync: self
                .sync_services
                .lock()
                .map_err(|_| ModuleError::State)?
                .clone(),
            sync_provider: manifest.sync_provider.is_some(),
            capabilities: manifest
                .capabilities
                .iter()
                .filter(|capability| record.capabilities.contains(capability))
                .copied()
                .collect(),
            preference_fields: manifest.preferences.clone(),
            storage: manifest.storage.clone(),
            provides: manifest
                .provides
                .iter()
                .map(|provider| provider.service)
                .collect(),
        })
    }

    /// The registry values of `id` with its synced values applied.
    fn preferences(
        &self,
        id: &str,
        manifest: &ModuleManifest,
        values: &BTreeMap<String, Value>,
    ) -> BTreeMap<String, Value> {
        crate::preferences::merged(
            self.shared.services().as_deref(),
            id,
            &manifest.preferences,
            values.clone(),
        )
    }

    /// Opens an endpoint connection. The session keeps the instance loaded
    /// until it closes. `privileged` is true only for the verified app.
    pub fn connect(
        &self,
        id: &str,
        endpoint: &str,
        privileged: bool,
    ) -> Result<EndpointSession, ModuleError> {
        let state = self.state.lock().map_err(|_| ModuleError::State)?;
        let record = state
            .registry
            .modules
            .get(id)
            .cloned()
            .ok_or(ModuleError::NotInstalled)?;
        if !record.enabled || record.removing {
            return Err(ModuleError::Disabled);
        }
        let manifest = self.manifest(id, &record)?;
        let platform = self.worker_identity.target.platform;
        if manifest.search_provider.is_some()
            || !record.capabilities.contains(&ModuleCapability::Endpoints)
            || !manifest
                .endpoints
                .iter()
                .any(|declared| declared.id == endpoint && declared.platforms.contains(&platform))
        {
            return Err(ModuleError::Invalid(
                "This module has no such endpoint on this device.".into(),
            ));
        }
        let preferences = resolve_fields(
            &manifest.preferences,
            &self.preferences(id, &manifest, &record.preferences),
        )?;
        let lease = self.lease(id, &manifest);
        drop(state);
        let instance = lease.ready()?;
        let (connection, outbound) = instance.endpoints().open()?;
        let session = EndpointSession::new(
            connection.clone(),
            Arc::clone(&instance),
            outbound,
            self.instances.session_use(id),
        );
        let package_dir = self.directory(id, &record.version);
        let data_dir = self.root.join("data").join(id);
        instance.open_endpoint(
            LoadSpec {
                verifier: &self.verifier,
                package_dir: &package_dir,
                data_dir: &data_dir,
                preferences: &preferences,
                access: self.access(&manifest, &record)?,
            },
            &ModuleDelivery::EndpointOpen {
                endpoint: endpoint.into(),
                connection,
                privileged,
            },
        )?;
        Ok(session)
    }

    /// The UI locale for summaries and the `context` host request. It is
    /// kept across restarts, so endpoint clients served before the app
    /// reconnects see it too.
    pub fn set_locale(&self, locale: &str) -> Result<(), ModuleError> {
        if !valid_locale(locale) {
            return Err(ModuleError::Invalid("The locale is invalid.".into()));
        }
        if self.shared.locale() != locale {
            copypaste_fs::write_atomically(
                &self.root.join(LOCALE_FILE),
                locale.as_bytes(),
                copypaste_fs::Visibility::OwnerOnly,
            )?;
            self.shared.set_locale(locale.into());
        }
        Ok(())
    }

    pub fn set_host_services(&self, services: Arc<dyn crate::ModuleHostServices>) {
        self.shared.set_services(services);
    }

    pub fn set_prompt_listener(&self, listener: crate::PromptListener) {
        self.shared.prompts.set_listener(listener);
    }

    pub fn prompts(&self) -> Vec<copypaste_module_sdk::ModulePrompt> {
        self.shared.prompts.list()
    }

    pub fn answer_prompt(
        &self,
        prompt_id: &str,
        choice: Option<String>,
    ) -> Result<(), ModuleError> {
        self.shared.prompts.answer(prompt_id, choice)
    }
}

fn summary(manifest: ModuleManifest, record: &Record) -> InstalledModule {
    let events = manifest
        .event_handlers
        .iter()
        .map(|handler| handler.event)
        .collect();
    let commands = manifest
        .commands
        .into_iter()
        .filter(|command| {
            manifest
                .sync_provider
                .as_ref()
                .is_none_or(|provider| provider.command != command.id)
                && manifest
                    .search_provider
                    .as_ref()
                    .is_none_or(|provider| provider.command != command.id)
                && !manifest
                    .event_handlers
                    .iter()
                    .any(|handler| handler.command == command.id)
                && !command.hidden
        })
        .collect();
    InstalledModule {
        search_language_field: manifest
            .search_provider
            .as_ref()
            .map(|provider| provider.language_field.clone()),
        search_models: Vec::new(),
        events,
        supported_platforms: if manifest.schema_version == 1 {
            vec![
                copypaste_module_sdk::ModulePlatform::Macos,
                copypaste_module_sdk::ModulePlatform::Windows,
                copypaste_module_sdk::ModulePlatform::Android,
            ]
        } else {
            manifest.supported_platforms
        },
        restart_required: false,
        id: manifest.id,
        title: manifest.title,
        description: manifest.description,
        version: manifest.version,
        enabled: record.enabled,
        size_bytes: manifest.files.iter().map(|file| file.size_bytes).sum(),
        commands,
        capabilities: manifest.capabilities,
        endpoints: manifest.endpoints,
        surfaces: manifest.surfaces,
        preference_fields: manifest.preferences,
        preferences: record.preferences.clone(),
        error: None,
    }
}

/// Schema 7 outputs come only from schema 7 modules, in shape, and their
/// views run only commands the module declares.
fn checked_output(
    manifest: &ModuleManifest,
    output: ModuleOutput,
) -> Result<ModuleOutput, ModuleError> {
    let newer = matches!(
        output,
        ModuleOutput::Results { .. } | ModuleOutput::Deliver { .. } | ModuleOutput::SaveFile { .. }
    );
    let declared = |command: &str| manifest.commands.iter().any(|entry| entry.id == command);
    let actions_declared = match &output {
        ModuleOutput::View { view } => view.commands().all(declared),
        ModuleOutput::Results { items } => items
            .iter()
            .flat_map(|item| &item.actions)
            .all(|action| declared(&action.command)),
        _ => true,
    };
    if (newer && manifest.schema_version < 7) || !actions_declared {
        return Err(ModuleError::Invalid(
            "The module returned an unsupported result.".into(),
        ));
    }
    output.validate().map_err(ModuleError::Invalid)?;
    Ok(output)
}

fn failed_summary(id: &str, record: &Record, error: String) -> InstalledModule {
    InstalledModule {
        search_language_field: None,
        search_models: Vec::new(),
        events: Vec::new(),
        supported_platforms: Vec::new(),
        restart_required: false,
        id: id.into(),
        title: id.into(),
        description: String::new(),
        version: record.version.clone(),
        enabled: false,
        size_bytes: 0,
        commands: Vec::new(),
        capabilities: Vec::new(),
        endpoints: Vec::new(),
        surfaces: Default::default(),
        preference_fields: Vec::new(),
        preferences: BTreeMap::new(),
        error: Some(error),
    }
}

fn validate_stored_preferences(
    manifest: &ModuleManifest,
    values: &BTreeMap<String, Value>,
) -> Result<(), String> {
    if values
        .keys()
        .any(|id| !manifest.preferences.iter().any(|field| &field.id == id))
    {
        return Err("The stored module preferences contain an unknown field.".into());
    }
    for field in &manifest.preferences {
        if let Some(value) = values.get(&field.id) {
            if !field.accepts_stored(value) {
                return Err(format!("{} has an invalid stored value.", field.title));
            }
        }
    }
    Ok(())
}

#[derive(Clone)]
pub(crate) struct SearchConfiguration {
    pub id: String,
    pub command: String,
    pub model: copypaste_module_sdk::SearchModel,
    pub scope: String,
}

fn selected_model<'a>(
    provider: &'a copypaste_module_sdk::ModuleSearchProvider,
    preferences: &BTreeMap<String, Value>,
) -> Result<&'a copypaste_module_sdk::SearchModel, ModuleError> {
    let languages: Vec<String> = preferences
        .get(&provider.language_field)
        .and_then(Value::as_array)
        .ok_or_else(|| ModuleError::Invalid("Choose at least one search language.".into()))?
        .iter()
        .map(|value| value.as_str().map(str::to_owned).ok_or(ModuleError::State))
        .collect::<Result<_, _>>()?;
    provider.model_for(&languages).ok_or_else(|| {
        ModuleError::Invalid("No model supports the selected search languages.".into())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn newer_outputs_need_schema_seven_and_views_run_declared_commands() {
        let manifest = |schema: u32| -> ModuleManifest {
            serde_json::from_value(json!({
                "schema_version": schema, "api_version": 1, "id": "example.library",
                "title": "Library", "description": "", "version": "0.1.0",
                "app_versions": ">=1.0.0", "target": {"platform": "macos", "architecture": "aarch64"},
                "supported_platforms": ["macos"], "entrypoint": "bin/module.dylib",
                "files": [{"path": "bin/module.dylib", "sha256": "0".repeat(64), "size_bytes": 1}],
                "commands": [{"id": "open", "title": "Open", "description": ""}]
            }))
            .unwrap()
        };
        let results = || ModuleOutput::Results { items: Vec::new() };
        assert!(checked_output(&manifest(7), results()).is_ok());
        assert!(checked_output(&manifest(6), results()).is_err());
        let view = |command: &str| ModuleOutput::View {
            view: copypaste_module_sdk::ModuleView {
                sections: vec![copypaste_module_sdk::ViewSection::new(
                    None,
                    vec![copypaste_module_sdk::ViewItem::row(
                        "Row".into(),
                        None,
                        vec![copypaste_module_sdk::ViewAction::new("Go", command)],
                    )],
                )],
            },
        };
        assert!(checked_output(&manifest(6), view("open")).is_ok());
        assert!(checked_output(&manifest(6), view("missing")).is_err());
    }

    #[test]
    fn a_changed_installed_package_is_verified_again() {
        let key = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let manager = ModuleManager::open(
            &directory.path().join("modules"),
            "1.0.0",
            crate::host::tests::target(),
            &key.pk.to_base64(),
        )
        .unwrap();
        let module = manager
            .install(&crate::host::tests::package(directory.path(), &key))
            .unwrap();
        assert!(manager.list().unwrap()[0].error.is_none());
        assert!(manager.manifests.lock().unwrap().contains_key(&module.id));
        fs::write(
            manager
                .directory(&module.id, &module.version)
                .join("manifest.json.sig"),
            b"replaced",
        )
        .unwrap();
        assert!(manager.list().unwrap()[0].error.is_some());
    }
}

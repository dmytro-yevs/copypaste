use crate::{native::ModuleInstance, package::PackageVerifier, ModuleError};
use copypaste_module_sdk::{
    resolve_fields, valid_id, ModuleEvent, ModuleInvocation, ModuleManifest, ModuleOutput,
    ModuleTarget,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledModule {
    pub search_language_field: Option<String>,
    pub search_models: Vec<copypaste_module_sdk::SearchModelInfo>,
    pub events: Vec<ModuleEvent>,
    pub restart_required: bool,
    pub id: String,
    pub title: String,
    pub description: String,
    pub version: String,
    pub enabled: bool,
    pub size_bytes: u64,
    pub commands: Vec<copypaste_module_sdk::ModuleCommand>,
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
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Registry {
    schema_version: u32,
    modules: BTreeMap<String, Record>,
}

struct Loaded {
    instance: Arc<ModuleInstance>,
    last_used: u64,
}
struct State {
    registry: Registry,
    loaded: BTreeMap<String, Loaded>,
    sequence: u64,
}

/// One owner per application data directory. Registry mutations are serialized;
/// native execution uses a separate lock per module, outside the registry lock.
pub struct ModuleManager {
    root: PathBuf,
    verifier: PackageVerifier,
    state: Mutex<State>,
    mutation: Mutex<()>,
    sync_services: Mutex<Option<Arc<crate::SyncServices>>>,
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
        let registry = match fs::read(root.join("registry.json")) {
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
        if registry.schema_version != 1
            || registry.modules.len() > 128
            || registry.modules.iter().any(|(id, record)| {
                !valid_id(id) || semver::Version::parse(&record.version).is_err()
            })
        {
            return Err(ModuleError::State);
        }
        Ok(Self {
            root,
            verifier: PackageVerifier::new(public_key, app_version, target)?,
            state: Mutex::new(State {
                registry,
                loaded: BTreeMap::new(),
                sequence: 0,
            }),
            mutation: Mutex::new(()),
            sync_services: Mutex::new(None),
        })
    }

    fn directory(&self, id: &str, version: &str) -> PathBuf {
        self.root.join("packages").join(id).join(version)
    }

    fn manifest(&self, id: &str, record: &Record) -> Result<ModuleManifest, ModuleError> {
        let manifest = self
            .verifier
            .installed(&self.directory(id, &record.version), false)?;
        if manifest.id != id || manifest.version != record.version {
            return Err(ModuleError::State);
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
        self.finish_pending_removals()?;
        let state = self.state.lock().map_err(|_| ModuleError::State)?;
        Ok(self.summaries(&state.registry))
    }

    fn summaries(&self, registry: &Registry) -> Vec<InstalledModule> {
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
                            "Module removal is incomplete. Remove it again to finish."
                        }
                        .into(),
                    );
                    module.restart_required = restart_required;
                    return module;
                }
                match self.manifest(id, record) {
                    Ok(manifest) => {
                        match validate_stored_preferences(&manifest, &record.preferences) {
                            Ok(_) => self.summary(manifest, record),
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
        let record = Record {
            version: manifest.version.clone(),
            enabled: old.as_ref().map_or(
                manifest.event_handlers.is_empty()
                    && manifest.search_provider.is_none()
                    && manifest.sync_provider.is_none(),
                |old| old.enabled,
            ),
            preferences,
            removing: false,
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
            fs::rename(stage.path(), &destination)?;
        }
        let mut registry = state.registry.clone();
        registry.modules.insert(manifest.id.clone(), record.clone());
        if let Err(error) = self.persist(&registry) {
            let _ = fs::remove_dir_all(&destination);
            return Err(error);
        }
        let previous = state.loaded.remove(&manifest.id);
        state.registry = registry;
        drop(state);
        if let Some(previous) = previous {
            previous.instance.stop();
        }
        if let Some(old) = old {
            let _ = fs::remove_dir_all(self.directory(&manifest.id, &old.version));
        }
        Ok(self.summary(manifest, &record))
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), ModuleError> {
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
            if let Some(provider) = &manifest.search_provider {
                let model = selected_model(provider, &record.preferences)?;
                if !crate::resources::available(&self.root.join("data").join(id), model) {
                    return Err(ModuleError::Invalid("Choose languages and download their model before enabling semantic search.".into()));
                }
            }
        }
        record.enabled = enabled;
        self.persist(&registry)?;
        state.registry = registry;
        let previous = if !enabled {
            state.loaded.remove(id)
        } else {
            None
        };
        drop(state);
        if let Some(previous) = previous {
            previous.instance.stop();
        }
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
        let previous = if !same_search_model || prepared.is_some() {
            state.loaded.remove(id)
        } else {
            None
        };
        if let Some(previous) = previous {
            previous.instance.stop();
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
        self.persist(&registry)?;
        state.registry = registry;
        let previous = if manifest.sync_provider.is_some() {
            state.loaded.remove(id)
        } else {
            None
        };
        drop(state);
        if let Some(previous) = previous {
            previous.instance.stop();
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
            let manifest = self.manifest(id, record)?;
            let Some(provider) = &manifest.search_provider else {
                continue;
            };
            let model = selected_model(provider, &record.preferences)?;
            if !crate::resources::available(&self.root.join("data").join(id), model) {
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

    pub(crate) fn search_loaded(&self, id: &str) -> bool {
        self.state
            .lock()
            .is_ok_and(|state| state.loaded.contains_key(id))
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
        let previous = state.loaded.remove(id);
        state.registry = registry;
        drop(state);
        if let Some(previous) = previous {
            previous.instance.stop();
        }
        if let Some(services) = self
            .sync_services
            .lock()
            .map_err(|_| ModuleError::State)?
            .as_ref()
        {
            services.clear(id)?;
        }
        if crate::native::pinned_under(&self.root.join("packages").join(id)) {
            let data = self.root.join("data").join(id);
            if data.exists() {
                fs::remove_dir_all(data)?;
            }
            return Ok(());
        }
        for directory in [
            self.root.join("packages").join(id),
            self.root.join("data").join(id),
        ] {
            if directory.exists() {
                fs::remove_dir_all(directory)?;
            }
        }
        let mut state = self.state.lock().map_err(|_| ModuleError::State)?;
        let mut registry = state.registry.clone();
        registry.modules.remove(id);
        self.persist(&registry)?;
        state.registry = registry;
        Ok(())
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
            if let Some(services) = self
                .sync_services
                .lock()
                .map_err(|_| ModuleError::State)?
                .as_ref()
            {
                if services.clear(&id).is_err() {
                    continue;
                }
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
                        available: crate::resources::available(
                            &self.root.join("data").join(&manifest.id),
                            model,
                        ),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut result = summary(manifest, record);
        result.search_models = models;
        result
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
            if let Some(provider) = self.manifest(id, record)?.sync_provider {
                providers.push((id.clone(), provider.command));
            }
        }
        Ok(providers)
    }
    pub(crate) fn stop_sync_providers(&self) -> Result<(), ModuleError> {
        let providers = self.sync_providers(false)?;
        let instances: Vec<_> = {
            let mut state = self.state.lock().map_err(|_| ModuleError::State)?;
            providers
                .iter()
                .filter_map(|(id, _)| state.loaded.remove(id))
                .collect()
        };
        for instance in instances {
            instance.instance.stop();
        }
        Ok(())
    }

    pub fn has_sms_handler(&self) -> Result<bool, ModuleError> {
        let state = self.state.lock().map_err(|_| ModuleError::State)?;
        for (id, record) in &state.registry.modules {
            if record.enabled
                && !record.removing
                && self
                    .manifest(id, record)?
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
                for handler in self.manifest(id, record)?.event_handlers {
                    if handler.event == ModuleEvent::SmsReceived {
                        handlers.push((id.clone(), handler.command));
                    }
                }
            }
            handlers
        };
        let mut codes = std::collections::BTreeSet::new();
        for (id, command) in handlers {
            let result = self.invoke(
                &id,
                &command,
                BTreeMap::from([("text".into(), Value::String(text.into()))]),
            )?;
            if let ModuleOutput::Text { text: code } = result {
                if code.is_empty() {
                    continue;
                }
                if !(4..=10).contains(&code.len())
                    || !code.bytes().all(|byte| byte.is_ascii_alphanumeric())
                {
                    return Err(ModuleError::Invalid(
                        "The SMS module returned an invalid code.".into(),
                    ));
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

    pub fn invoke(
        &self,
        id: &str,
        command: &str,
        arguments: BTreeMap<String, Value>,
    ) -> Result<ModuleOutput, ModuleError> {
        let mut state = self.state.lock().map_err(|_| ModuleError::State)?;
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
        let mut invocation = ModuleInvocation {
            command: command.id.clone(),
            arguments: resolve_fields(&command.arguments, &arguments)?,
            preferences: resolve_fields(&manifest.preferences, &record.preferences)?,
        };
        state.sequence = state.sequence.saturating_add(1);
        let last_used = state.sequence;
        let loaded = state.loaded.entry(id.into()).or_insert_with(|| Loaded {
            instance: Arc::new(ModuleInstance::new()),
            last_used,
        });
        loaded.last_used = last_used;
        let instance = Arc::clone(&loaded.instance);
        let evicted = trim_idle_instances(&mut state);
        drop(state);
        drop(evicted);
        let result = (|| {
            let _inputs =
                crate::input::snapshot(&self.root, &command.arguments, &mut invocation.arguments)?;
            instance.invoke(
                &self.verifier,
                &self.directory(id, &record.version),
                &self.root.join("data").join(id),
                &invocation,
                if manifest.sync_provider.is_some() {
                    self.sync_services
                        .lock()
                        .map_err(|_| ModuleError::State)?
                        .clone()
                } else {
                    None
                },
                id,
            )
        })();
        drop(instance);
        let mut state = self.state.lock().map_err(|_| ModuleError::State)?;
        let evicted = trim_idle_instances(&mut state);
        drop(state);
        drop(evicted);
        result
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
        })
        .collect();
    InstalledModule {
        search_language_field: manifest
            .search_provider
            .as_ref()
            .map(|provider| provider.language_field.clone()),
        search_models: Vec::new(),
        events,
        restart_required: false,
        id: manifest.id,
        title: manifest.title,
        description: manifest.description,
        version: manifest.version,
        enabled: record.enabled,
        size_bytes: manifest.files.iter().map(|file| file.size_bytes).sum(),
        commands,
        preference_fields: manifest.preferences,
        preferences: record.preferences.clone(),
        error: None,
    }
}

fn failed_summary(id: &str, record: &Record, error: String) -> InstalledModule {
    InstalledModule {
        search_language_field: None,
        search_models: Vec::new(),
        events: Vec::new(),
        restart_required: false,
        id: id.into(),
        title: id.into(),
        description: String::new(),
        version: record.version.clone(),
        enabled: false,
        size_bytes: 0,
        commands: Vec::new(),
        preference_fields: Vec::new(),
        preferences: BTreeMap::new(),
        error: Some(error),
    }
}

// Four idle instances bound the cost of a growing module collection. Active
// invocations retain their own lease and can never be unloaded by eviction.
fn trim_idle_instances(state: &mut State) -> Vec<Arc<ModuleInstance>> {
    let mut evicted = Vec::new();
    while state.loaded.len() > 4 {
        let oldest = state
            .loaded
            .iter()
            .filter(|(_, loaded)| Arc::strong_count(&loaded.instance) == 1)
            .min_by_key(|(_, loaded)| loaded.last_used)
            .map(|(id, _)| id.clone());
        let Some(id) = oldest else {
            break;
        };
        if let Some(loaded) = state.loaded.remove(&id) {
            evicted.push(loaded.instance);
        }
    }
    evicted
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

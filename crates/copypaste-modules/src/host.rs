use crate::{ModuleError, ModuleManager, MODULE_RELEASE_PUBLIC_KEY};
use copypaste_module_sdk::{ModuleOperation, ModuleTarget, MAX_INVOCATION_BYTES};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

/// Lightweight composition owner. Construction opens no module registry and
/// loads no library. Commands load installed code only when invoked.
pub struct ModuleHost {
    root: PathBuf,
    manager: Mutex<Option<Arc<ModuleManager>>>,
    sync_services: Mutex<Option<Arc<crate::SyncServices>>>,
    wake: tokio::sync::Notify,
    binding: Mutex<Option<crate::semantic_search::SearchBinding>>,
    search: Mutex<Option<Arc<crate::semantic_search::SemanticSearch>>>,
}

impl ModuleHost {
    pub fn new(application_data: &Path) -> Self {
        Self {
            root: application_data.join("modules"),
            manager: Mutex::new(None),
            sync_services: Mutex::new(None),
            wake: tokio::sync::Notify::new(),
            binding: Mutex::new(None),
            search: Mutex::new(None),
        }
    }

    pub(crate) fn manager(&self) -> Result<Arc<ModuleManager>, ModuleError> {
        let mut initialized = self.manager.lock().map_err(|_| ModuleError::State)?;
        if let Some(manager) = initialized.as_ref() {
            return Ok(Arc::clone(manager));
        }
        let target = ModuleTarget::current().ok_or_else(|| {
            ModuleError::Invalid("Modules are unavailable on this platform.".into())
        })?;
        let manager = Arc::new(ModuleManager::open(
            &self.root,
            env!("CARGO_PKG_VERSION"),
            target,
            MODULE_RELEASE_PUBLIC_KEY,
        )?);
        if let Some(services) = self
            .sync_services
            .lock()
            .map_err(|_| ModuleError::State)?
            .clone()
        {
            manager.set_sync_services(services)?;
        }
        *initialized = Some(Arc::clone(&manager));
        Ok(manager)
    }

    pub fn bind_search(&self, store: copypaste_core::Store, changed: Arc<dyn Fn() + Send + Sync>) {
        *self
            .binding
            .lock()
            .unwrap_or_else(|error| error.into_inner()) =
            Some(crate::semantic_search::SearchBinding { store, changed });
    }

    pub fn set_inference_launcher(
        &self,
        launcher: Arc<dyn crate::InferenceLauncher>,
    ) -> Result<(), ModuleError> {
        self.manager()?.set_inference_launcher(launcher);
        Ok(())
    }

    fn search_service(&self) -> Option<Arc<crate::semantic_search::SemanticSearch>> {
        let mut active = self.search.lock().ok()?;
        if let Some(service) = active.as_ref() {
            return Some(Arc::clone(service));
        }
        if !self.root.join("registry.json").exists() {
            return None;
        }
        let manager = self.manager().ok()?;
        manager.search_configuration().ok()??;
        let binding = self.binding.lock().ok()?;
        let service = crate::semantic_search::SemanticSearch::start(manager, binding.as_ref()?);
        *active = Some(Arc::clone(&service));
        Some(service)
    }

    pub fn history_changed(&self) {
        if let Some(service) = self
            .search
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
        {
            service.notify();
        }
    }

    pub fn query_history(
        &self,
        store: &copypaste_core::Store,
        query: &copypaste_core::HistoryQuery,
        after: Option<&copypaste_core::HistoryCursor>,
        limit: u32,
        budget: usize,
        device: Option<&str>,
    ) -> Result<copypaste_core::HistoryPage, copypaste_core::StoreError> {
        if let Some(service) = self.search_service() {
            service.query(query, after, limit, budget, device)
        } else {
            store.query_history_bounded_for_device(query, after, limit, budget, device)
        }
    }

    pub fn search(
        &self,
        store: &copypaste_core::Store,
        raw: &str,
        limit: u32,
        budget: usize,
    ) -> Result<Vec<copypaste_core::StoredItem>, copypaste_core::StoreError> {
        if self.search_service().is_none() {
            return store.search_bounded(raw, limit, budget);
        }
        let query = copypaste_core::HistoryQuery {
            search: Some(raw.into()),
            sort: copypaste_core::HistorySort::Relevance,
            ..Default::default()
        };
        self.query_history(store, &query, None, limit, budget, None)
            .map(|page| page.items)
    }

    pub fn set_sync_services(&self, services: Arc<crate::SyncServices>) -> Result<(), ModuleError> {
        *self.sync_services.lock().map_err(|_| ModuleError::State)? = Some(Arc::clone(&services));
        if let Some(manager) = self
            .manager
            .lock()
            .map_err(|_| ModuleError::State)?
            .as_ref()
        {
            manager.set_sync_services(services)?;
        }
        self.wake.notify_one();
        Ok(())
    }

    pub(crate) fn clear_cloud_credentials(&self, id: &str) -> Result<(), ModuleError> {
        self.sync_services
            .lock()
            .map_err(|_| ModuleError::State)?
            .as_ref()
            .ok_or(ModuleError::State)?
            .clear_cloud_credentials(id)
    }

    pub(crate) fn stop_cloud_provider(&self, id: &str) -> Result<(), String> {
        let manager = self
            .manager
            .lock()
            .map_err(|_| ModuleError::State)
            .map_err(|error| error.to_string())?
            .clone();
        if let Some(manager) = manager {
            manager.stop_module(id).map_err(|error| error.to_string())?;
        }
        Ok(())
    }
    pub fn wake_sync(&self) {
        self.wake.notify_one();
    }
    pub fn note_version(&self, stamp: i64) {
        if let Ok(services) = self.sync_services.lock() {
            if let Some(services) = services.as_ref() {
                let _ = services.note_versions(stamp);
            }
        }
        self.wake_sync();
    }
    pub fn sync_enabled_changed(&self, enabled: bool) {
        if !enabled {
            if let Ok(manager) = self.manager() {
                let _ = manager.stop_sync_providers();
            }
        }
        self.wake_sync();
    }
    pub async fn run_sync(self: &Arc<Self>, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let stopping = Arc::new(std::sync::atomic::AtomicBool::new(false));
        loop {
            if *shutdown.borrow() {
                break;
            }
            let host = Arc::clone(self);
            let fence = Arc::clone(&stopping);
            let mut work = tokio::task::spawn_blocking(move || -> Result<(), ModuleError> {
                if !host
                    .sync_services
                    .lock()
                    .map_err(|_| ModuleError::State)?
                    .as_ref()
                    .is_some_and(|service| service.enabled())
                {
                    return Ok(());
                }
                if !host.root.join("registry.json").exists() {
                    return Ok(());
                }
                let manager = host.manager()?;
                for (id, command) in manager.sync_providers(true)? {
                    if fence.load(std::sync::atomic::Ordering::Acquire) {
                        break;
                    }
                    let _ = manager.invoke(&id, &command, std::collections::BTreeMap::new());
                }
                Ok(())
            });
            tokio::select! {
                biased;
                _ = shutdown.changed() => {
                    stopping.store(true, std::sync::atomic::Ordering::Release);
                    self.stop_sync_workers().await;
                    let _ = work.await;
                    return;
                },
                _ = &mut work => {},
            }
            tokio::select! {
                biased;
                _ = shutdown.changed() => break,
                _ = self.wake.notified() => {},
                _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {},
            }
        }
        stopping.store(true, std::sync::atomic::Ordering::Release);
        self.stop_sync_workers().await;
    }
    async fn stop_sync_workers(self: &Arc<Self>) {
        let host = Arc::clone(self);
        let _ = tokio::task::spawn_blocking(move || {
            if let Ok(manager) = host.manager() {
                let _ = manager.stop_sync_providers();
            }
        })
        .await;
    }

    pub fn has_sms_handler(&self) -> Result<bool, ModuleError> {
        if ModuleTarget::current().is_none() {
            return Ok(false);
        }
        self.manager()?.has_sms_handler()
    }

    pub fn dispatch_sms(
        &self,
        text: &str,
        publish: impl FnMut(&str) -> Result<(), ModuleError>,
    ) -> Result<bool, ModuleError> {
        if ModuleTarget::current().is_none() {
            return Ok(false);
        }
        self.manager()?.dispatch_sms(text, publish)
    }

    pub async fn request(
        self: &Arc<Self>,
        operation: ModuleOperation,
    ) -> Result<String, ModuleError> {
        let host = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            let manager = host.manager()?;
            let changed = !matches!(
                operation,
                ModuleOperation::List
                    | ModuleOperation::Invoke { .. }
                    | ModuleOperation::StageUpdate { .. }
            );
            let result = match operation {
                ModuleOperation::List => {
                    serde_json::to_value(manager.list()?).map_err(|_| ModuleError::State)?
                }
                ModuleOperation::Install { package_path } => {
                    serde_json::to_value(manager.install(Path::new(&package_path))?)
                        .map_err(|_| ModuleError::State)?
                }
                ModuleOperation::StageUpdate {
                    id,
                    package_path,
                    app_version,
                } => {
                    manager.stage_update(&id, Path::new(&package_path), &app_version)?;
                    serde_json::Value::Null
                }
                ModuleOperation::SetEnabled { id, enabled } => {
                    manager.set_enabled(&id, enabled)?;
                    serde_json::Value::Null
                }
                ModuleOperation::SetPreferences { id, values_json } => {
                    manager.set_preferences(&id, decode_values(&values_json)?)?;
                    serde_json::Value::Null
                }
                ModuleOperation::Remove { id } => {
                    manager.remove(&id)?;
                    serde_json::Value::Null
                }
                ModuleOperation::Invoke {
                    id,
                    command,
                    arguments_json,
                } => serde_json::to_value(manager.invoke(
                    &id,
                    &command,
                    decode_values(&arguments_json)?,
                )?)
                .map_err(|_| ModuleError::State)?,
            };
            if changed {
                if let Some(service) = host.search_service() {
                    service.notify();
                }
                if let Some(binding) = host
                    .binding
                    .lock()
                    .map_err(|_| ModuleError::State)?
                    .as_ref()
                {
                    (binding.changed)();
                }
            }
            let result = serde_json::to_string(&result).map_err(|_| ModuleError::State)?;
            if result.len() > MAX_INVOCATION_BYTES {
                return Err(ModuleError::Invalid(
                    "The module response exceeds its size limit.".into(),
                ));
            }
            host.wake_sync();
            Ok(result)
        })
        .await
        .map_err(|_| ModuleError::State)?
    }
}

fn decode_values(
    json: &str,
) -> Result<std::collections::BTreeMap<String, serde_json::Value>, ModuleError> {
    if json.len() > MAX_INVOCATION_BYTES {
        return Err(ModuleError::Invalid(
            "The module input exceeds its size limit.".into(),
        ));
    }
    serde_json::from_str(json)
        .map_err(|_| ModuleError::Invalid("The module input is invalid.".into()))
}

impl Drop for ModuleHost {
    fn drop(&mut self) {
        if let Some(service) = self
            .search
            .get_mut()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        {
            service.shutdown();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SyncServices;
    use copypaste_core::{
        compute_content_hash, storage::open_validated, Keyring, NewItem, Store, StoreSource,
    };
    use copypaste_module_sdk::{ModuleArchitecture, ModulePlatform};
    use copypaste_sync::store::StoreView;
    use minisign::KeyPair;
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use std::{
        fs,
        io::{Cursor, Write},
        path::{Path, PathBuf},
        sync::Arc,
    };
    use zip::{write::SimpleFileOptions, ZipWriter};

    const MODULE_ID: &str = "copypaste.text-tools";

    fn target() -> ModuleTarget {
        ModuleTarget::current().unwrap_or(ModuleTarget {
            platform: ModulePlatform::Macos,
            architecture: ModuleArchitecture::Aarch64,
        })
    }

    fn package(directory: &Path, key: &KeyPair) -> PathBuf {
        let extension = if cfg!(windows) {
            "dll"
        } else if cfg!(target_os = "macos") {
            "dylib"
        } else {
            "so"
        };
        let entrypoint = format!("bin/module.{extension}");
        let library = b"fixture";
        let mut manifest = json!({
            "schema_version": 1,
            "api_version": 1,
            "id": MODULE_ID,
            "title": "Text Tools",
            "description": "Fixture.",
            "version": "1.0.0",
            "app_versions": ">=1.0.0, <2.0.0",
            "target": target(),
            "entrypoint": entrypoint,
            "files": [{"path": entrypoint, "sha256": hex::encode(Sha256::digest(library)), "size_bytes": library.len()}],
            "commands": [{
                "id": "transform",
                "title": "Transform",
                "description": "Fixture.",
                "arguments": [{"id": "text", "title": "Text", "kind": "text", "default": "", "required": true}]
            }],
            "preferences": []
        });
        if target().platform == ModulePlatform::Linux {
            manifest["schema_version"] = json!(5);
            manifest["supported_platforms"] = json!(["linux"]);
        }
        let bytes = serde_json::to_vec(&manifest).unwrap();
        let signature = minisign::sign(
            Some(&key.pk),
            &key.sk,
            Cursor::new(&bytes),
            Some("file:manifest.json"),
            None,
        )
        .unwrap()
        .to_string();
        let path = directory.join("fixture.cpmodule");
        let mut archive = ZipWriter::new(fs::File::create(&path).unwrap());
        for (name, contents) in [
            ("manifest.json", bytes.as_slice()),
            ("manifest.json.sig", signature.as_bytes()),
            (entrypoint.as_str(), library.as_slice()),
        ] {
            archive
                .start_file(name, SimpleFileOptions::default())
                .unwrap();
            archive.write_all(contents).unwrap();
        }
        archive.finish().unwrap();
        path
    }

    #[tokio::test]
    async fn removal_keeps_visible_cleanup_debt_when_derived_index_delete_fails() {
        let directory = tempfile::tempdir().unwrap();
        let key = KeyPair::generate_unencrypted_keypair().unwrap();
        let root = directory.path().join("modules");
        let manager =
            Arc::new(ModuleManager::open(&root, "1.0.0", target(), &key.pk.to_base64()).unwrap());
        manager.install(&package(directory.path(), &key)).unwrap();

        let keyring = Arc::new(Keyring::from_secret(&[3; 32]));
        let database = directory.path().join("history.db");
        let store = Store::open(&database, &keyring.db_key()).unwrap();
        store
            .insert(NewItem {
                id: "item".into(),
                content_ciphertext: vec![1],
                nonce: vec![1],
                content_type: "text/plain".into(),
                content_hash: compute_content_hash(b"indexed text"),
                search_text: Some("indexed text".into()),
                created_at: 1,
                app_bundle_id: None,
                app_name: None,
                payload_metadata: None,
            })
            .unwrap();
        let work = store.semantic_work("scope").unwrap().unwrap();
        store
            .save_semantic_work(MODULE_ID, "scope", &work, &[vec![1.0]])
            .unwrap();
        let source = StoreSource::new(
            store.clone(),
            Arc::clone(&keyring),
            "device".into(),
            "test".into(),
            Default::default(),
        );
        let services = Arc::new(SyncServices::new(
            store.clone(),
            StoreView::new(source, "device".into()),
            || true,
            |_| {},
        ));
        let host = Arc::new(ModuleHost::new(directory.path()));
        *host.manager.lock().unwrap() = Some(Arc::clone(&manager));
        host.bind_search(store.clone(), Arc::new(|| {}));
        host.set_sync_services(services).unwrap();

        let connection = open_validated(&database, &keyring.db_key()).unwrap();
        connection
            .execute_batch(
                "CREATE TRIGGER reject_semantic_remove BEFORE DELETE ON module_search_documents \
                 WHEN OLD.module_id = 'copypaste.text-tools' \
                 BEGIN SELECT RAISE(ABORT, 'injected semantic cleanup failure'); END;",
            )
            .unwrap();

        host.request(ModuleOperation::Remove {
            id: MODULE_ID.into(),
        })
        .await
        .unwrap();
        let pending = manager.list().unwrap();
        assert_eq!(pending.len(), 1);
        assert!(!pending[0].enabled);
        assert!(pending[0]
            .error
            .as_deref()
            .is_some_and(|message| message.contains("finishing cleanup")));
        assert_eq!(store.count().unwrap(), 1, "local history is retained");

        connection
            .execute_batch("DROP TRIGGER reject_semantic_remove;")
            .unwrap();
        assert!(manager.list().unwrap().is_empty());
    }
}

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
                ModuleOperation::List | ModuleOperation::Invoke { .. }
            );
            let removed = if let ModuleOperation::Remove { id } = &operation {
                Some(id.clone())
            } else {
                None
            };
            let result = match operation {
                ModuleOperation::List => {
                    serde_json::to_value(manager.list()?).map_err(|_| ModuleError::State)?
                }
                ModuleOperation::Install { package_path } => {
                    serde_json::to_value(manager.install(Path::new(&package_path))?)
                        .map_err(|_| ModuleError::State)?
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
            if let Some(id) = removed {
                if let Some(binding) = host
                    .binding
                    .lock()
                    .map_err(|_| ModuleError::State)?
                    .as_ref()
                {
                    binding
                        .store
                        .clear_semantic_index(&id)
                        .map_err(|_| ModuleError::State)?;
                }
            }
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

//! The snippets package against an in-memory record store standing in for
//! the runtime's: saving a snippet, finding it from Quick Paste and opening
//! the library view.
use copypaste_module_sdk::{HostError, ModuleRecord, RecordPage};
use copypaste_modules::{
    InstalledModule, ModuleHostServices, ModuleManager, ModuleOutput, RecordLimits,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

pub(super) fn qualify(
    manager: &ModuleManager,
    installed: &InstalledModule,
) -> Result<usize, String> {
    let id = &installed.id;
    manager.set_host_services(Arc::new(Records::default()));
    manager
        .set_sync_services(super::supabase::sync_services()?)
        .map_err(|e| e.to_string())?;
    manager
        .set_enabled(id, true, &installed.capabilities)
        .map_err(|e| e.to_string())?;
    let invoke = |command: &str, arguments: Value| {
        let arguments = serde_json::from_value(arguments).map_err(|e| e.to_string())?;
        manager
            .invoke(id, command, arguments)
            .map_err(|e| e.to_string())
    };
    let values = json!({"title": "Signature", "abbreviation": "sig", "body": "Best regards"});
    if !matches!(
        invoke("save", json!({ "values": values }))?,
        ModuleOutput::Message { .. }
    ) {
        return Err("The snippets package did not save a snippet.".into());
    }
    let ModuleOutput::Results { items } = invoke("search", json!({"query": "sig"}))? else {
        return Err("The snippets package did not answer a Quick Paste search.".into());
    };
    if items.len() != 1 {
        return Err("Quick Paste search must find the saved snippet.".into());
    }
    let ModuleOutput::View { view } = invoke("library", json!({}))? else {
        return Err("The snippets package did not return its library.".into());
    };
    view.validate()?;
    Ok(3)
}

#[derive(Default)]
struct Records(Mutex<BTreeMap<(String, String), String>>);

impl Records {
    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<(String, String), String>> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl ModuleHostServices for Records {
    fn record_list(
        &self,
        module_id: &str,
        prefix: Option<String>,
        after: Option<String>,
        limit: u32,
    ) -> Result<RecordPage, HostError> {
        let prefix = prefix.unwrap_or_default();
        let records = self.lock();
        let mut matching = records
            .iter()
            .filter(|((module, key), _)| {
                module == module_id
                    && key.starts_with(&prefix)
                    && after.as_ref().is_none_or(|after| key > after)
            })
            .map(|((_, key), value_json)| ModuleRecord {
                module_id: module_id.into(),
                key: key.clone(),
                value_json: value_json.clone(),
                deleted: false,
            });
        let page: Vec<_> = matching.by_ref().take(limit as usize).collect();
        let next = (matching.next().is_some())
            .then(|| page.last().map(|record| record.key.clone()))
            .flatten();
        Ok(RecordPage {
            records: page,
            next,
        })
    }
    fn record_get(&self, module_id: &str, key: &str) -> Result<Option<ModuleRecord>, HostError> {
        Ok(self
            .lock()
            .get(&(module_id.to_owned(), key.to_owned()))
            .map(|value_json| ModuleRecord {
                module_id: module_id.into(),
                key: key.into(),
                value_json: value_json.clone(),
                deleted: false,
            }))
    }
    fn record_put(
        &self,
        module_id: &str,
        key: String,
        value_json: String,
        _: RecordLimits,
    ) -> Result<(), HostError> {
        self.lock().insert((module_id.into(), key), value_json);
        Ok(())
    }
    fn record_search(
        &self,
        module_id: &str,
        query: &str,
        prefix: Option<String>,
        limit: u32,
    ) -> Result<RecordPage, HostError> {
        let query = query.to_lowercase();
        let mut page = self.record_list(module_id, prefix, None, u32::MAX)?;
        page.records
            .retain(|record| record.value_json.to_lowercase().contains(&query));
        page.records.truncate(limit as usize);
        Ok(page)
    }
    fn record_delete(&self, module_id: &str, key: &str) -> Result<(), HostError> {
        self.lock().remove(&(module_id.to_owned(), key.to_owned()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use copypaste_modules::test_support::PackageSigner;
    use std::path::Path;

    fn app_version() -> String {
        let release: serde_json::Value =
            serde_json::from_str(include_str!("../../../config/release.json")).unwrap();
        release["version"].as_str().unwrap().to_owned()
    }

    #[test]
    fn the_snippets_package_qualifies() {
        let directory = tempfile::tempdir().unwrap();
        let signer = PackageSigner::default();
        let package = directory.path().join("snippets.cpmodule");
        signer.package_module(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../modules/snippets"),
            &package,
        );
        let manager = super::ModuleManager::open(
            &directory.path().join("modules"),
            &app_version(),
            copypaste_modules::ModuleTarget::current().unwrap(),
            &signer.public_key(),
        )
        .unwrap();
        let installed = manager.install(&package).unwrap();
        assert_eq!(super::qualify(&manager, &installed), Ok(3));
    }
}

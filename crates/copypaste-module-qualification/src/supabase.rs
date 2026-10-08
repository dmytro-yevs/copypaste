//! Production sync host callbacks, endpoint policy, and account lifecycle.
use copypaste_core::{Keyring, Store, StoreSource};
use copypaste_modules::{ModuleManager, ModuleOutput, SyncServices};
use copypaste_sync::store::StoreView;
use serde_json::json;
use std::{collections::BTreeMap, sync::Arc};

pub(super) fn qualify(manager: &ModuleManager, id: &str) -> Result<usize, String> {
    let keys = Arc::new(Keyring::from_secret(&[41; 32]));
    let store = Store::open_in_memory(&keys.db_key()).map_err(|e| e.to_string())?;
    let identity = store
        .device_identity("qualification")
        .map_err(|e| e.to_string())?;
    let source = StoreSource::new(
        store.clone(),
        keys,
        identity.device_id.clone(),
        identity.device_name,
        Default::default(),
    );
    manager
        .set_sync_services(Arc::new(SyncServices::new(
            store,
            StoreView::new(source, identity.device_id),
            || true,
            |_| {},
        )))
        .map_err(|e| e.to_string())?;
    manager.set_enabled(id, true).map_err(|e| e.to_string())?;
    signed_out(manager, id, false)?;
    manager
        .set_preferences(
            id,
            BTreeMap::from([
                ("url".into(), json!("http://127.0.0.1:1")),
                ("anon_key".into(), json!("qualification-public-key")),
            ]),
        )
        .map_err(|e| e.to_string())?;
    let error = manager
        .invoke(id, "status", BTreeMap::new())
        .err()
        .ok_or("Production sync accepted an insecure endpoint")?;
    if !error.to_string().to_ascii_lowercase().contains("https") {
        return Err(format!(
            "Endpoint rejection did not establish HTTPS policy: {error}"
        ));
    }
    manager
        .set_preferences(
            id,
            BTreeMap::from([
                ("url".into(), json!("https://qualification.invalid")),
                ("anon_key".into(), json!("qualification-public-key")),
            ]),
        )
        .map_err(|e| e.to_string())?;
    signed_out(manager, id, true)?;
    manager
        .invoke(id, "sign-out", BTreeMap::new())
        .map_err(|e| e.to_string())?;
    signed_out(manager, id, true)?;
    manager.set_enabled(id, false).map_err(|e| e.to_string())?;
    if manager.invoke(id, "status", BTreeMap::new()).is_ok() {
        return Err("Disabled sync module accepted commands.".into());
    }
    manager.set_enabled(id, true).map_err(|e| e.to_string())?;
    signed_out(manager, id, true)?;
    Ok(3)
}

fn signed_out(manager: &ModuleManager, id: &str, configured: bool) -> Result<(), String> {
    let output = manager
        .invoke(id, "status", BTreeMap::new())
        .map_err(|e| e.to_string())?;
    let ModuleOutput::Data { data, .. } = output else {
        return Err("Sync package did not return account state.".into());
    };
    if data["status"]["configured"] != configured
        || data["status"]["signed_in"] != false
        || data["status"]["key_ready"] != false
    {
        return Err(format!("Unexpected isolated sync account state: {data}"));
    }
    Ok(())
}

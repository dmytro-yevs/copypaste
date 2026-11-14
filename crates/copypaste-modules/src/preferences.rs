//! Module preference values. The registry keeps every value on this device;
//! values of `synced: true` fields also live in the module's reserved
//! `host.preferences` record, which follows the user to paired devices and
//! wins over the registry copy.
use crate::{ModuleError, ModuleHostServices};
use copypaste_module_sdk::{HostErrorCode, ModuleField, HOST_PREFERENCES_RECORD};
use serde_json::Value;
use std::collections::BTreeMap;

/// `values` with each synced field replaced by the record's value, when the
/// record holds one this version of the module accepts.
pub(crate) fn merged(
    services: Option<&dyn ModuleHostServices>,
    id: &str,
    fields: &[ModuleField],
    mut values: BTreeMap<String, Value>,
) -> BTreeMap<String, Value> {
    if !fields.iter().any(|field| field.synced) {
        return values;
    }
    let Some(record) = services.and_then(|services| stored(services, id)) else {
        return values;
    };
    for field in fields.iter().filter(|field| field.synced) {
        if let Some(value) = record
            .get(&field.id)
            .filter(|value| field.accepts_stored(value))
        {
            values.insert(field.id.clone(), value.clone());
        }
    }
    values
}

/// Writes the synced fields of `values` to the record when they changed.
/// Values of fields this version does not declare are kept for others.
pub(crate) fn store(
    services: Option<&dyn ModuleHostServices>,
    id: &str,
    fields: &[ModuleField],
    values: &BTreeMap<String, Value>,
) -> Result<(), ModuleError> {
    let Some(services) = services else {
        return Ok(());
    };
    if !fields.iter().any(|field| field.synced) {
        return Ok(());
    }
    let current = stored(services, id);
    let mut record = current.clone().unwrap_or_default();
    for field in fields.iter().filter(|field| field.synced) {
        if let Some(value) = values.get(&field.id) {
            record.insert(field.id.clone(), value.clone());
        }
    }
    if current.as_ref() == Some(&record) {
        return Ok(());
    }
    let json = serde_json::to_string(&record).map_err(|_| ModuleError::State)?;
    match services.record_put(id, HOST_PREFERENCES_RECORD.into(), json) {
        Ok(()) => Ok(()),
        // A host without records keeps the values on this device only.
        Err(error) if error.code == HostErrorCode::Unsupported => Ok(()),
        Err(error) => Err(ModuleError::Invalid(error.message)),
    }
}

fn stored(services: &dyn ModuleHostServices, id: &str) -> Option<BTreeMap<String, Value>> {
    let record = services.record_get(id, HOST_PREFERENCES_RECORD).ok()??;
    serde_json::from_str(&record.value_json).ok()
}

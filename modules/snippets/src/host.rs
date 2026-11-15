//! The module's only path to CopyPaste: typed host requests through one
//! object-safe trait, so tests can stand in for the native host.
use copypaste_module_sdk::native::HostClient;
use copypaste_module_sdk::{
    HostError, HostErrorCode, ModuleHostContext, ModuleHostRequest, ModuleRecord, RecordPage,
    MAX_RECORD_PAGE,
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::BTreeMap;

pub(crate) trait Host: Send + Sync {
    fn call(&self, request: &ModuleHostRequest) -> Result<Value, HostError>;
}

impl Host for HostClient {
    fn call(&self, request: &ModuleHostRequest) -> Result<Value, HostError> {
        HostClient::call(self, request)
    }
}

pub(crate) fn request<T: DeserializeOwned>(
    host: &dyn Host,
    request: ModuleHostRequest,
) -> Result<T, HostError> {
    let value = host.call(&request)?;
    serde_json::from_value(value).map_err(|_| {
        HostError::new(
            HostErrorCode::Unavailable,
            "CopyPaste answered with an unexpected reply.",
        )
    })
}

pub(crate) fn context(host: &dyn Host) -> Option<ModuleHostContext> {
    request(host, ModuleHostRequest::Context).ok()
}

pub(crate) fn get(host: &dyn Host, key: &str) -> Result<Option<String>, HostError> {
    let record: Option<ModuleRecord> = request(
        host,
        ModuleHostRequest::RecordGet {
            key: key.to_owned(),
        },
    )?;
    Ok(record
        .filter(|record| !record.deleted)
        .map(|record| record.value_json))
}

pub(crate) fn get_json<T: DeserializeOwned>(
    host: &dyn Host,
    key: &str,
) -> Result<Option<T>, HostError> {
    Ok(get(host, key)?.and_then(|json| serde_json::from_str(&json).ok()))
}

pub(crate) fn put<T: serde::Serialize>(
    host: &dyn Host,
    key: &str,
    value: &T,
) -> Result<(), HostError> {
    let value_json = serde_json::to_string(value)
        .map_err(|_| HostError::new(HostErrorCode::Invalid, "The record could not be encoded."))?;
    request(
        host,
        ModuleHostRequest::RecordPut {
            key: key.to_owned(),
            value_json,
        },
    )
}

pub(crate) fn delete(host: &dyn Host, key: &str) -> Result<(), HostError> {
    request(
        host,
        ModuleHostRequest::RecordDelete {
            key: key.to_owned(),
        },
    )
}

/// Every live record under `prefix`, page by page.
pub(crate) fn list(
    host: &dyn Host,
    prefix: &str,
    mut each: impl FnMut(ModuleRecord),
) -> Result<(), HostError> {
    let mut after = None;
    loop {
        let page: RecordPage = request(
            host,
            ModuleHostRequest::RecordList {
                prefix: Some(prefix.to_owned()),
                after: after.take(),
                limit: MAX_RECORD_PAGE,
            },
        )?;
        for record in page.records {
            if !record.deleted {
                each(record);
            }
        }
        match page.next {
            Some(next) => after = Some(next),
            None => return Ok(()),
        }
    }
}

pub(crate) fn read_state(
    host: &dyn Host,
    keys: &[String],
) -> Result<BTreeMap<String, Option<String>>, HostError> {
    request(
        host,
        ModuleHostRequest::ReadState {
            keys: keys.to_vec(),
        },
    )
}

pub(crate) fn write_state(
    host: &dyn Host,
    values: BTreeMap<String, String>,
) -> Result<(), HostError> {
    request(host, ModuleHostRequest::WriteState { values })
}

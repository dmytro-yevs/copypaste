//! The module's only path to CopyPaste: typed host requests through one
//! object-safe trait, so tests can stand in for the native host.
use copypaste_module_sdk::native::HostClient;
use copypaste_module_sdk::{HostError, HostErrorCode, ModuleHostContext, ModuleHostRequest};
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

pub(crate) fn read_state(host: &dyn Host, key: &str) -> Result<Option<String>, HostError> {
    let mut values: BTreeMap<String, Option<String>> = request(
        host,
        ModuleHostRequest::ReadState {
            keys: vec![key.to_owned()],
        },
    )?;
    Ok(values.remove(key).flatten())
}

pub(crate) fn write_state(host: &dyn Host, key: &str, value: String) -> Result<(), HostError> {
    request(
        host,
        ModuleHostRequest::WriteState {
            values: BTreeMap::from([(key.to_owned(), value)]),
        },
    )
}

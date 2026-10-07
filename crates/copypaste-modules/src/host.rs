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
}

impl ModuleHost {
    pub fn new(application_data: &Path) -> Self {
        Self {
            root: application_data.join("modules"),
            manager: Mutex::new(None),
        }
    }

    fn manager(&self) -> Result<Arc<ModuleManager>, ModuleError> {
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
        *initialized = Some(Arc::clone(&manager));
        Ok(manager)
    }

    pub fn has_sms_handler(&self) -> Result<bool, ModuleError> {
        self.manager()?.has_sms_handler()
    }

    pub fn dispatch_sms(
        &self,
        text: &str,
        publish: impl FnMut(&str) -> Result<(), ModuleError>,
    ) -> Result<bool, ModuleError> {
        self.manager()?.dispatch_sms(text, publish)
    }

    pub async fn request(
        self: &Arc<Self>,
        operation: ModuleOperation,
    ) -> Result<String, ModuleError> {
        let host = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            let manager = host.manager()?;
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
            let result = serde_json::to_string(&result).map_err(|_| ModuleError::State)?;
            if result.len() > MAX_INVOCATION_BYTES {
                return Err(ModuleError::Invalid(
                    "The module response exceeds its size limit.".into(),
                ));
            }
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

//! Requests a loaded module sends to the host: sync provider requests and the
//! capability-gated schema 6 services. Pointer handling stays in `native`.
use crate::{endpoints::Endpoints, prompts::Prompts, ModuleHostServices, SyncServices};
use copypaste_module_sdk::{
    native::{to_exact_vec, MAX_HOST_BYTES},
    HostError, HostErrorCode, ModuleCapability, ModuleField, ModuleHostContext, ModuleHostRequest,
};
use copypaste_sync::host::SyncHostRequest;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, RwLock,
    },
};

/// Host state every module bridge shares: the UI locale, pending prompts
/// and the application services.
pub(crate) struct HostShared {
    pub(crate) prompts: Prompts,
    locale: RwLock<String>,
    services: RwLock<Option<Arc<dyn ModuleHostServices>>>,
}

impl Default for HostShared {
    fn default() -> Self {
        Self {
            prompts: Prompts::default(),
            locale: RwLock::new("en".into()),
            services: RwLock::new(None),
        }
    }
}

impl HostShared {
    pub(crate) fn locale(&self) -> String {
        self.locale
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub(crate) fn set_locale(&self, locale: String) {
        *self.locale.write().unwrap_or_else(|e| e.into_inner()) = locale;
    }
    pub(crate) fn services(&self) -> Option<Arc<dyn ModuleHostServices>> {
        self.services
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub(crate) fn set_services(&self, services: Arc<dyn ModuleHostServices>) {
        *self.services.write().unwrap_or_else(|e| e.into_inner()) = Some(services);
    }
}

/// What one loaded module may reach. Granted capabilities are the declared
/// ones the user confirmed.
pub(crate) struct HostAccess {
    pub(crate) sync: Option<Arc<SyncServices>>,
    pub(crate) sync_provider: bool,
    pub(crate) capabilities: Vec<ModuleCapability>,
    /// The manifest's preference fields, for `context`.
    pub(crate) preference_fields: Vec<ModuleField>,
}

impl HostAccess {
    /// Modules without a sync provider or capabilities are loaded without a bridge.
    pub(crate) fn needed(&self) -> bool {
        (self.sync_provider && self.sync.is_some()) || !self.capabilities.is_empty()
    }
}

pub(crate) struct HostBridge {
    pub(crate) id: String,
    pub(crate) owner: u64,
    pub(crate) access: HostAccess,
    pub(crate) shared: Arc<HostShared>,
    pub(crate) accepting: Arc<AtomicBool>,
    pub(crate) lifetime: Arc<crate::host_lifetime::HostLifetime>,
    pub(crate) endpoints: Arc<Endpoints>,
    pub(crate) preferences: Arc<RwLock<BTreeMap<String, Value>>>,
}

fn unavailable() -> HostError {
    HostError::new(
        HostErrorCode::Unavailable,
        "The host service is unavailable.",
    )
}

impl HostBridge {
    /// `Err(None)` is the untyped refusal every SDK version reads as unavailable.
    pub(crate) fn handle(&self, input: &[u8]) -> Result<Vec<u8>, Option<HostError>> {
        // Already admitted token rotations may finish saving encrypted metadata
        // during teardown. History access and the live lease close immediately.
        if !self.accepting.load(Ordering::Acquire) && !is_state_operation(input) {
            return Err(None);
        }
        if self.access.sync_provider {
            // Parsed straight into the typed request: an intermediate JSON tree
            // would hold another copy of the plaintext outside `Zeroizing`.
            if let (Ok(request), Some(sync)) = (
                serde_json::from_slice::<SyncHostRequest>(input),
                &self.access.sync,
            ) {
                let output = sync.request(&self.id, request).map_err(|_| None)?;
                return to_exact_vec(&output, MAX_HOST_BYTES).map_err(|_| None);
            }
        }
        let request: ModuleHostRequest = serde_json::from_slice(input).map_err(|_| {
            Some(HostError::new(
                HostErrorCode::Unsupported,
                "This host service is not available.",
            ))
        })?;
        self.serve(request).map_err(Some)
    }

    fn serve(&self, request: ModuleHostRequest) -> Result<Vec<u8>, HostError> {
        request.validate()?;
        if !request
            .required_capabilities()
            .iter()
            .all(|capability| self.access.capabilities.contains(capability))
        {
            return Err(HostError::new(
                HostErrorCode::Denied,
                "The module was not granted this permission.",
            ));
        }
        let state = |request: SyncHostRequest| {
            let sync = self.access.sync.as_ref().ok_or_else(unavailable)?;
            let output = sync.request(&self.id, request).map_err(|_| unavailable())?;
            reply(&output)
        };
        match request {
            ModuleHostRequest::Context => reply(&ModuleHostContext {
                locale: self.shared.locale(),
                preferences: self.preferences(),
            }),
            ModuleHostRequest::ReadState { keys } => state(SyncHostRequest::ReadState { keys }),
            ModuleHostRequest::WriteState { values } => {
                state(SyncHostRequest::WriteState { values })
            }
            ModuleHostRequest::ClearState { keys } => state(SyncHostRequest::ClearState { keys }),
            ModuleHostRequest::Prompt(prompt) => reply(&self.shared.prompts.ask(
                &self.id,
                self.owner,
                &self.accepting,
                prompt,
            )?),
            ModuleHostRequest::EndpointSend { connection, data } => {
                self.endpoints.send(&connection, data)?;
                reply(&())
            }
            ModuleHostRequest::EndpointClose { connection } => {
                self.endpoints.close(&connection);
                reply(&())
            }
            request => match self.shared.services() {
                Some(services) => crate::services::serve(services.as_ref(), &self.id, request),
                None => Err(HostError::new(
                    HostErrorCode::Unsupported,
                    "This host service is not available.",
                )),
            },
        }
    }
}

impl HostBridge {
    /// Current values, synced ones read from their record each time so a
    /// change on another device applies without a reload.
    fn preferences(&self) -> Option<BTreeMap<String, Value>> {
        let fields = &self.access.preference_fields;
        if fields.is_empty() {
            return None;
        }
        let local = self
            .preferences
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        Some(crate::preferences::merged(
            self.shared.services().as_deref(),
            &self.id,
            fields,
            local,
        ))
    }
}

fn reply<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, HostError> {
    to_exact_vec(value, MAX_HOST_BYTES).map_err(|_| unavailable())
}

/// Reads only the operation name, borrowed from the request.
pub(crate) fn is_state_operation(input: &[u8]) -> bool {
    #[derive(serde::Deserialize)]
    struct Operation<'a> {
        #[serde(borrow)]
        operation: std::borrow::Cow<'a, str>,
    }
    serde_json::from_slice::<Operation>(input).is_ok_and(|request| {
        matches!(
            &*request.operation,
            "read_state" | "write_state" | "clear_state"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn teardown_admits_only_state_operations() {
        assert!(is_state_operation(
            br#"{"operation":"write_state","values":{"k":"v"}}"#
        ));
        assert!(!is_state_operation(
            br#"{"operation":"scan","since_ms":0,"after_item_id":null}"#
        ));
        assert!(!is_state_operation(br#"{"keys":[]}"#));
        assert!(!is_state_operation(b"not json"));
    }

    #[test]
    fn schema_6_requests_are_gated_by_granted_capabilities() {
        let bridge = |capabilities| HostBridge {
            id: "copypaste.ai-tools".into(),
            owner: 1,
            access: HostAccess {
                sync: None,
                sync_provider: false,
                capabilities,
                preference_fields: Vec::new(),
            },
            shared: Arc::default(),
            accepting: Arc::new(AtomicBool::new(true)),
            lifetime: Arc::default(),
            endpoints: Arc::default(),
            preferences: Arc::default(),
        };
        let code =
            |bridge: &HostBridge, input: &[u8]| bridge.handle(input).unwrap_err().unwrap().code;
        let none = bridge(Vec::new());
        assert_eq!(
            none.handle(br#"{"operation":"context"}"#).unwrap(),
            br#"{"locale":"en"}"#
        );
        assert_eq!(
            code(&none, br#"{"operation":"device_names"}"#),
            HostErrorCode::Denied
        );
        assert_eq!(
            code(
                &none,
                br#"{"operation":"scan","since_ms":0,"after_item_id":null}"#
            ),
            HostErrorCode::Unsupported
        );
        let granted = bridge(vec![
            ModuleCapability::HistoryRead,
            ModuleCapability::Endpoints,
        ]);
        assert_eq!(
            code(&granted, br#"{"operation":"device_names"}"#),
            HostErrorCode::Unsupported,
            "no application services are attached"
        );
        assert_eq!(
            code(
                &granted,
                br#"{"operation":"endpoint_send","connection":"c","data":"x"}"#
            ),
            HostErrorCode::NotFound
        );
        granted.accepting.store(false, Ordering::Release);
        assert_eq!(granted.handle(br#"{"operation":"context"}"#), Err(None));
    }

    #[test]
    fn context_carries_current_preferences_with_synced_values_from_their_record() {
        struct Synced;
        impl ModuleHostServices for Synced {
            fn record_get(
                &self,
                module_id: &str,
                key: &str,
            ) -> Result<Option<copypaste_module_sdk::ModuleRecord>, HostError> {
                Ok(Some(copypaste_module_sdk::ModuleRecord {
                    module_id: module_id.into(),
                    key: key.into(),
                    value_json: r#"{"window":["all"],"local":false}"#.into(),
                    deleted: false,
                }))
            }
        }
        let shared = Arc::new(HostShared::default());
        shared.set_services(Arc::new(Synced));
        let fields = serde_json::from_value(serde_json::json!([
            {"id": "window", "title": "Window", "kind": "choices", "synced": true,
             "options": [{"id": "week", "title": "Week"}, {"id": "all", "title": "All"}],
             "default": ["week"]},
            {"id": "local", "title": "Local", "kind": "boolean", "default": false}
        ]))
        .unwrap();
        let bridge = HostBridge {
            id: "copypaste.ai-tools".into(),
            owner: 1,
            access: HostAccess {
                sync: None,
                sync_provider: false,
                capabilities: Vec::new(),
                preference_fields: fields,
            },
            shared,
            accepting: Arc::new(AtomicBool::new(true)),
            lifetime: Arc::default(),
            endpoints: Arc::default(),
            preferences: Arc::new(RwLock::new(BTreeMap::from([
                ("window".into(), serde_json::json!(["week"])),
                ("local".into(), serde_json::json!(true)),
            ]))),
        };
        let reply: serde_json::Value =
            serde_json::from_slice(&bridge.handle(br#"{"operation":"context"}"#).unwrap()).unwrap();
        assert_eq!(
            reply["preferences"],
            serde_json::json!({"window": ["all"], "local": true}),
            "only synced fields come from the record"
        );
    }
}

//! Existing account IPC verbs dispatch through the optional signed provider.
use crate::ModuleHost;
use copypaste_ipc::{CloudStatusData, ErrorCode, Method, Response, ResponseData};
use copypaste_module_sdk::{ModuleOperation, ModuleOutput};
use std::{collections::BTreeMap, sync::Arc};
const MODULE_ID: &str = "copypaste.supabase";
impl ModuleHost {
    pub async fn cloud_request(self: &Arc<Self>, id: u64, method: Method) -> Response {
        let result = self.cloud_operation(method).await;
        match result {
            Ok(data) => Response::ok(id, data),
            Err(error) => Response::err(id, ErrorCode::InvalidRequest, error),
        }
    }
    async fn cloud_operation(self: &Arc<Self>, method: Method) -> Result<ResponseData, String> {
        if matches!(method, Method::CloudStatus | Method::CloudSignOut) {
            let available = if copypaste_module_sdk::ModuleTarget::current().is_none() {
                false
            } else {
                self.manager()
                    .map_err(|e| e.to_string())?
                    .list()
                    .map_err(|e| e.to_string())?
                    .iter()
                    .any(|module| {
                        module.id == MODULE_ID && module.enabled && module.error.is_none()
                    })
            };
            if !available {
                return Ok(ResponseData::CloudStatus(CloudStatusData {
                    configured: false,
                    signed_in: false,
                    key_ready: false,
                    email: None,
                    last_sync_ms: None,
                    last_error: None,
                    poll_interval_secs: 0,
                    unreadable_uploads: 0,
                }));
            }
        }
        let (command, arguments) = match method {
            Method::CloudSignIn {
                email,
                password,
                passphrase,
            } => (
                "sign-in",
                BTreeMap::from([
                    ("email", email),
                    ("password", password),
                    ("passphrase", passphrase),
                ]),
            ),
            Method::CloudSignUp {
                email,
                password,
                passphrase,
            } => (
                "sign-up",
                BTreeMap::from([
                    ("email", email),
                    ("password", password),
                    ("passphrase", passphrase),
                ]),
            ),
            Method::CloudSignOut => ("sign-out", BTreeMap::new()),
            Method::CloudSyncNow => ("sync-now", BTreeMap::new()),
            Method::CloudStatus => ("status", BTreeMap::new()),
            Method::CloudSetEndpoint { url, anon_key } => {
                self.request(ModuleOperation::SetPreferences {
                    id: MODULE_ID.into(),
                    values_json: serde_json::json!({"url":url,"anon_key":anon_key}).to_string(),
                })
                .await
                .map_err(|e| e.to_string())?;
                ("status", BTreeMap::new())
            }
            _ => return Err("Invalid sync provider request.".into()),
        };
        let json = self
            .request(ModuleOperation::Invoke {
                id: MODULE_ID.into(),
                command: command.into(),
                arguments_json: serde_json::to_string(&arguments)
                    .map_err(|_| "Invalid account fields.")?,
            })
            .await
            .map_err(|e| e.to_string())?;
        let ModuleOutput::Data { data } =
            serde_json::from_str(&json).map_err(|_| "Invalid sync provider result.")?
        else {
            return Err("Invalid sync provider result.".into());
        };
        if command == "sync-now" {
            Ok(ResponseData::CloudSync(
                serde_json::from_value(data.get("sync").cloned().ok_or("Invalid sync result.")?)
                    .map_err(|_| "Invalid sync result.")?,
            ))
        } else {
            Ok(ResponseData::CloudStatus(
                serde_json::from_value(
                    data.get("status")
                        .cloned()
                        .ok_or("Invalid account status.")?,
                )
                .map_err(|_| "Invalid account status.")?,
            ))
        }
    }
}

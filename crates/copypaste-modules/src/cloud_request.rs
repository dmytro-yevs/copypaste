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
        if matches!(method, Method::CloudStatus) && !self.cloud_provider_available()? {
            return Ok(signed_out_status());
        }
        if matches!(method, Method::CloudSignOut) {
            // A disabled or damaged provider cannot receive the command, but
            // its credentials are host-owned encrypted state and must still be
            // forgotten. The normal provider path remains responsible for its
            // best-effort remote revocation when it is available.
            if !self.cloud_provider_available().unwrap_or(false) {
                self.stop_cloud_provider(MODULE_ID)?;
                self.clear_cloud_credentials(MODULE_ID)
                    .map_err(|error| error.to_string())?;
                return Ok(signed_out_status());
            }
            return self.invoke_cloud_provider(method).await;
        }
        self.invoke_cloud_provider(method).await
    }

    fn cloud_provider_available(&self) -> Result<bool, String> {
        if copypaste_module_sdk::ModuleTarget::current().is_none() {
            return Ok(false);
        }
        Ok(self
            .manager()
            .map_err(|error| error.to_string())?
            .list()
            .map_err(|error| error.to_string())?
            .iter()
            .any(|module| module.id == MODULE_ID && module.enabled && module.error.is_none()))
    }

    async fn invoke_cloud_provider(
        self: &Arc<Self>,
        method: Method,
    ) -> Result<ResponseData, String> {
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

fn signed_out_status() -> ResponseData {
    ResponseData::CloudStatus(CloudStatusData {
        configured: false,
        signed_in: false,
        key_ready: false,
        email: None,
        last_sync_ms: None,
        last_error: None,
        poll_interval_secs: 0,
        unreadable_uploads: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SyncServices;
    use copypaste_core::{Keyring, Store, StoreSource};
    use copypaste_sync::account_state::{CloudStateKey, SIGN_OUT_KEYS};
    use copypaste_sync::store::StoreView;
    use std::sync::Arc;

    struct Fixture {
        _directory: tempfile::TempDir,
        host: Arc<ModuleHost>,
        store: Store,
    }

    fn fixture() -> Fixture {
        let directory = tempfile::tempdir().unwrap();
        let keyring = Arc::new(Keyring::from_secret(&[7; 32]));
        let store = Store::open_in_memory(&keyring.db_key()).unwrap();
        let source = StoreSource::new(
            store.clone(),
            keyring,
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
        host.set_sync_services(services).unwrap();
        Fixture {
            _directory: directory,
            host,
            store,
        }
    }

    fn state_key(key: CloudStateKey) -> String {
        format!("module:{MODULE_ID}:{}", key.as_str())
    }

    #[tokio::test]
    async fn unavailable_provider_sign_out_forgets_credentials_before_it_can_recover() {
        let fixture = fixture();
        for key in SIGN_OUT_KEYS {
            fixture.store.set_state(&state_key(*key), "stored").unwrap();
        }
        fixture
            .store
            .set_state(&state_key(CloudStateKey::CursorUserId), "account-a")
            .unwrap();

        let response = fixture.host.cloud_request(1, Method::CloudSignOut).await;

        assert!(response.ok);
        let Some(ResponseData::CloudStatus(status)) = response.data else {
            panic!("sign-out returns an account status");
        };
        assert!(!status.signed_in);
        assert!(!status.key_ready);
        for key in SIGN_OUT_KEYS {
            assert_eq!(fixture.store.state(&state_key(*key)).unwrap(), None);
        }
        // Sign-out deliberately keeps the cursor binding so a later sign-in to
        // the same account keeps its existing pagination boundary.
        assert_eq!(
            fixture
                .store
                .state(&state_key(CloudStateKey::CursorUserId))
                .unwrap()
                .as_deref(),
            Some("account-a")
        );
    }

    #[tokio::test]
    async fn unavailable_provider_sign_out_is_idempotent() {
        let fixture = fixture();

        for request_id in [1, 2] {
            let response = fixture
                .host
                .cloud_request(request_id, Method::CloudSignOut)
                .await;
            assert!(response.ok);
            assert!(matches!(response.data, Some(ResponseData::CloudStatus(_))));
        }
        for key in SIGN_OUT_KEYS {
            assert_eq!(fixture.store.state(&state_key(*key)).unwrap(), None);
        }
    }
}

//! Optional Supabase account and encrypted background synchronization.
mod host;
mod realtime;
use copypaste_cloud::{
    credentials::{AccountChange, CloudStateKey, CredentialState, CredentialStore},
    derive_sync_key, CloudConfig, CloudSync, SupabaseAuth, SupabaseRest,
};
use copypaste_ipc::{CloudStatusData, CloudSyncData};
use copypaste_module_sdk::{
    native::HostClient, Module, ModuleEnvironment, ModuleInvocation, ModuleOutput,
};
use copypaste_sync::{unreadable::UnreadableUploads, SyncError, SyncStats};
use host::HostStore;
use std::{collections::BTreeMap, sync::Arc, thread::JoinHandle, time::Duration};
use tokio::sync::{mpsc, Notify};
use tokio_util::sync::CancellationToken;
use zeroize::Zeroizing;

type Driver = CloudSync<SupabaseRest, SupabaseAuth>;
struct Command {
    invocation: ModuleInvocation,
    reply: std::sync::mpsc::Sender<Result<ModuleOutput, String>>,
}
pub struct SupabaseModule {
    commands: mpsc::UnboundedSender<Command>,
    cancel: CancellationToken,
    worker: Option<JoinHandle<()>>,
}
impl Module for SupabaseModule {
    fn create(_: ModuleEnvironment) -> Result<Self, String> {
        Err("Supabase requires the application sync host.".into())
    }
    fn create_with_host(_: ModuleEnvironment, host: HostClient) -> Result<Self, String> {
        let (commands, receiver) = mpsc::unbounded_channel();
        let cancel = CancellationToken::new();
        let worker_cancel = cancel.clone();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|_| "The sync worker could not start.")?;
        let worker = std::thread::Builder::new().name("supabase-module".into()).spawn(move || {
            runtime.block_on(async move {
                let monitor_host = host.clone();
                let monitor_cancel = worker_cancel.clone();
                let monitor = tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            biased;
                            _ = monitor_cancel.cancelled() => return,
                            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                                if monitor_host.request::<_, ()>(&copypaste_sync::host::SyncHostRequest::Active).is_err() {
                                    monitor_cancel.cancel();
                                    return;
                                }
                            }
                        }
                    }
                });
                run(HostStore(host), receiver, worker_cancel.clone()).await;
                worker_cancel.cancel();
                let _ = monitor.await;
            });
        }).map_err(|_| "The sync worker could not start.")?;
        Ok(Self {
            commands,
            cancel,
            worker: Some(worker),
        })
    }
    fn invoke(&mut self, invocation: ModuleInvocation) -> Result<ModuleOutput, String> {
        let (reply, receiver) = std::sync::mpsc::channel();
        self.commands
            .send(Command { invocation, reply })
            .map_err(|_| "The sync module has stopped.")?;
        receiver
            .recv()
            .map_err(|_| "The sync module has stopped.")?
    }
}
impl Drop for SupabaseModule {
    fn drop(&mut self) {
        self.cancel.cancel();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
copypaste_module_sdk::export_host_module!(SupabaseModule);

struct Account {
    driver: Arc<Driver>,
    email: String,
    cancel: CancellationToken,
    realtime: Option<tokio::task::JoinHandle<()>>,
}
struct State {
    host: HostStore,
    config: Option<CloudConfig>,
    account: Option<Account>,
    endpoint: Option<(String, String)>,
    last_error: Option<String>,
    wake: Arc<Notify>,
    revision: Option<u64>,
}
impl State {
    async fn stop_account(&mut self) {
        if let Some(account) = self.account.take() {
            account.driver.fence_session(None);
            account.cancel.cancel();
            if let Some(realtime) = account.realtime {
                let _ = realtime.await;
            }
        }
    }
    async fn configure(
        &mut self,
        preferences: &BTreeMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        let url = preferences
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        let key = preferences
            .get("anon_key")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        let endpoint = (url.to_string(), key.to_string());
        if self.endpoint.as_ref() == Some(&endpoint) {
            return Ok(());
        }
        let config = if url.is_empty() && key.is_empty() {
            None
        } else {
            if url.is_empty() || key.is_empty() {
                return Err("Set both the Supabase project URL and publishable key.".into());
            }
            Some(configuration(url, key)?)
        };
        self.stop_account().await;
        let persisted_url = self
            .host
            .state(CloudStateKey::EndpointUrl.as_str())
            .map_err(state_error)?;
        let persisted_key = self
            .host
            .state(CloudStateKey::EndpointAnonKey.as_str())
            .map_err(state_error)?;
        if persisted_url.as_deref() != Some(url) || persisted_key.as_deref() != Some(key) {
            self.host.clear_cloud_credentials().map_err(state_error)?;
            self.host
                .set_state_all(&[
                    (CloudStateKey::EndpointUrl.as_str(), url),
                    (CloudStateKey::EndpointAnonKey.as_str(), key),
                ])
                .map_err(state_error)?;
        }
        self.config = config;
        self.endpoint = Some(endpoint);
        if let (Some(config), Some(credentials)) = (
            self.config.clone(),
            self.host.cloud_credentials().map_err(state_error)?,
        ) {
            self.start_account(
                config,
                credentials.email,
                credentials.session,
                credentials.sync_key,
            );
        }
        Ok(())
    }
    fn start_account(
        &mut self,
        config: CloudConfig,
        email: String,
        session: copypaste_cloud::Session,
        key: copypaste_cloud::SyncKey,
    ) {
        let persistence = self.host.clone();
        let driver = Arc::new(
            CloudSync::new(
                SupabaseRest::new(config.clone()),
                SupabaseAuth::new(config.clone()),
                key,
                config,
                session,
            )
            .with_session_persistence(move |session| {
                persistence
                    .update_cloud_session(session)
                    .map_err(|_| SyncError::Source("encrypted session storage unavailable"))
            }),
        );
        let cancel = CancellationToken::new();
        let realtime = self.host.enabled().then(|| {
            tokio::spawn(realtime::run(
                Arc::clone(&driver),
                Arc::clone(&self.wake),
                cancel.clone(),
            ))
        });
        self.account = Some(Account {
            driver,
            email,
            cancel,
            realtime,
        });
        self.last_error = None;
        self.wake.notify_one();
    }
    fn next_delay(&self) -> Duration {
        self.account
            .as_ref()
            .map_or(Duration::from_secs(60), |account| {
                let until_refresh = account.driver.inspect_session(|session| {
                    session
                        .expires_at_ms
                        .saturating_sub(now_ms())
                        .saturating_sub(copypaste_cloud::auth::REFRESH_MARGIN_MS)
                        .max(5000)
                });
                account
                    .driver
                    .poll_interval()
                    .min(Duration::from_millis(until_refresh as u64))
            })
    }
    fn status(&self) -> CloudStatusData {
        CloudStatusData {
            configured: self.config.is_some(),
            signed_in: self.account.is_some(),
            key_ready: self.account.is_some(),
            email: self.account.as_ref().map(|a| a.email.clone()),
            last_sync_ms: self
                .host
                .state(CloudStateKey::LastSyncMs.as_str())
                .ok()
                .flatten()
                .and_then(|v| v.parse().ok()),
            last_error: self.last_error.clone(),
            poll_interval_secs: self
                .account
                .as_ref()
                .map_or(0, |a| a.driver.poll_interval().as_secs()),
            unreadable_uploads: UnreadableUploads::decode(
                self.host
                    .state(CloudStateKey::UnreadableUploads.as_str())
                    .ok()
                    .flatten()
                    .as_deref(),
            )
            .total,
        }
    }
    fn status_output(&self, message: &str) -> ModuleOutput {
        ModuleOutput::Data {
            data: serde_json::json!({"status": self.status(), "message": message}),
        }
    }
    async fn command(&mut self, invocation: ModuleInvocation) -> Result<ModuleOutput, String> {
        self.configure(&invocation.preferences).await?;
        match invocation.command.as_str() {
            "provider-tick" => {
                if self.host.enabled() {
                    if let Some(account) = self.account.as_mut() {
                        if account.realtime.is_none() {
                            account.realtime = Some(tokio::spawn(realtime::run(
                                Arc::clone(&account.driver),
                                Arc::clone(&self.wake),
                                account.cancel.clone(),
                            )));
                        }
                    }
                    let revision = self
                        .host
                        .0
                        .request(&copypaste_sync::host::SyncHostRequest::Revision)
                        .map_err(state_error)?;
                    if self.revision != Some(revision) {
                        self.revision = Some(revision);
                        if let Some(account) = &self.account {
                            account.driver.wake();
                        }
                        self.wake.notify_one();
                    }
                }
                Ok(ModuleOutput::Message {
                    message: "Sync scheduled.".into(),
                })
            }
            "status" => {
                let mut message = self.account.as_ref().map_or_else(
                    || "Signed out.".to_string(),
                    |a| format!("Signed in as {}.", a.email),
                );
                if !self.host.enabled() {
                    message.push_str(" Synchronization is disabled.");
                }
                if let Some(error) = &self.last_error {
                    message.push(' ');
                    message.push_str(error);
                }
                let unreadable = self.status().unreadable_uploads;
                if unreadable > 0 {
                    message.push_str(&format!(
                        " {unreadable} local items could not be read for upload."
                    ));
                }
                Ok(self.status_output(&message))
            }
            "sign-in" | "sign-up" => {
                let config = self
                    .config
                    .clone()
                    .ok_or("Configure the Supabase project first.")?;
                let email = argument(&invocation, "email")?;
                let password = Zeroizing::new(argument(&invocation, "password")?);
                let passphrase = Zeroizing::new(argument(&invocation, "passphrase")?);
                if passphrase.chars().count() < copypaste_cloud::crypto::key::MIN_PASSPHRASE_CHARS {
                    return Err("Use a sync passphrase of at least 12 characters.".into());
                }
                let auth = SupabaseAuth::new(config.clone());
                let session = if invocation.command == "sign-up" {
                    auth.sign_up(&email, &password).await
                } else {
                    auth.sign_in(&email, &password).await
                }
                .map_err(auth_error)?;
                let key = derive_sync_key(&passphrase, &session.user_id)
                    .map_err(|_| "The sync key could not be derived.")?;
                let owner = self
                    .host
                    .state(CloudStateKey::CursorUserId.as_str())
                    .map_err(state_error)?;
                let change = if owner.as_deref() == Some(session.user_id.as_str()) {
                    AccountChange::SameAccount
                } else {
                    AccountChange::SwitchedAccount
                };
                self.stop_account().await;
                self.host
                    .replace_cloud_credentials(&email, &session, &key.to_bytes(), change)
                    .map_err(state_error)?;
                self.start_account(config, email, session, key);
                Ok(self.status_output("Signed in."))
            }
            "sign-out" => {
                let token = self.account.as_ref().map(|a| {
                    a.driver
                        .inspect_session(|s| Zeroizing::new(s.access_token.clone()))
                });
                self.stop_account().await;
                self.host.clear_cloud_credentials().map_err(state_error)?;
                if let (Some(config), Some(token)) = (self.config.clone(), token) {
                    let _ = SupabaseAuth::new(config).sign_out(&token).await;
                }
                self.last_error = None;
                Ok(self.status_output("Signed out."))
            }
            "sync-now" => {
                let stats = self.sync_round().await?;
                let mut message = format!(
                    "Uploaded: {}. Downloaded: {}. Applied: {}.",
                    stats.uploaded, stats.downloaded, stats.applied
                );
                if stats.skipped_undecryptable + stats.skipped_forged > 0 {
                    message.push_str(" Some remote items could not be verified or decrypted. Check the sync passphrase.");
                }
                if stats.skipped_future > 0 {
                    message.push_str(&format!(
                        " {} remote items have an invalid timestamp.",
                        stats.skipped_future
                    ));
                }
                if stats.skipped_too_large > 0 {
                    message.push_str(&format!(
                        " {} local items exceed the upload limit.",
                        stats.skipped_too_large
                    ));
                }
                Ok(ModuleOutput::Data {
                    data: serde_json::json!({"sync": stats, "message": message}),
                })
            }
            _ => Err("Unknown Supabase command.".into()),
        }
    }
    async fn sync_round(&mut self) -> Result<CloudSyncData, String> {
        if !self.host.enabled() {
            return Err("Synchronization is disabled.".into());
        }
        let driver = Arc::clone(
            &self
                .account
                .as_ref()
                .ok_or("Sign in before syncing.")?
                .driver,
        );
        let started_ms = now_ms();
        let outcome = async {
            if driver.inspect_session(|s| s.needs_refresh(started_ms)) {
                driver.refresh_session().await?;
            }
            driver.sync(&self.host).await
        }
        .await;
        // A data request can rotate the token even when its later request fails.
        driver
            .inspect_session(|s| self.host.update_cloud_session(s))
            .map_err(state_error)?;
        match outcome {
            Ok(stats) => {
                self.host
                    .commit(started_ms)
                    .map_err(|_| "The upload cursor could not be saved.")?;
                self.host
                    .set_state_all(&[(CloudStateKey::LastSyncMs.as_str(), &now_ms().to_string())])
                    .map_err(state_error)?;
                self.last_error = None;
                Ok(to_wire(stats))
            }
            Err(error) => {
                let message = describe(&error);
                if matches!(
                    error,
                    SyncError::SessionExpired
                        | SyncError::InvalidCredentials
                        | SyncError::Unauthorized
                ) {
                    self.stop_account().await;
                    self.host.clear_cloud_credentials().map_err(state_error)?;
                }
                self.last_error = Some(message.into());
                Err(message.into())
            }
        }
    }
}
async fn run(
    host: HostStore,
    mut commands: mpsc::UnboundedReceiver<Command>,
    cancel: CancellationToken,
) {
    let mut state = State {
        host,
        config: None,
        account: None,
        endpoint: None,
        last_error: None,
        wake: Arc::new(Notify::new()),
        revision: None,
    };
    let mut next_round = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let wake = Arc::clone(&state.wake);
        tokio::select! {
            biased;
            _ = cancel.cancelled() => break,
            command = commands.recv() => {
                let Some(command) = command else { break; };
                let explicit_round = command.invocation.command == "sync-now";
                let result = tokio::select! { biased; _ = cancel.cancelled() => break, result = state.command(command.invocation) => result };
                if explicit_round { next_round = tokio::time::Instant::now() + state.next_delay(); }
                let _ = command.reply.send(result);
            }
            _ = wake.notified() => {
                if state.account.is_some() && state.host.enabled() {
                    tokio::select! { biased; _ = cancel.cancelled() => break, _ = state.sync_round() => {} }
                }
                next_round = tokio::time::Instant::now() + state.next_delay();
            }
            _ = tokio::time::sleep_until(next_round) => {
                if state.account.is_some() && state.host.enabled() {
                    tokio::select! { biased; _ = cancel.cancelled() => break, _ = state.sync_round() => {} }
                }
                next_round = tokio::time::Instant::now() + state.next_delay();
            }
        }
    }
    state.stop_account().await;
}
fn argument(invocation: &ModuleInvocation, id: &str) -> Result<String, String> {
    invocation
        .arguments
        .get(id)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| "Complete all account fields.".into())
}
fn configuration(url: &str, key: &str) -> Result<CloudConfig, String> {
    #[cfg(feature = "test-endpoints")]
    let config = CloudConfig::new_loopback(url, key);
    #[cfg(not(feature = "test-endpoints"))]
    let config = CloudConfig::new(url, key);
    config.map_err(|_| "The Supabase project URL must use HTTPS.".into())
}
fn now_ms() -> i64 {
    use copypaste_clock::WallClock;
    copypaste_clock::SystemWallClock.now_ms()
}
fn state_error<T>(_: T) -> String {
    "Encrypted account state is unavailable.".into()
}
fn auth_error(error: copypaste_cloud::AuthError) -> String {
    match error {
        copypaste_cloud::AuthError::InvalidCredentials => "The email or password is incorrect.",
        copypaste_cloud::AuthError::EmailConfirmationRequired => {
            "Confirm your email, then sign in."
        }
        copypaste_cloud::AuthError::SessionExpired => "The session expired. Sign in again.",
        copypaste_cloud::AuthError::RateLimited { .. } => {
            "The account service is rate limiting requests."
        }
        _ => "The Supabase account service could not complete the request.",
    }
    .into()
}
fn describe(error: &SyncError) -> &'static str {
    match error {
        SyncError::Source(_) => "The local history could not be read.",
        SyncError::Encrypt => "An item could not be encrypted for upload.",
        SyncError::Unauthorized => "The session was rejected. Sign in again.",
        SyncError::InvalidCredentials => "The stored credentials were rejected. Sign in again.",
        SyncError::SessionExpired => "The session expired. Sign in again.",
        SyncError::RateLimited => "The sync backend is rate limiting requests.",
        SyncError::Transport(_) => "The sync backend could not be reached.",
    }
}
fn to_wire(stats: SyncStats) -> CloudSyncData {
    let count = |n| u32::try_from(n).unwrap_or(u32::MAX);
    CloudSyncData {
        uploaded: count(stats.uploaded),
        tombstoned: count(stats.tombstoned),
        downloaded: count(stats.downloaded),
        applied: count(stats.applied),
        skipped_undecryptable: count(stats.skipped_undecryptable),
        skipped_forged: count(stats.skipped_forged),
        skipped_future: count(stats.skipped_future),
        skipped_too_large: count(stats.skipped_too_large),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn production_package_requires_secure_endpoints_and_host_capabilities() {
        #[cfg(not(feature = "test-endpoints"))]
        assert!(configuration("http://127.0.0.1:4000", "key").is_err());
        assert!(configuration("https://project.supabase.co", "publishable-key").is_ok());
        assert!(SupabaseModule::create(ModuleEnvironment {
            package_dir: "unused".into(),
            data_dir: "unused".into()
        })
        .is_err());
    }
}

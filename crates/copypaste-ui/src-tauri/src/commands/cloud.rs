use copypaste_ipc::{CloudStatusData, CloudSyncData};
use tauri::State;
use zeroize::Zeroizing;

use crate::backend::{Backend, BackendError, SelectedBackend};

type Result<T> = std::result::Result<T, BackendError>;

#[tauri::command]
pub async fn cloud_sign_in(
    backend: State<'_, SelectedBackend>,
    email: String,
    password: String,
    passphrase: String,
) -> Result<CloudStatusData> {
    let password = Zeroizing::new(password);
    let passphrase = Zeroizing::new(passphrase);
    backend
        .cloud_sign_in(email.trim(), &password, &passphrase)
        .await
}

#[tauri::command]
pub async fn cloud_sign_up(
    backend: State<'_, SelectedBackend>,
    email: String,
    password: String,
    passphrase: String,
) -> Result<CloudStatusData> {
    let password = Zeroizing::new(password);
    let passphrase = Zeroizing::new(passphrase);
    backend
        .cloud_sign_up(email.trim(), &password, &passphrase)
        .await
}

#[tauri::command]
pub async fn cloud_set_endpoint(
    backend: State<'_, SelectedBackend>,
    url: String,
    anon_key: String,
) -> Result<CloudStatusData> {
    backend.cloud_set_endpoint(&url, &anon_key).await
}

#[tauri::command]
pub async fn cloud_sign_out(backend: State<'_, SelectedBackend>) -> Result<CloudStatusData> {
    backend.cloud_sign_out().await
}

#[tauri::command]
pub async fn cloud_status(backend: State<'_, SelectedBackend>) -> Result<CloudStatusData> {
    backend.cloud_status().await
}

#[tauri::command]
pub async fn cloud_sync_now(backend: State<'_, SelectedBackend>) -> Result<CloudSyncData> {
    backend.cloud_sync().await
}

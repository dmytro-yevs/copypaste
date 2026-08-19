//! Onboarding permission commands. Platform work lives in `shell::permissions`.

use tauri::AppHandle;

use crate::backend::BackendError;
use crate::shell::permissions::{OnboardingPermissions, PermissionId};

type Result<T> = std::result::Result<T, BackendError>;

#[tauri::command]
pub async fn permission_snapshot(app: AppHandle) -> Result<OnboardingPermissions> {
    on_worker(move || crate::shell::permissions::snapshot(&app)).await
}

#[tauri::command]
pub async fn permission_request(app: AppHandle, id: PermissionId) -> Result<OnboardingPermissions> {
    on_worker(move || crate::shell::permissions::request(&app, id)).await
}

#[tauri::command]
pub async fn permission_open_settings(
    app: AppHandle,
    id: PermissionId,
) -> Result<OnboardingPermissions> {
    on_worker(move || crate::shell::permissions::open_settings(&app, id)).await
}

async fn on_worker<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|_| {
            BackendError::Internal("CopyPaste couldn't finish that permission request.".into())
        })?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_ids_are_the_snake_case_names_the_webview_sends() {
        assert_eq!(
            serde_json::from_str::<PermissionId>(r#""notifications""#).unwrap(),
            PermissionId::Notifications
        );
        assert_eq!(
            serde_json::from_str::<PermissionId>(r#""tile""#).unwrap(),
            PermissionId::Tile
        );
    }
}

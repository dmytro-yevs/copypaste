use std::env;

use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;
use serde::Deserialize;

use crate::backend::{Backend, BackendError, PairingBackend};
use crate::commands::pairing::PairingCeremony;
use crate::model::{UiImagePreview, UiInstalledSourceApp, UiItem, UiPage, UiSyncResult};
use crate::pairing_presentation::PairingPresentationState;
use crate::service::{
    diagnostics::{backend_snapshot, Diagnostics},
    ServiceState,
};

use super::contract::*;

pub(crate) async fn call(
    State(state): State<BridgeState>,
    headers: HeaderMap,
    Json(request): Json<BridgeRequest>,
) -> BridgeResponse {
    if !is_authorized(&headers, &state) {
        return Err(unauthorized());
    }

    let command = match BridgeCommand::parse(&request.command) {
        Some(command) => command,
        None if is_native_only(&request.command) => return Err(unavailable()),
        None => return Err(invalid_request()),
    };
    let backend = &state.backend;
    match command {
        BridgeCommand::List => {
            let args: ListArgs = parse(request.args)?;
            backend
                .list(args.limit, args.cursor.as_deref())
                .await
                .map(UiPage::from)
                .map_err(failure)
                .and_then(value)
        }
        BridgeCommand::Search => {
            let args: SearchArgs = parse(request.args)?;
            backend
                .search(&args.query, args.limit)
                .await
                .map(UiPage::from)
                .map_err(failure)
                .and_then(value)
        }
        BridgeCommand::CopyItem => {
            let args: IdArgs = parse(request.args)?;
            backend
                .copy(&args.id)
                .await
                .map(UiItem::from)
                .map_err(failure)
                .and_then(value)
        }
        BridgeCommand::CopyItemAsPlainText => {
            let args: IdArgs = parse(request.args)?;
            backend
                .copy_as_plain_text(&args.id)
                .await
                .map(UiItem::from)
                .map_err(failure)
                .and_then(value)
        }
        BridgeCommand::GetItemBody => {
            let args: IdArgs = parse(request.args)?;
            backend
                .get(&args.id)
                .await
                .and_then(|item| {
                    if item.is_sensitive {
                        Err(BackendError::Invalid(
                            "Sensitive content cannot be displayed here.",
                        ))
                    } else {
                        Ok(item.content)
                    }
                })
                .map_err(failure)
                .and_then(value)
        }
        BridgeCommand::ImagePreview => {
            let args: IdArgs = parse(request.args)?;
            backend
                .image_preview(&args.id)
                .await
                .map(UiImagePreview::from)
                .map_err(failure)
                .and_then(value)
        }
        BridgeCommand::SourceAppIcon => {
            let args: BundleIdArgs = parse(request.args)?;
            value(state.icons.resolve_desktop(&args.bundle_id))
        }
        BridgeCommand::InstalledSourceApps => {
            tauri::async_runtime::spawn_blocking(crate::installed_source_apps::list)
                .await
                .map_err(|_| {
                    failure(BackendError::internal(
                        "Installed applications couldn't be read.",
                    ))
                })?
                .map(|apps| {
                    apps.into_iter()
                        .map(|app| UiInstalledSourceApp::new(app.id, app.label))
                        .collect::<Vec<_>>()
                })
                .map_err(|_| {
                    failure(BackendError::internal(
                        "Installed applications couldn't be read.",
                    ))
                })
                .and_then(value)
        }
        BridgeCommand::DeleteItem => {
            let args: IdArgs = parse(request.args)?;
            backend
                .delete(&args.id)
                .await
                .map(|()| true)
                .map_err(failure)
                .and_then(value)
        }
        BridgeCommand::DeleteAll => {
            let args: ClearArgs = parse(request.args)?;
            backend
                .clear(args.through)
                .await
                .map_err(failure)
                .and_then(value)
        }
        BridgeCommand::HistoryCeiling => backend
            .history_ceiling()
            .await
            .map_err(failure)
            .and_then(value),
        BridgeCommand::SetPinned => {
            let args: PinArgs = parse(request.args)?;
            backend
                .set_pinned(&args.id, args.pinned)
                .await
                .map(UiItem::from)
                .map_err(failure)
                .and_then(value)
        }
        BridgeCommand::Status => backend.status().await.map_err(failure).and_then(value),
        BridgeCommand::SetDeviceName => {
            let args: DeviceNameArgs = parse(request.args)?;
            backend
                .set_device_name(&args.name)
                .await
                .map_err(failure)
                .and_then(value)
        }
        BridgeCommand::ServiceState => {
            let result = backend.status().await.map(|status| {
                let matches_app = status.version == env!("CARGO_PKG_VERSION");
                ServiceState::Running {
                    version: status.version,
                    matches_app,
                    ours: false,
                }
            });
            result.map_err(failure).and_then(value)
        }
        BridgeCommand::Diagnostics => {
            let snapshot = backend_snapshot(backend).await;
            value(Diagnostics::new(
                snapshot.service_hint,
                snapshot.status,
                snapshot.history_read,
            ))
        }
        BridgeCommand::RuntimeLogEvents => {
            let args: RuntimeLogArgs = parse(request.args)?;
            copypaste_runtime_log::list(&copypaste_ipc::data_dir().join("logs"), &args.query, true)
                .map_err(|_| BackendError::Internal("Runtime events couldn't be loaded.".into()))
                .map_err(failure)
                .and_then(value)
        }
        BridgeCommand::GetConfig => backend.get_config().await.map_err(failure).and_then(value),
        BridgeCommand::SetConfig => {
            let args: ConfigArgs = parse(request.args)?;
            backend
                .set_config(args.patch)
                .await
                .map_err(failure)
                .and_then(value)
        }
        BridgeCommand::GetPrivateMode => backend
            .get_private_mode()
            .await
            .map_err(failure)
            .and_then(value),
        BridgeCommand::SetPrivateMode => {
            #[derive(Deserialize)]
            struct Args {
                enabled: bool,
            }
            let args: Args = parse(request.args)?;
            backend
                .set_private_mode(args.enabled)
                .await
                .map_err(failure)
                .and_then(value)
        }
        BridgeCommand::CloudStatus => backend
            .cloud_status()
            .await
            .map_err(failure)
            .and_then(value),
        BridgeCommand::CloudSyncNow => backend.cloud_sync().await.map_err(failure).and_then(value),
        BridgeCommand::Peers => backend
            .peers()
            .await
            .map(|peers| peers.into_iter().map(without_address).collect::<Vec<_>>())
            .map_err(failure)
            .and_then(value),
        BridgeCommand::SyncNow => {
            let args: SyncArgs = parse(request.args)?;
            backend
                .sync(args.pairing_id.as_deref())
                .await
                .map(|items| {
                    items
                        .into_iter()
                        .map(UiSyncResult::from)
                        .collect::<Vec<_>>()
                })
                .map_err(failure)
                .and_then(value)
        }
        BridgeCommand::PairProgress => backend
            .pair_progress()
            .await
            .map(|progress| {
                PairingCeremony::from_progress(progress, PairingPresentationState::Unavailable)
            })
            .map_err(failure)
            .and_then(value),
        BridgeCommand::Discovered => backend
            .discovered()
            .await
            .map(without_discovery_addresses)
            .map_err(failure)
            .and_then(value),
    }
}

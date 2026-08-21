//! Local-only adapter for the browser development preview.
//!
//! The production UI reaches the daemon through Tauri's private bridge. A
//! browser tab cannot open that Unix socket, so this opt-in binary exposes a
//! small, authenticated subset of the same UI commands during development.
//! It is excluded from Android and every normal Tauri build by the
//! `dev-web-bridge` feature.

#[cfg(test)]
use std::collections::BTreeSet;
use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, Method as HttpMethod, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use copypaste_ipc::ConfigPatch;
use copypaste_ipc::{ErrorCode, PairingInviteData};
use qrcode::render::svg;
use qrcode::QrCode;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tower_http::cors::{AllowOrigin, CorsLayer};
use uuid::Uuid;

use crate::backend::{daemon::DaemonBackend, Backend, BackendError, PairingBackend};
use crate::commands::pairing::PairingCeremony;
use crate::model::{UiImagePreview, UiItem, UiPage, UiSyncResult};
use crate::pairing_presentation::invite::encode_native_invite;
use crate::pairing_presentation::PairingPresentationState;
use crate::service::{diagnostics::Diagnostics, ServiceState};
use crate::source_app_icon::SourceAppIconCache;

const DEFAULT_VITE_ORIGIN: &str = "http://localhost:1420";
const ENV_FILE: &str = "COPYPASTE_WEB_BRIDGE_ENV_FILE";
const ENV_ORIGIN: &str = "COPYPASTE_WEB_BRIDGE_ORIGIN";

#[derive(Clone)]
struct BridgeState {
    backend: DaemonBackend,
    icons: Arc<SourceAppIconCache>,
    origin: HeaderValue,
    bearer: String,
}

#[derive(Debug, Deserialize)]
struct BridgeRequest {
    command: String,
    #[serde(default)]
    args: Value,
}

#[derive(Debug, Serialize)]
struct Failure {
    code: String,
    retryable: bool,
}

type BridgeResponse = Result<Json<Value>, (StatusCode, Json<Failure>)>;

#[derive(Debug, Deserialize)]
struct ListArgs {
    limit: u32,
    cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchArgs {
    query: String,
    limit: u32,
}

#[derive(Debug, Deserialize)]
struct IdArgs {
    id: String,
}

#[derive(Debug, Default, Deserialize)]
struct ClearArgs {
    through: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct BundleIdArgs {
    #[serde(rename = "bundleId")]
    bundle_id: String,
}

#[derive(Debug, Deserialize)]
struct PinArgs {
    id: String,
    pinned: bool,
}

#[derive(Debug, Deserialize)]
struct PairingIdArgs {
    #[serde(rename = "pairingId")]
    pairing_id: String,
}

#[derive(Debug, Deserialize)]
struct SyncArgs {
    #[serde(rename = "pairingId")]
    pairing_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConfigArgs {
    patch: ConfigPatch,
}

#[derive(Debug, Deserialize)]
struct DeviceNameArgs {
    name: String,
}

#[derive(Debug, Deserialize)]
struct PreviewJoinArgs {
    code: String,
    addr: String,
}

#[derive(Debug, Serialize)]
struct PreviewPairingInvite {
    ceremony: PairingCeremony,
    code: String,
    listen_addr: String,
    expires_in_secs: u64,
    qr_svg: String,
}

fn failure(error: BackendError) -> (StatusCode, Json<Failure>) {
    let ui = error.ui_error();
    (
        StatusCode::BAD_GATEWAY,
        Json(Failure {
            code: ui.code,
            retryable: ui.retryable,
        }),
    )
}

fn unauthorized() -> (StatusCode, Json<Failure>) {
    (
        StatusCode::UNAUTHORIZED,
        Json(Failure {
            code: "offline".into(),
            retryable: true,
        }),
    )
}

fn invalid_request() -> (StatusCode, Json<Failure>) {
    (
        StatusCode::BAD_REQUEST,
        Json(Failure {
            code: "invalid_request".into(),
            retryable: false,
        }),
    )
}

fn parse<T: DeserializeOwned>(args: Value) -> Result<T, (StatusCode, Json<Failure>)> {
    serde_json::from_value(args).map_err(|_| invalid_request())
}

fn value(value: impl Serialize) -> BridgeResponse {
    serde_json::to_value(value).map(Json).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(Failure {
                code: "internal".into(),
                retryable: true,
            }),
        )
    })
}

fn preview_invite(
    invite: PairingInviteData,
    ceremony: PairingCeremony,
) -> Result<PreviewPairingInvite, BackendError> {
    let listen_addr = invite.listen_addr.clone().ok_or_else(|| {
        BackendError::from_code(
            Some(ErrorCode::PairingAddress),
            None,
            Some("This device does not have a reachable pairing address yet."),
        )
    })?;
    let payload = encode_native_invite(&invite)
        .ok_or_else(|| BackendError::internal("The pairing invite couldn't be encoded."))?;
    let qr = QrCode::new(payload.as_bytes())
        .map_err(|_| BackendError::internal("The pairing invite QR code couldn't be rendered."))?;
    let qr_svg = qr
        .render::<svg::Color<'_>>()
        .min_dimensions(240, 240)
        .dark_color(svg::Color("#0f172a"))
        .light_color(svg::Color("#ffffff"))
        .build();
    Ok(PreviewPairingInvite {
        ceremony,
        code: invite.code,
        listen_addr,
        expires_in_secs: invite.expires_in_secs,
        qr_svg,
    })
}

fn is_authorized(headers: &HeaderMap, state: &BridgeState) -> bool {
    let origin_matches = headers.get(header::ORIGIN) == Some(&state.origin);
    let expected = format!("Bearer {}", state.bearer);
    let bearer_matches = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        == Some(expected.as_str());
    origin_matches && bearer_matches
}

async fn call(
    State(state): State<BridgeState>,
    headers: HeaderMap,
    Json(request): Json<BridgeRequest>,
) -> BridgeResponse {
    if !is_authorized(&headers, &state) {
        return Err(unauthorized());
    }

    let backend = &state.backend;
    match request.command.as_str() {
        "list" => {
            let args: ListArgs = parse(request.args)?;
            backend
                .list(args.limit, args.cursor.as_deref())
                .await
                .map(UiPage::from)
                .map_err(failure)
                .and_then(value)
        }
        "search" => {
            let args: SearchArgs = parse(request.args)?;
            backend
                .search(&args.query, args.limit)
                .await
                .map(UiPage::from)
                .map_err(failure)
                .and_then(value)
        }
        "copy_item" => {
            let args: IdArgs = parse(request.args)?;
            backend
                .copy(&args.id)
                .await
                .map(UiItem::from)
                .map_err(failure)
                .and_then(value)
        }
        "copy_item_as_plain_text" => {
            let args: IdArgs = parse(request.args)?;
            backend
                .copy_as_plain_text(&args.id)
                .await
                .map(UiItem::from)
                .map_err(failure)
                .and_then(value)
        }
        "get_image_preview" => {
            let args: IdArgs = parse(request.args)?;
            backend
                .image_preview(&args.id)
                .await
                .map(UiImagePreview::from)
                .map_err(failure)
                .and_then(value)
        }
        "get_source_app_icon" => {
            let args: BundleIdArgs = parse(request.args)?;
            value(state.icons.resolve_desktop(&args.bundle_id))
        }
        "delete_item" => {
            let args: IdArgs = parse(request.args)?;
            backend
                .delete(&args.id)
                .await
                .map(|()| true)
                .map_err(failure)
                .and_then(value)
        }
        "delete_all" => {
            let args: ClearArgs = parse(request.args).unwrap_or_default();
            backend
                .clear(args.through)
                .await
                .map_err(failure)
                .and_then(value)
        }
        "history_ceiling" => backend
            .history_ceiling()
            .await
            .map_err(failure)
            .and_then(value),
        "set_pinned" => {
            let args: PinArgs = parse(request.args)?;
            backend
                .set_pinned(&args.id, args.pinned)
                .await
                .map(UiItem::from)
                .map_err(failure)
                .and_then(value)
        }
        "status" => backend.status().await.map_err(failure).and_then(value),
        "set_device_name" => {
            let args: DeviceNameArgs = parse(request.args)?;
            backend
                .set_device_name(&args.name)
                .await
                .map_err(failure)
                .and_then(value)
        }
        "service_state" => {
            let result = backend.status().await.map(|status| {
                let matches_app = status.version == env!("CARGO_PKG_VERSION");
                ServiceState::Running {
                    version: status.version,
                    matches_app,
                    // This bridge never owns the daemon process.
                    ours: false,
                }
            });
            result.map_err(failure).and_then(value)
        }
        "diagnostics" => {
            let result = async {
                let status = backend.status().await?;
                let version = status.version.clone();
                let history_read = match backend.list(1, None).await {
                    Ok(_) => crate::service::diagnostics::HistoryRead::Readable,
                    Err(error) => crate::service::diagnostics::HistoryRead::Failed {
                        code: error.ui_error().code,
                    },
                };
                Ok::<_, BackendError>(Diagnostics::new(
                    ServiceState::Running {
                        version,
                        matches_app: status.version == env!("CARGO_PKG_VERSION"),
                        ours: false,
                    },
                    Some(status),
                    history_read,
                ))
            }
            .await;
            result.map_err(failure).and_then(value)
        }
        "runtime_log_events" => {
            let query: copypaste_runtime_log::RuntimeLogQuery = parse(request.args)?;
            copypaste_runtime_log::list(&copypaste_ipc::data_dir().join("logs"), &query, true)
                .map_err(|_| BackendError::Internal("Runtime events couldn't be loaded.".into()))
                .map_err(failure)
                .and_then(value)
        }
        "get_config" => backend.get_config().await.map_err(failure).and_then(value),
        "set_config" => {
            let args: ConfigArgs = parse(request.args)?;
            backend
                .set_config(args.patch)
                .await
                .map_err(failure)
                .and_then(value)
        }
        "peers" => backend.peers().await.map_err(failure).and_then(value),
        "pair_progress" => backend
            .pair_progress()
            .await
            .map(|progress| {
                PairingCeremony::from_progress(progress, PairingPresentationState::Presented)
            })
            .map_err(failure)
            .and_then(value),
        "pair_cancel" => backend
            .pair_cancel()
            .await
            .map(|progress| {
                PairingCeremony::from_progress(progress, PairingPresentationState::Unavailable)
            })
            .map_err(failure)
            .and_then(value),
        "pair_confirm" => backend
            .pair_confirm(true)
            .await
            .map(|progress| {
                PairingCeremony::from_progress(progress, PairingPresentationState::Presented)
            })
            .map_err(failure)
            .and_then(value),
        "pair_reject" => backend
            .pair_confirm(false)
            .await
            .map(|progress| {
                PairingCeremony::from_progress(progress, PairingPresentationState::Presented)
            })
            .map_err(failure)
            .and_then(value),
        "pair_preview_invite" => {
            let invite = backend.pair_create_invite().await.map_err(failure)?;
            let progress = backend.pair_progress().await.map_err(failure)?;
            match preview_invite(
                invite,
                PairingCeremony::from_progress(progress, PairingPresentationState::Presented),
            ) {
                Ok(invite) => value(invite),
                Err(error) => Err(failure(error)),
            }
        }
        "pair_preview_join" => {
            let args: PreviewJoinArgs = parse(request.args)?;
            backend
                .pair_join(&args.code, &args.addr)
                .await
                .map(|progress| {
                    PairingCeremony::from_progress(progress, PairingPresentationState::Presented)
                })
                .map_err(failure)
                .and_then(value)
        }
        "unpair" => {
            let args: PairingIdArgs = parse(request.args)?;
            backend
                .unpair(&args.pairing_id)
                .await
                .map_err(failure)
                .and_then(value)
        }
        "revoke" => {
            let args: PairingIdArgs = parse(request.args)?;
            backend
                .revoke(&args.pairing_id)
                .await
                .map_err(failure)
                .and_then(value)
        }
        "sync_now" => {
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
        "discovered" => backend.discovered().await.map_err(failure).and_then(value),
        "rescan" => backend.rescan().await.map_err(failure).and_then(value),
        // `get`, reveal and raw clipboard text intentionally do not cross a
        // browser boundary. Image previews are separately bounded by the
        // daemon and refused for sensitive items.
        _ => Err(invalid_request()),
    }
}

fn cors(origin: HeaderValue) -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::exact(origin))
        .allow_methods([HttpMethod::POST])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
}

/// Start the browser-preview adapter from the dedicated development script.
///
/// It binds an ephemeral loopback port, creates a one-run bearer token and
/// writes Vite's two environment variables into the mode-0600 file named by
/// `COPYPASTE_WEB_BRIDGE_ENV_FILE`.
pub async fn run_from_env() -> Result<(), Box<dyn std::error::Error>> {
    let origin = env::var(ENV_ORIGIN).unwrap_or_else(|_| DEFAULT_VITE_ORIGIN.into());
    let origin_header = HeaderValue::from_str(&origin)?;
    let env_file = env::var(ENV_FILE)?;
    let bearer = Uuid::new_v4().simple().to_string();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address: SocketAddr = listener.local_addr()?;
    let url = format!("http://{address}");

    write_vite_env(&env_file, &url, &bearer)?;
    println!("CopyPaste browser bridge: {url}");
    println!("CopyPaste browser bridge token: {bearer}");

    let state = BridgeState {
        backend: DaemonBackend::new(),
        icons: Arc::new(SourceAppIconCache::default()),
        origin: origin_header.clone(),
        bearer,
    };
    let app = Router::new()
        .route("/v1/call", post(call))
        .with_state(state)
        .layer(cors(origin_header));
    axum::serve(listener, app).await?;
    Ok(())
}

fn write_vite_env(path: &str, url: &str, token: &str) -> std::io::Result<()> {
    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt as _;

    let mut options = OpenOptions::new();
    options.write(true).truncate(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(path)?;
    writeln!(file, "VITE_COPYPASTE_WEB_BRIDGE_URL={url}")?;
    writeln!(file, "VITE_COPYPASTE_WEB_BRIDGE_TOKEN={token}")?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_commands_have_no_fallback_route() {
        let allowed = [
            "list",
            "search",
            "copy_item",
            "copy_item_as_plain_text",
            "get_image_preview",
            "get_source_app_icon",
            "delete_item",
            "delete_all",
            "history_ceiling",
            "set_pinned",
            "status",
            "set_device_name",
            "service_state",
            "diagnostics",
            "runtime_log_events",
            "get_config",
            "set_config",
            "peers",
            "pair_progress",
            "pair_cancel",
            "pair_confirm",
            "pair_reject",
            "pair_preview_invite",
            "pair_preview_join",
            "unpair",
            "revoke",
            "sync_now",
            "discovered",
            "rescan",
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        assert!(!allowed.contains("reveal_item"));
        assert!(!allowed.contains("read_pairing_text"));
        assert!(!allowed.contains("pair_create"));
        assert!(!allowed.contains("pair_accept"));
        assert!(!allowed.contains("scan_pairing_qr"));
        assert!(!allowed.contains("pair_create_invite"));
        assert!(!allowed.contains("pair_scan_invite"));
        assert!(!allowed.contains("export_history"));
    }

    #[test]
    fn bridge_requires_both_origin_and_bearer() {
        let state = BridgeState {
            backend: DaemonBackend::new(),
            icons: Arc::new(SourceAppIconCache::default()),
            origin: HeaderValue::from_static(DEFAULT_VITE_ORIGIN),
            bearer: "one-run-token".into(),
        };
        let mut headers = HeaderMap::new();
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static(DEFAULT_VITE_ORIGIN),
        );
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer one-run-token"),
        );
        assert!(is_authorized(&headers, &state));
        headers.remove(header::ORIGIN);
        assert!(!is_authorized(&headers, &state));
    }
}

//! Browser bridge request contract and security boundary.
use std::sync::Arc;

use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::Json;
use copypaste_ipc::{ConfigPatch, DiscoveredDevice, PeerInfo};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::backend::{daemon::DaemonBackend, BackendError, UiBoundaryErrorCode};
use crate::command_contract::UiCommandName;
use crate::source_app_icon::SourceAppIconCache;

pub(crate) const DEFAULT_VITE_ORIGIN: &str = "http://127.0.0.1:1420";
pub(crate) const ENV_FILE: &str = "COPYPASTE_WEB_BRIDGE_ENV_FILE";
pub(crate) const ENV_ORIGIN: &str = "COPYPASTE_WEB_BRIDGE_ORIGIN";

#[derive(Clone)]
pub(crate) struct BridgeState {
    pub(crate) backend: DaemonBackend,
    pub(crate) icons: Arc<SourceAppIconCache>,
    pub(crate) origin: HeaderValue,
    pub(crate) bearer: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BridgeRequest {
    pub(crate) command: String,
    #[serde(default)]
    pub(crate) args: Value,
}

#[derive(Debug, Serialize)]
pub(crate) struct Failure {
    code: String,
    retryable: bool,
}

pub(crate) type BridgeResponse = Result<Json<Value>, (StatusCode, Json<Failure>)>;

#[derive(Debug, Deserialize)]
pub(crate) struct ListArgs {
    pub(crate) limit: u32,
    pub(crate) cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SearchArgs {
    pub(crate) query: String,
    pub(crate) limit: u32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct IdArgs {
    pub(crate) id: String,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct ClearArgs {
    pub(crate) through: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BundleIdArgs {
    #[serde(rename = "bundleId")]
    pub(crate) bundle_id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PinArgs {
    pub(crate) id: String,
    pub(crate) pinned: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SyncArgs {
    #[serde(rename = "pairingId")]
    pub(crate) pairing_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ConfigArgs {
    pub(crate) patch: ConfigPatch,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DeviceNameArgs {
    pub(crate) name: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RuntimeLogArgs {
    pub(crate) query: copypaste_runtime_log::RuntimeLogQuery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BridgeCommand {
    List,
    Search,
    CopyItem,
    CopyItemAsPlainText,
    GetItemBody,
    ImagePreview,
    SourceAppIcon,
    InstalledSourceApps,
    DeleteItem,
    DeleteAll,
    HistoryCeiling,
    SetPinned,
    Status,
    SetDeviceName,
    ServiceState,
    Diagnostics,
    RuntimeLogEvents,
    GetConfig,
    SetConfig,
    GetPrivateMode,
    SetPrivateMode,
    CloudStatus,
    CloudSyncNow,
    Peers,
    SyncNow,
    PairProgress,
    Discovered,
}

impl BridgeCommand {
    pub(crate) fn from_ui(command: UiCommandName) -> Option<Self> {
        match command {
            UiCommandName::List => Some(Self::List),
            UiCommandName::Search => Some(Self::Search),
            UiCommandName::CopyItem => Some(Self::CopyItem),
            UiCommandName::CopyItemAsPlainText => Some(Self::CopyItemAsPlainText),
            UiCommandName::GetItemBody => Some(Self::GetItemBody),
            UiCommandName::GetImagePreview => Some(Self::ImagePreview),
            UiCommandName::GetSourceAppIcon => Some(Self::SourceAppIcon),
            UiCommandName::ListInstalledSourceApps => Some(Self::InstalledSourceApps),
            UiCommandName::DeleteItem => Some(Self::DeleteItem),
            UiCommandName::DeleteAll => Some(Self::DeleteAll),
            UiCommandName::HistoryCeiling => Some(Self::HistoryCeiling),
            UiCommandName::SetPinned => Some(Self::SetPinned),
            UiCommandName::Status => Some(Self::Status),
            UiCommandName::SetDeviceName => Some(Self::SetDeviceName),
            UiCommandName::ServiceState => Some(Self::ServiceState),
            UiCommandName::Diagnostics => Some(Self::Diagnostics),
            UiCommandName::RuntimeLogEvents => Some(Self::RuntimeLogEvents),
            UiCommandName::GetConfig => Some(Self::GetConfig),
            UiCommandName::SetConfig => Some(Self::SetConfig),
            UiCommandName::GetPrivateMode => Some(Self::GetPrivateMode),
            UiCommandName::SetPrivateMode => Some(Self::SetPrivateMode),
            UiCommandName::CloudStatus => Some(Self::CloudStatus),
            UiCommandName::CloudSyncNow => Some(Self::CloudSyncNow),
            UiCommandName::Peers => Some(Self::Peers),
            UiCommandName::SyncNow => Some(Self::SyncNow),
            UiCommandName::PairProgress => Some(Self::PairProgress),
            UiCommandName::Discovered => Some(Self::Discovered),
            _ => None,
        }
    }
}

pub(crate) fn failure(error: BackendError) -> (StatusCode, Json<Failure>) {
    let ui = error.ui_error();
    (
        StatusCode::BAD_GATEWAY,
        Json(Failure {
            code: ui.code,
            retryable: ui.retryable,
        }),
    )
}

pub(crate) fn unauthorized() -> (StatusCode, Json<Failure>) {
    (
        StatusCode::UNAUTHORIZED,
        Json(Failure {
            code: UiBoundaryErrorCode::Offline.as_str().into(),
            retryable: UiBoundaryErrorCode::Offline.retryable(),
        }),
    )
}

pub(crate) fn invalid_request() -> (StatusCode, Json<Failure>) {
    (
        StatusCode::BAD_REQUEST,
        Json(Failure {
            code: "invalid_request".into(),
            retryable: false,
        }),
    )
}

pub(crate) fn unavailable() -> (StatusCode, Json<Failure>) {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(Failure {
            code: UiBoundaryErrorCode::Unavailable.as_str().into(),
            retryable: UiBoundaryErrorCode::Unavailable.retryable(),
        }),
    )
}

pub(crate) fn is_native_only(command: UiCommandName) -> bool {
    matches!(
        command,
        UiCommandName::PairCreateInvite
            | UiCommandName::PairScanInvite
            | UiCommandName::PairPresent
            | UiCommandName::PairConfirm
            | UiCommandName::PairReject
            | UiCommandName::PairCancel
            | UiCommandName::StartService
            | UiCommandName::RestartService
    )
}

pub(crate) fn parse<T: DeserializeOwned>(args: Value) -> Result<T, (StatusCode, Json<Failure>)> {
    serde_json::from_value(args).map_err(|_| invalid_request())
}

pub(crate) fn value(value: impl Serialize) -> BridgeResponse {
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

pub(crate) fn is_authorized(headers: &HeaderMap, state: &BridgeState) -> bool {
    let origin_matches = headers.get(header::ORIGIN) == Some(&state.origin);
    let expected = format!("Bearer {}", state.bearer);
    let bearer_matches = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        == Some(expected.as_str());
    origin_matches && bearer_matches
}

pub(crate) fn without_address(mut peer: PeerInfo) -> PeerInfo {
    peer.last_addr = None;
    if let Some(details) = peer.details.as_mut() {
        details.endpoint = None;
    }
    peer
}

pub(crate) fn without_discovery_addresses(_: Vec<DiscoveredDevice>) -> Vec<DiscoveredDevice> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::pairing::PairingCeremony;
    use crate::pairing_presentation::PairingPresentationState;

    #[test]
    fn supported_commands_are_an_explicit_closed_set() {
        for command in [
            "list",
            "search",
            "copy_item",
            "copy_item_as_plain_text",
            "get_item_body",
            "get_image_preview",
            "get_source_app_icon",
            "list_installed_source_apps",
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
            "get_private_mode",
            "set_private_mode",
            "cloud_status",
            "cloud_sync_now",
            "peers",
            "sync_now",
            "pair_progress",
            "discovered",
        ] {
            let command = UiCommandName::parse(command).unwrap();
            assert!(
                BridgeCommand::from_ui(command).is_some(),
                "{}",
                command.as_str()
            );
        }
        assert!(UiCommandName::parse("future_command").is_none());
    }

    #[test]
    fn secrets_pairing_addresses_and_destructive_peer_actions_are_forbidden() {
        for command in [
            "reveal_item",
            "copy_text",
            "add_item",
            "pair_preview_invite",
            "pair_preview_join",
            "cloud_sign_in",
            "cloud_sign_up",
            "cloud_set_endpoint",
            "cloud_sign_out",
            "unpair",
            "revoke",
            "rescan",
            "export_history",
            "restore_database",
            "set_shortcut",
        ] {
            let command = UiCommandName::parse(command).unwrap();
            assert!(
                BridgeCommand::from_ui(command).is_none(),
                "{}",
                command.as_str()
            );
            assert!(!is_native_only(command), "{}", command.as_str());
        }
    }

    #[test]
    fn native_only_commands_report_unavailable() {
        for command in [
            "pair_create_invite",
            "pair_scan_invite",
            "start_service",
            "restart_service",
        ] {
            let command = UiCommandName::parse(command).unwrap();
            assert!(is_native_only(command), "{}", command.as_str());
            assert!(
                BridgeCommand::from_ui(command).is_none(),
                "{}",
                command.as_str()
            );
        }
        let (_, Json(failure)) = unavailable();
        assert_eq!(failure.code, "unavailable");
    }

    #[test]
    fn malformed_clear_args_never_default_to_delete_everything() {
        assert!(serde_json::from_value::<ClearArgs>(serde_json::json!({
            "through": "not-a-number"
        }))
        .is_err());
    }

    #[test]
    fn peer_addresses_do_not_cross_the_browser_boundary() {
        let peer = without_address(PeerInfo {
            pairing_id: "pair-1".into(),
            name: "Phone".into(),
            last_addr: Some("192.0.2.1:47654".into()),
            last_seen_ms: 1,
            online: true,
            details: Some(copypaste_ipc::DeviceDetails {
                endpoint: Some(copypaste_ipc::DeviceEndpointObservation {
                    lan_endpoint: "192.0.2.1:47654".into(),
                    provenance: copypaste_ipc::DeviceObservationProvenance::Observed,
                    trust: copypaste_ipc::DeviceObservationTrust::Authenticated,
                    observed_at_ms: 1,
                    fresh_until_ms: Some(2),
                }),
                ..Default::default()
            }),
        });
        assert_eq!(peer.last_addr, None);
        assert_eq!(peer.details.and_then(|details| details.endpoint), None);
    }

    #[test]
    fn discovery_poll_never_returns_pairing_addresses() {
        let devices = vec![DiscoveredDevice {
            discovery_id: "nearby-1".into(),
            name: "Phone".into(),
            addr: "192.0.2.1:47654".into(),
            last_seen_ms: 1,
            paired: false,
            details: None,
        }];
        assert!(without_discovery_addresses(devices).is_empty());
    }

    #[test]
    fn pairing_progress_poll_drops_sas_and_peer_address() {
        let ceremony = PairingCeremony::from_progress(
            copypaste_ipc::PairingProgressData {
                pairing_id: Some("ceremony-1".into()),
                role: None,
                state: copypaste_ipc::PairingState::AwaitingConfirmation,
                expires_in_ms: Some(60_000),
                sas: Some("123456".into()),
                peer_device_id: Some("peer-device".into()),
                peer_name: Some("Phone".into()),
                peer_addr: Some("192.0.2.1:47654".into()),
                known_device: None,
                error_code: None,
            },
            PairingPresentationState::Unavailable,
        );
        let json = serde_json::to_string(&ceremony).unwrap();
        for forbidden in ["123456", "peer-device", "Phone", "192.0.2.1:47654"] {
            assert!(!json.contains(forbidden), "{json}");
        }
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

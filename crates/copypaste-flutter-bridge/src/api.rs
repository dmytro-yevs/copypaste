//! Flutter-facing runtime API.
//!
//! DTOs deliberately contain only safe display data. Generated pairing
//! credentials and SAS values remain in protected platform adapters. The sole
//! secret input is a user-entered join code submitted from protected UI.

use std::io::Write;

use copypaste_ipc::{Method, ResponseData};

use crate::client;

async fn module_request(operation: copypaste_ipc::ModuleOperation) -> Result<String, RuntimeError> {
    let response = client::request(Method::Modules { operation }).await?;
    match response.data {
        Some(ResponseData::Modules { json }) => Ok(json),
        _ => Err(RuntimeError::internal()),
    }
}

pub async fn modules_list() -> Result<String, RuntimeError> {
    module_request(copypaste_ipc::ModuleOperation::List).await
}

pub async fn module_install(package_path: String) -> Result<String, RuntimeError> {
    module_request(copypaste_ipc::ModuleOperation::Install { package_path }).await
}

pub async fn module_set_enabled(id: String, enabled: bool) -> Result<(), RuntimeError> {
    module_request(copypaste_ipc::ModuleOperation::SetEnabled { id, enabled })
        .await
        .map(|_| ())
}

pub async fn module_set_preferences(id: String, values_json: String) -> Result<(), RuntimeError> {
    module_request(copypaste_ipc::ModuleOperation::SetPreferences { id, values_json })
        .await
        .map(|_| ())
}

pub async fn module_remove(id: String) -> Result<(), RuntimeError> {
    module_request(copypaste_ipc::ModuleOperation::Remove { id })
        .await
        .map(|_| ())
}

pub async fn module_invoke(
    id: String,
    command: String,
    arguments_json: String,
) -> Result<String, RuntimeError> {
    module_request(copypaste_ipc::ModuleOperation::Invoke {
        id,
        command,
        arguments_json,
    })
    .await
}

#[derive(Debug, Clone)]
pub struct RuntimeError {
    pub code: String,
    pub message: String,
}

impl RuntimeError {
    pub(crate) fn daemon_unreachable() -> Self {
        Self {
            code: "daemon_unreachable".into(),
            message: "CopyPaste is not running.".into(),
        }
    }

    pub(crate) fn timeout() -> Self {
        Self {
            code: "runtime_timeout".into(),
            message: "CopyPaste did not respond in time.".into(),
        }
    }

    pub(crate) fn internal() -> Self {
        Self {
            code: "runtime_internal".into(),
            message: "CopyPaste could not complete the request.".into(),
        }
    }

    fn export_write_failed() -> Self {
        Self {
            code: "export_write_failed".into(),
            message: "The text history export could not be saved.".into(),
        }
    }

    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub(crate) fn android_runtime_unavailable() -> Self {
        Self {
            code: "android_runtime_unavailable".into(),
            message: "The Android runtime is not available yet.".into(),
        }
    }

    pub(crate) fn not_initialized() -> Self {
        Self {
            code: "runtime_not_initialized".into(),
            message: "CopyPaste is starting.".into(),
        }
    }

    pub(crate) fn unsafe_data_directory() -> Self {
        Self {
            code: "unsafe_data_directory".into(),
            message: "CopyPaste runtime data directory is invalid.".into(),
        }
    }

    pub(crate) fn daemon_start_failed() -> Self {
        Self {
            code: "daemon_start_failed".into(),
            message: "CopyPaste could not start its runtime.".into(),
        }
    }

    pub(crate) fn daemon_spawn_failed() -> Self {
        Self {
            code: "daemon_spawn_failed".into(),
            message: "CopyPaste could not launch its runtime helper.".into(),
        }
    }

    pub(crate) fn daemon_exited_early() -> Self {
        Self {
            code: "daemon_exited_early".into(),
            message: "CopyPaste runtime helper exited during startup.".into(),
        }
    }

    pub(crate) fn ceremony_not_found() -> Self {
        Self {
            code: "pairing_ceremony_not_found".into(),
            message: "That pairing ceremony is no longer active.".into(),
        }
    }

    pub(crate) fn watch_not_found() -> Self {
        Self {
            code: "runtime_watch_not_found".into(),
            message: "That runtime watch is no longer active.".into(),
        }
    }

    pub(crate) fn from_daemon(code: String, message: String) -> Self {
        Self { code, message }
    }
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for RuntimeError {}

#[derive(Debug, Clone)]
pub struct Clip {
    pub id: String,
    pub content: String,
    pub content_type: String,
    pub content_class: ClipContentClass,
    pub semantic_kind: Option<ClipSemanticKind>,
    /// Packed as `0xRRGGBBAA` for color clips.
    pub color_rgba: Option<u32>,
    pub created_at_ms: i64,
    pub pinned: bool,
    pub origin_device_name: Option<String>,
    pub origin_device_class: DeviceClass,
    pub source_app_name: Option<String>,
    pub truncated: bool,
    pub too_large_to_sync: bool,
    pub file_details: Option<ClipFileDetails>,
    pub image_details: Option<ClipImageDetails>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipContentClass {
    Text,
    Image,
    File,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipSemanticKind {
    PlainText,
    Link,
    Email,
    Color,
    Phone,
    Code,
    Json,
    Path,
}

#[derive(Debug, Clone)]
pub struct ClipFileDetails {
    pub filename: Option<String>,
    pub mime_type: Option<String>,
    pub source_reference: Option<String>,
    pub source_available: bool,
    pub size_bytes: u64,
    pub file_count: u32,
}

#[derive(Debug, Clone)]
pub struct ClipImageDetails {
    pub width: u32,
    pub height: u32,
    pub size_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct ClipPage {
    pub clips: Vec<Clip>,
    pub next_cursor: Option<String>,
    pub skipped_undecryptable: u32,
}
#[derive(Debug, Clone)]
pub struct HistoryDeviceFacet {
    pub id: String,
    pub label: String,
    pub device_class: DeviceClass,
}

#[derive(Debug, Clone)]
pub struct HistorySourceAppFacet {
    pub id: String,
    pub label: String,
    pub icon_item_id: Option<String>,
}
#[derive(Debug, Clone)]
pub struct HistoryFacets {
    pub origin_devices: Vec<HistoryDeviceFacet>,
    pub source_apps: Vec<HistorySourceAppFacet>,
}

/// Rust-owned query DTO for the generated Flutter API.
///
/// It maps one-for-one to `copypaste_ipc::HistoryQuery`; no Dart model
/// reimplements filtering, ordering or cursor semantics.
#[derive(Debug, Clone)]
pub struct ClipQuery {
    pub search: Option<String>,
    pub content_classes: Vec<ClipContentClass>,
    pub semantic_kinds: Vec<ClipSemanticKind>,
    pub pinned_only: bool,
    pub origin_device_id: Option<String>,
    pub source_app_bundle_id: Option<String>,
    pub sort: ClipSort,
}

#[derive(Debug, Clone, Copy)]
pub enum ClipSort {
    Newest,
    Oldest,
    Relevance,
}

#[derive(Debug, Clone)]
pub struct ClipImagePreview {
    pub png_base64: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct ClipImagePreviewBounds {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct Peer {
    pub pairing_id: String,
    pub name: String,
    pub last_seen_ms: i64,
    pub online: bool,
    pub details: Option<DeviceDetails>,
}

#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    pub discovery_id: String,
    pub name: String,
    pub address: String,
    pub paired: bool,
    pub last_seen_ms: i64,
    pub details: Option<DeviceDetails>,
}

#[derive(Debug, Clone)]
pub struct SyncOutcome {
    pub pairing_id: String,
    pub name: String,
    pub sent: u32,
    pub received: u32,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ThisDevice {
    /// The stable local device identity, never a pairing id.
    pub device_id: Option<String>,
    pub name: String,
    pub app_version: String,
    pub protocol_version: u32,
    pub listen_address: Option<String>,
    pub details: Option<DeviceDetails>,
}

/// Display-safe observations of a device. This intentionally excludes pairing
/// material and any credential-derived values.
#[derive(Debug, Clone)]
pub struct DeviceDetails {
    pub profile: Option<DeviceProfile>,
    pub endpoint: Option<DeviceEndpoint>,
    pub latency: Option<DeviceLatency>,
    pub presence: Option<DevicePresence>,
}

#[derive(Debug, Clone)]
pub struct DeviceProfile {
    pub display_name: String,
    pub app_version: Option<String>,
    pub protocol_version: Option<u32>,
    pub platform: DevicePlatform,
    pub device_class: DeviceClass,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub model: Option<String>,
    pub provenance: DeviceObservationProvenance,
    pub trust: DeviceObservationTrust,
    pub observed_at_ms: i64,
    pub fresh_until_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct DeviceEndpoint {
    pub lan_endpoint: String,
    pub provenance: DeviceObservationProvenance,
    pub trust: DeviceObservationTrust,
    pub observed_at_ms: i64,
    pub fresh_until_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct DeviceLatency {
    /// Encrypted Probe-to-ProbeAck round-trip time after the Noise handshake.
    pub round_trip_latency_ms: u64,
    pub provenance: DeviceObservationProvenance,
    pub trust: DeviceObservationTrust,
    pub observed_at_ms: i64,
    pub fresh_until_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct DevicePresence {
    pub state: DevicePresenceState,
    pub last_seen_ms: i64,
    pub provenance: DeviceObservationProvenance,
    pub trust: DeviceObservationTrust,
    pub observed_at_ms: i64,
    pub fresh_until_ms: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevicePlatform {
    Macos,
    Windows,
    Android,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceClass {
    Desktop,
    Laptop,
    Phone,
    Tablet,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevicePresenceState {
    Online,
    Offline,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceObservationProvenance {
    SelfReported,
    Observed,
    Measured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceObservationTrust {
    Local,
    Unverified,
    Authenticated,
}

#[derive(Debug, Clone)]
pub struct RuntimeEvent {
    pub kind: String,
    pub item_count: u64,
    pub captured: bool,
    pub captured_item_id: Option<String>,
}

/// Safe pairing state for ordinary Flutter UI. It deliberately has no invite
/// code, QR payload, SAS, network address, or peer identity value.
#[derive(Debug, Clone)]
pub struct PairingCeremony {
    pub ceremony_id: String,
    pub state: String,
    pub expires_in_ms: Option<u64>,
    pub peer_name: Option<String>,
    pub failure_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CaptureState {
    pub running: bool,
    pub paused: bool,
    pub private_mode_epoch: u64,
}

#[derive(Debug, Clone)]
pub struct RuntimeSettingsData {
    pub retention_days: u32,
    pub storage_quota_bytes: u64,
    pub excluded_app_ids: Vec<String>,
    pub lan_visibility: bool,
    pub sync_enabled: bool,
    pub notify_on_copy: bool,
    pub notification_preview: bool,
    pub sound_on_copy: bool,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeSettingsPatch {
    pub retention_days: Option<u32>,
    pub storage_quota_bytes: Option<u64>,
    pub excluded_app_ids: Option<Vec<String>>,
    pub lan_visibility: Option<bool>,
    pub sync_enabled: Option<bool>,
    pub notify_on_copy: Option<bool>,
    pub notification_preview: Option<bool>,
    pub sound_on_copy: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct CloudAccountStatus {
    pub configured: bool,
    pub signed_in: bool,
    pub key_ready: bool,
    pub email: Option<String>,
    pub last_sync_ms: Option<i64>,
    pub last_error: Option<String>,
    pub unreadable_uploads: u32,
}

#[derive(Debug, Clone)]
pub struct CloudSyncSummary {
    pub uploaded: u32,
    pub tombstoned: u32,
    pub downloaded: u32,
    pub applied: u32,
    pub skipped_undecryptable: u32,
    pub skipped_forged: u32,
    pub skipped_future: u32,
    pub skipped_too_large: u32,
}

#[derive(Debug, Clone)]
pub struct TextExportSummary {
    pub exported: u32,
    pub skipped_non_text: u32,
    pub skipped_undecryptable: u32,
}

#[derive(Debug, Clone)]
pub struct BackupSummary {
    pub size_bytes: u64,
}

/// Starts an app-owned desktop daemon in an explicit application data directory.
///
/// The application packaging layer supplies the bundled daemon executable. The
/// child receives no cloud configuration and exits when this bridge releases
/// its parent pipe. The data directory is isolated from the standalone CLI
/// daemon so each process has one unambiguous storage and lifetime owner.
pub async fn start_desktop_runtime(
    daemon_executable: String,
    data_dir: String,
) -> Result<(), RuntimeError> {
    crate::runtime::start(daemon_executable, data_dir).await
}

/// Releases the private app-parent pipe and lets the desktop daemon shut down.
pub fn stop_desktop_runtime() {
    crate::runtime::stop();
}

/// Returns runtime status only; it does not start or discover a
/// daemon. Startup remains owned by the application lifecycle integration.
pub async fn runtime_status() -> Result<ThisDevice, RuntimeError> {
    let response = client::request(Method::Status).await?;
    match response.data {
        Some(ResponseData::Status(status)) => Ok(this_device(status)),
        _ => Err(RuntimeError::internal()),
    }
}

pub async fn capture_state() -> Result<CaptureState, RuntimeError> {
    let response = client::request(Method::Status).await?;
    match response.data {
        Some(ResponseData::Status(status)) => Ok(CaptureState {
            running: status.capture_running,
            paused: status.private_mode,
            private_mode_epoch: status.private_mode_epoch,
        }),
        _ => Err(RuntimeError::internal()),
    }
}

pub async fn set_capture_paused(paused: bool) -> Result<CaptureState, RuntimeError> {
    let response = client::request(Method::SetPrivateMode { enabled: paused }).await?;
    if !matches!(response.data, Some(ResponseData::PrivateMode(_))) {
        return Err(RuntimeError::internal());
    }
    capture_state().await
}

pub async fn get_runtime_settings() -> Result<RuntimeSettingsData, RuntimeError> {
    let response = client::request(Method::GetConfig).await?;
    match response.data {
        Some(ResponseData::Config(applied)) => Ok(runtime_settings(applied.config)),
        _ => Err(RuntimeError::internal()),
    }
}

pub async fn update_runtime_settings(
    patch: RuntimeSettingsPatch,
) -> Result<RuntimeSettingsData, RuntimeError> {
    let response = client::request(Method::SetConfig {
        patch: copypaste_ipc::ConfigPatch {
            retention_days: patch.retention_days,
            storage_quota_bytes: patch.storage_quota_bytes,
            excluded_app_bundle_ids: patch.excluded_app_ids,
            lan_visibility: patch.lan_visibility,
            sync_enabled: patch.sync_enabled,
            notify_on_copy: patch.notify_on_copy,
            notification_preview: patch.notification_preview,
            sound_on_copy: patch.sound_on_copy,
            ..Default::default()
        },
    })
    .await?;
    match response.data {
        Some(ResponseData::Config(applied)) => Ok(runtime_settings(applied.config)),
        _ => Err(RuntimeError::internal()),
    }
}

pub async fn cloud_account_status() -> Result<CloudAccountStatus, RuntimeError> {
    cloud_status_response(Method::CloudStatus).await
}

pub async fn cloud_sign_in(
    email: String,
    password: String,
    passphrase: String,
) -> Result<CloudAccountStatus, RuntimeError> {
    cloud_status_response(Method::CloudSignIn {
        email,
        password,
        passphrase,
    })
    .await
}

pub async fn cloud_sign_up(
    email: String,
    password: String,
    passphrase: String,
) -> Result<CloudAccountStatus, RuntimeError> {
    cloud_status_response(Method::CloudSignUp {
        email,
        password,
        passphrase,
    })
    .await
}

pub async fn cloud_sign_out() -> Result<CloudAccountStatus, RuntimeError> {
    cloud_status_response(Method::CloudSignOut).await
}

pub async fn cloud_sync_now() -> Result<CloudSyncSummary, RuntimeError> {
    let response = client::request(Method::CloudSyncNow).await?;
    match response.data {
        Some(ResponseData::CloudSync(summary)) => Ok(CloudSyncSummary {
            uploaded: summary.uploaded,
            tombstoned: summary.tombstoned,
            downloaded: summary.downloaded,
            applied: summary.applied,
            skipped_undecryptable: summary.skipped_undecryptable,
            skipped_forged: summary.skipped_forged,
            skipped_future: summary.skipped_future,
            skipped_too_large: summary.skipped_too_large,
        }),
        _ => Err(RuntimeError::internal()),
    }
}

pub async fn export_text_history(file_path: String) -> Result<TextExportSummary, RuntimeError> {
    let response = client::request(Method::Export { limit: 0 }).await?;
    let Some(ResponseData::Export(export)) = response.data else {
        return Err(RuntimeError::internal());
    };
    let encoded = serde_json::to_vec_pretty(&export).map_err(|_| RuntimeError::internal())?;
    let mut file =
        create_private_file(&file_path).map_err(|_| RuntimeError::export_write_failed())?;
    file.write_all(&encoded)
        .and_then(|()| file.write_all(b"\n"))
        .map_err(|_| RuntimeError::export_write_failed())?;
    Ok(TextExportSummary {
        exported: u32::try_from(export.items.len()).unwrap_or(u32::MAX),
        skipped_non_text: export.skipped_non_text,
        skipped_undecryptable: export.skipped_undecryptable,
    })
}

pub async fn backup_history(file_path: String) -> Result<BackupSummary, RuntimeError> {
    let response = client::request(Method::Backup {
        dest_path: file_path,
    })
    .await?;
    match response.data {
        Some(ResponseData::Backup(summary)) => Ok(BackupSummary {
            size_bytes: summary.size_bytes,
        }),
        _ => Err(RuntimeError::internal()),
    }
}

pub async fn restore_history(file_path: String) -> Result<(), RuntimeError> {
    empty_response(Method::Restore {
        src_path: file_path,
        confirm: true,
    })
    .await
}

pub async fn list_clips(limit: u32, cursor: Option<String>) -> Result<ClipPage, RuntimeError> {
    let response = client::request(Method::List { limit, cursor }).await?;
    match response.data {
        Some(ResponseData::Page(page)) => Ok(ClipPage {
            clips: page.items.into_iter().map(clip).collect(),
            next_cursor: page.next_cursor,
            skipped_undecryptable: page.skipped_undecryptable,
        }),
        _ => Err(RuntimeError::internal()),
    }
}

pub async fn search_clips(query: String, limit: u32) -> Result<ClipPage, RuntimeError> {
    let response = client::request(Method::Search { query, limit }).await?;
    match response.data {
        Some(ResponseData::Page(page)) => Ok(ClipPage {
            clips: page.items.into_iter().map(clip).collect(),
            next_cursor: page.next_cursor,
            skipped_undecryptable: page.skipped_undecryptable,
        }),
        _ => Err(RuntimeError::internal()),
    }
}

/// Lazily queries the complete retained history through the daemon's unified
/// filter/sort/cursor contract.
pub async fn query_clips(
    query: ClipQuery,
    limit: u32,
    cursor: Option<String>,
) -> Result<ClipPage, RuntimeError> {
    let response = client::request(Method::HistoryQuery {
        query: history_query(query),
        limit,
        cursor,
    })
    .await?;
    match response.data {
        Some(ResponseData::Page(page)) => Ok(ClipPage {
            clips: page.items.into_iter().map(clip).collect(),
            next_cursor: page.next_cursor,
            skipped_undecryptable: page.skipped_undecryptable,
        }),
        _ => Err(RuntimeError::internal()),
    }
}
pub async fn history_facets() -> Result<HistoryFacets, RuntimeError> {
    let response = client::request(Method::HistoryFacets).await?;
    let Some(ResponseData::HistoryFacets(facets)) = response.data else {
        return Err(RuntimeError::internal());
    };
    let origin_devices = facets
        .origin_devices
        .into_iter()
        .map(|row| HistoryDeviceFacet {
            id: row.id,
            label: row.label,
            device_class: device_class(row.device_class),
        })
        .collect();
    let source_apps = facets
        .source_apps
        .into_iter()
        .map(|row| HistorySourceAppFacet {
            id: row.id,
            label: row.label,
            icon_item_id: row.icon_item_id,
        })
        .collect();
    Ok(HistoryFacets {
        origin_devices,
        source_apps,
    })
}

/// Allocates one independently cancellable runtime watch subscription.
pub fn allocate_runtime_watch() -> u64 {
    crate::runtime::allocate_watch()
}

/// Emits content-free backend invalidation signals until the matching Dart
/// subscription is cancelled. A `kind` of `items` or `peers` tells the
/// repository what to reload; it never carries a clip id or clipboard body.
pub async fn watch_runtime(
    watch_id: u64,
    sink: crate::frb_generated::StreamSink<RuntimeEvent>,
) -> Result<(), RuntimeError> {
    let result = crate::client::watch(watch_id, sink).await;
    crate::runtime::cancel_watch(watch_id);
    result
}

/// Cancels active generated runtime watch streams without waiting for a daemon
/// event. Repositories call this from their disposal path; runtime shutdown
/// also invokes it before releasing the daemon parent pipe.
pub fn cancel_runtime_watch(watch_id: u64) {
    crate::runtime::cancel_watch(watch_id);
}

/// Starts a Rust-held invitation ceremony and returns safe progress only.
pub async fn create_pairing_ceremony() -> Result<PairingCeremony, RuntimeError> {
    crate::protected::create().await
}

/// Starts the backend `PairJoin { code, addr }` flow and returns safe progress.
pub async fn join_pairing_ceremony(
    code: String,
    address: String,
) -> Result<PairingCeremony, RuntimeError> {
    crate::protected::join(code, address).await
}

/// Starts pairing from the one versioned `copypaste://pair` invite contract.
pub async fn join_pairing_uri(uri: String) -> Result<PairingCeremony, RuntimeError> {
    crate::protected::join_uri(uri).await
}

/// Reads Rust-owned ceremony state without exposing SAS or invite material.
pub async fn pairing_ceremony_status(ceremony_id: String) -> Result<PairingCeremony, RuntimeError> {
    crate::protected::status(&ceremony_id).await
}

/// Renders the Rust-held invitation only inside capture-protected Flutter UI.
pub async fn reveal_pairing_qr(ceremony_id: String) -> Result<Vec<u8>, RuntimeError> {
    crate::protected::reveal_qr_for_flutter(&ceremony_id).await
}

/// Returns the bound SAS only for immediate display in capture-protected UI.
pub async fn reveal_pairing_sas(ceremony_id: String) -> Result<String, RuntimeError> {
    crate::protected::reveal_sas_for_flutter(&ceremony_id).await
}

/// Records a decision only when it carries the freshly revealed bound SAS.
pub async fn confirm_pairing_ceremony(
    ceremony_id: String,
    verification_code: String,
    accept: bool,
) -> Result<PairingCeremony, RuntimeError> {
    crate::protected::confirm(&ceremony_id, &verification_code, accept).await
}

pub async fn cancel_pairing_ceremony(ceremony_id: String) -> Result<PairingCeremony, RuntimeError> {
    crate::protected::cancel(&ceremony_id).await
}

pub async fn dispose_pairing_ceremony(ceremony_id: String) -> Result<(), RuntimeError> {
    crate::protected::dispose(&ceremony_id).await
}

pub async fn get_clip(id: String) -> Result<Clip, RuntimeError> {
    item_response(Method::Get { id }).await
}

pub async fn copy_clip(id: String) -> Result<Clip, RuntimeError> {
    item_response(Method::Copy { id }).await
}

pub async fn copy_clip_as_plain_text(id: String) -> Result<Clip, RuntimeError> {
    item_response(Method::CopyPlainText { id }).await
}

pub async fn save_clip_file(id: String, dest_path: String) -> Result<(), RuntimeError> {
    empty_response(Method::SaveFile { id, dest_path }).await
}

pub async fn delete_clip(id: String) -> Result<(), RuntimeError> {
    empty_response(Method::Delete { id }).await
}

pub async fn delete_clips_through(through: Option<i64>) -> Result<u64, RuntimeError> {
    let response = client::request(Method::DeleteAll { through }).await?;
    match response.data {
        Some(ResponseData::Count(count)) => Ok(count),
        _ => Err(RuntimeError::internal()),
    }
}

pub async fn history_ceiling() -> Result<u64, RuntimeError> {
    let response = client::request(Method::HistoryCeiling).await?;
    match response.data {
        Some(ResponseData::Count(count)) => Ok(count),
        _ => Err(RuntimeError::internal()),
    }
}

pub async fn set_clip_pinned(id: String, pinned: bool) -> Result<Clip, RuntimeError> {
    item_response(Method::Pin { id, pinned }).await
}

pub async fn reorder_pinned_clips(ids: Vec<String>) -> Result<u64, RuntimeError> {
    let response = client::request(Method::ReorderPinned { ids }).await?;
    match response.data {
        Some(ResponseData::Count(count)) => Ok(count),
        _ => Err(RuntimeError::internal()),
    }
}

pub async fn clip_image_preview(
    id: String,
    max_edge: Option<u32>,
    bounds: Option<ClipImagePreviewBounds>,
) -> Result<ClipImagePreview, RuntimeError> {
    let response = client::request(Method::ImagePreview {
        id,
        max_edge,
        bounds: bounds.map(|bounds| copypaste_ipc::ImagePreviewBounds {
            width: bounds.width,
            height: bounds.height,
        }),
    })
    .await?;
    match response.data {
        Some(ResponseData::ImagePreview(preview)) => Ok(ClipImagePreview {
            png_base64: preview.png_base64,
            width: preview.width,
            height: preview.height,
        }),
        _ => Err(RuntimeError::internal()),
    }
}

/// Lazily returns the persisted source-application icon when the backend has
/// authenticated icon metadata for this clip.
pub async fn clip_source_app_icon(id: String) -> Result<Option<ClipImagePreview>, RuntimeError> {
    let response = client::request(Method::SourceAppIcon { id }).await?;
    match response.data {
        Some(ResponseData::SourceAppIcon(preview)) => Ok(Some(ClipImagePreview {
            png_base64: preview.png_base64,
            width: preview.width,
            height: preview.height,
        })),
        Some(ResponseData::Empty {}) => Ok(None),
        _ => Err(RuntimeError::internal()),
    }
}

pub async fn list_peers() -> Result<Vec<Peer>, RuntimeError> {
    let response = client::request(Method::Peers).await?;
    match response.data {
        Some(ResponseData::Peers(peers)) => Ok(peers
            .into_iter()
            .map(|peer| Peer {
                pairing_id: peer.pairing_id,
                name: peer.name,
                last_seen_ms: peer.last_seen_ms,
                online: peer.online,
                details: peer.details.map(device_details),
            })
            .collect()),
        _ => Err(RuntimeError::internal()),
    }
}

pub async fn list_discovered_devices() -> Result<Vec<DiscoveredDevice>, RuntimeError> {
    discovered_response(Method::Discovered).await
}

pub async fn rescan_devices() -> Result<Vec<DiscoveredDevice>, RuntimeError> {
    discovered_response(Method::Rescan).await
}

pub async fn sync_devices(pairing_id: Option<String>) -> Result<Vec<SyncOutcome>, RuntimeError> {
    let response = client::request(Method::SyncNow { pairing_id }).await?;
    match response.data {
        Some(ResponseData::Sync(outcomes)) => Ok(outcomes
            .into_iter()
            .map(|outcome| SyncOutcome {
                pairing_id: outcome.pairing_id,
                name: outcome.name,
                sent: outcome.sent,
                received: outcome.received,
                error_code: outcome.error_code.map(|code| code.as_str().to_string()),
            })
            .collect()),
        _ => Err(RuntimeError::internal()),
    }
}

pub async fn unpair_device(pairing_id: String) -> Result<(), RuntimeError> {
    empty_response(Method::Unpair { pairing_id }).await
}

pub async fn revoke_device(pairing_id: String) -> Result<(), RuntimeError> {
    empty_response(Method::Revoke { pairing_id }).await
}

pub async fn this_device_name() -> Result<ThisDevice, RuntimeError> {
    runtime_status().await
}

pub async fn set_this_device_name(name: String) -> Result<(), RuntimeError> {
    empty_response(Method::SetDeviceName { name }).await
}

async fn item_response(method: Method) -> Result<Clip, RuntimeError> {
    let response = client::request(method).await?;
    match response.data {
        Some(ResponseData::Item(item)) => Ok(clip(item)),
        _ => Err(RuntimeError::internal()),
    }
}

async fn empty_response(method: Method) -> Result<(), RuntimeError> {
    let response = client::request(method).await?;
    match response.data {
        Some(ResponseData::Empty {}) => Ok(()),
        _ => Err(RuntimeError::internal()),
    }
}

async fn discovered_response(method: Method) -> Result<Vec<DiscoveredDevice>, RuntimeError> {
    let response = client::request(method).await?;
    match response.data {
        Some(ResponseData::Discovered(discovered)) => Ok(discovered
            .devices
            .into_iter()
            .map(|device| DiscoveredDevice {
                discovery_id: device.discovery_id,
                name: device.name,
                address: device.addr,
                paired: device.paired,
                last_seen_ms: device.last_seen_ms,
                details: device.details.map(device_details),
            })
            .collect()),
        _ => Err(RuntimeError::internal()),
    }
}

async fn cloud_status_response(method: Method) -> Result<CloudAccountStatus, RuntimeError> {
    let response = client::request(method).await?;
    match response.data {
        Some(ResponseData::CloudStatus(status)) => Ok(CloudAccountStatus {
            configured: status.configured,
            signed_in: status.signed_in,
            key_ready: status.key_ready,
            email: status.email,
            last_sync_ms: status.last_sync_ms,
            last_error: status.last_error,
            unreadable_uploads: status.unreadable_uploads,
        }),
        _ => Err(RuntimeError::internal()),
    }
}

fn runtime_settings(config: copypaste_ipc::ConfigData) -> RuntimeSettingsData {
    RuntimeSettingsData {
        retention_days: config.retention_days,
        storage_quota_bytes: config.storage_quota_bytes,
        excluded_app_ids: config.excluded_app_bundle_ids,
        lan_visibility: config.lan_visibility,
        sync_enabled: config.sync_enabled,
        notify_on_copy: config.notify_on_copy,
        notification_preview: config.notification_preview,
        sound_on_copy: config.sound_on_copy,
    }
}

#[cfg(unix)]
fn create_private_file(path: &str) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn create_private_file(path: &str) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
}

fn device_details(details: copypaste_ipc::DeviceDetails) -> DeviceDetails {
    DeviceDetails {
        profile: details.profile.map(device_profile),
        endpoint: details.endpoint.map(device_endpoint),
        latency: details.latency.map(device_latency),
        presence: details.presence.map(device_presence),
    }
}

fn this_device(status: copypaste_ipc::StatusData) -> ThisDevice {
    ThisDevice {
        device_id: status.device_id,
        name: status.device_name,
        app_version: status.version,
        protocol_version: status.protocol_version,
        listen_address: status.listen_addr,
        details: status.device_details.map(device_details),
    }
}

fn device_profile(profile: copypaste_ipc::DeviceProfileObservation) -> DeviceProfile {
    DeviceProfile {
        display_name: profile.display_name,
        app_version: profile.app_version,
        protocol_version: profile.protocol_version,
        platform: device_platform(profile.platform),
        device_class: device_class(profile.device_class),
        os_name: profile.os_name,
        os_version: profile.os_version,
        model: profile.model,
        provenance: device_observation_provenance(profile.provenance),
        trust: device_observation_trust(profile.trust),
        observed_at_ms: profile.observed_at_ms,
        fresh_until_ms: profile.fresh_until_ms,
    }
}

fn device_endpoint(endpoint: copypaste_ipc::DeviceEndpointObservation) -> DeviceEndpoint {
    DeviceEndpoint {
        lan_endpoint: endpoint.lan_endpoint,
        provenance: device_observation_provenance(endpoint.provenance),
        trust: device_observation_trust(endpoint.trust),
        observed_at_ms: endpoint.observed_at_ms,
        fresh_until_ms: endpoint.fresh_until_ms,
    }
}

fn device_latency(latency: copypaste_ipc::DeviceLatencyObservation) -> DeviceLatency {
    DeviceLatency {
        round_trip_latency_ms: latency.round_trip_latency_ms,
        provenance: device_observation_provenance(latency.provenance),
        trust: device_observation_trust(latency.trust),
        observed_at_ms: latency.observed_at_ms,
        fresh_until_ms: latency.fresh_until_ms,
    }
}

fn device_presence(presence: copypaste_ipc::DevicePresenceObservation) -> DevicePresence {
    DevicePresence {
        state: match presence.state {
            copypaste_ipc::DevicePresence::Online => DevicePresenceState::Online,
            copypaste_ipc::DevicePresence::Offline => DevicePresenceState::Offline,
            copypaste_ipc::DevicePresence::Unknown => DevicePresenceState::Unknown,
        },
        last_seen_ms: presence.last_seen_ms,
        provenance: device_observation_provenance(presence.provenance),
        trust: device_observation_trust(presence.trust),
        observed_at_ms: presence.observed_at_ms,
        fresh_until_ms: presence.fresh_until_ms,
    }
}

fn device_platform(platform: copypaste_ipc::DevicePlatform) -> DevicePlatform {
    match platform {
        copypaste_ipc::DevicePlatform::Macos => DevicePlatform::Macos,
        copypaste_ipc::DevicePlatform::Windows => DevicePlatform::Windows,
        copypaste_ipc::DevicePlatform::Android => DevicePlatform::Android,
        copypaste_ipc::DevicePlatform::Unknown => DevicePlatform::Unknown,
    }
}

fn device_class(device_class: copypaste_ipc::DeviceClass) -> DeviceClass {
    match device_class {
        copypaste_ipc::DeviceClass::Desktop => DeviceClass::Desktop,
        copypaste_ipc::DeviceClass::Laptop => DeviceClass::Laptop,
        copypaste_ipc::DeviceClass::Phone => DeviceClass::Phone,
        copypaste_ipc::DeviceClass::Tablet => DeviceClass::Tablet,
        copypaste_ipc::DeviceClass::Unknown => DeviceClass::Unknown,
    }
}

fn device_observation_provenance(
    provenance: copypaste_ipc::DeviceObservationProvenance,
) -> DeviceObservationProvenance {
    match provenance {
        copypaste_ipc::DeviceObservationProvenance::SelfReported => {
            DeviceObservationProvenance::SelfReported
        }
        copypaste_ipc::DeviceObservationProvenance::Observed => {
            DeviceObservationProvenance::Observed
        }
        copypaste_ipc::DeviceObservationProvenance::Measured => {
            DeviceObservationProvenance::Measured
        }
    }
}

fn device_observation_trust(
    trust: copypaste_ipc::DeviceObservationTrust,
) -> DeviceObservationTrust {
    match trust {
        copypaste_ipc::DeviceObservationTrust::Local => DeviceObservationTrust::Local,
        copypaste_ipc::DeviceObservationTrust::Unverified => DeviceObservationTrust::Unverified,
        copypaste_ipc::DeviceObservationTrust::Authenticated => {
            DeviceObservationTrust::Authenticated
        }
    }
}

fn clip(item: copypaste_ipc::Item) -> Clip {
    Clip {
        id: item.id,
        content: item.content,
        content_type: item.content_type,
        content_class: content_class(item.content_class),
        semantic_kind: item.semantic_kind.map(semantic_kind),
        color_rgba: item.color_rgba,
        created_at_ms: item.created_at,
        pinned: item.pinned,
        origin_device_name: item.origin_device_name,
        origin_device_class: device_class(item.origin_device_class),
        source_app_name: item.source_app_name,
        truncated: item.truncated,
        too_large_to_sync: item.too_large_to_sync,
        file_details: item.file_details.map(|details| ClipFileDetails {
            filename: details.filename,
            mime_type: details.mime_type,
            source_reference: details.source_reference,
            source_available: details.source_available,
            size_bytes: details.size_bytes,
            file_count: details.file_count,
        }),
        image_details: item.image_details.map(|details| ClipImageDetails {
            width: details.width,
            height: details.height,
            size_bytes: details.size_bytes,
        }),
    }
}

fn history_query(query: ClipQuery) -> copypaste_ipc::HistoryQuery {
    copypaste_ipc::HistoryQuery {
        search: query.search,
        content_classes: query
            .content_classes
            .into_iter()
            .map(content_class_to_ipc)
            .collect(),
        semantic_kinds: query
            .semantic_kinds
            .into_iter()
            .map(semantic_kind_to_ipc)
            .collect(),
        pinned_only: query.pinned_only,
        origin_device_id: query.origin_device_id,
        source_app_bundle_id: query.source_app_bundle_id,
        sort: match query.sort {
            ClipSort::Newest => copypaste_ipc::HistorySort::Newest,
            ClipSort::Oldest => copypaste_ipc::HistorySort::Oldest,
            ClipSort::Relevance => copypaste_ipc::HistorySort::Relevance,
        },
    }
}

fn semantic_kind(value: copypaste_ipc::SemanticKind) -> ClipSemanticKind {
    match value {
        copypaste_ipc::SemanticKind::PlainText => ClipSemanticKind::PlainText,
        copypaste_ipc::SemanticKind::Link => ClipSemanticKind::Link,
        copypaste_ipc::SemanticKind::Email => ClipSemanticKind::Email,
        copypaste_ipc::SemanticKind::Color => ClipSemanticKind::Color,
        copypaste_ipc::SemanticKind::Phone => ClipSemanticKind::Phone,
        copypaste_ipc::SemanticKind::Code => ClipSemanticKind::Code,
        copypaste_ipc::SemanticKind::Json => ClipSemanticKind::Json,
        copypaste_ipc::SemanticKind::Path => ClipSemanticKind::Path,
    }
}

fn semantic_kind_to_ipc(value: ClipSemanticKind) -> copypaste_ipc::SemanticKind {
    match value {
        ClipSemanticKind::PlainText => copypaste_ipc::SemanticKind::PlainText,
        ClipSemanticKind::Link => copypaste_ipc::SemanticKind::Link,
        ClipSemanticKind::Email => copypaste_ipc::SemanticKind::Email,
        ClipSemanticKind::Color => copypaste_ipc::SemanticKind::Color,
        ClipSemanticKind::Phone => copypaste_ipc::SemanticKind::Phone,
        ClipSemanticKind::Code => copypaste_ipc::SemanticKind::Code,
        ClipSemanticKind::Json => copypaste_ipc::SemanticKind::Json,
        ClipSemanticKind::Path => copypaste_ipc::SemanticKind::Path,
    }
}

fn content_class(value: copypaste_ipc::ContentClass) -> ClipContentClass {
    match value {
        copypaste_ipc::ContentClass::Text => ClipContentClass::Text,
        copypaste_ipc::ContentClass::Image => ClipContentClass::Image,
        copypaste_ipc::ContentClass::File => ClipContentClass::File,
        copypaste_ipc::ContentClass::Other => ClipContentClass::Other,
    }
}

fn content_class_to_ipc(value: ClipContentClass) -> copypaste_ipc::ContentClass {
    match value {
        ClipContentClass::Text => copypaste_ipc::ContentClass::Text,
        ClipContentClass::Image => copypaste_ipc::ContentClass::Image,
        ClipContentClass::File => copypaste_ipc::ContentClass::File,
        ClipContentClass::Other => copypaste_ipc::ContentClass::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_device_maps_the_explicit_local_identity() {
        let device = this_device(copypaste_ipc::StatusData {
            device_name: "Desktop".into(),
            device_id: Some("local-device-id".into()),
            version: "1.2.3".into(),
            protocol_version: 7,
            listen_addr: Some("192.0.2.10:47654".into()),
            device_details: None,
            item_count: 0,
            capture_running: false,
            clipboard_backend: "fake".into(),
            private_mode: false,
            private_mode_epoch: 0,
            counters: Default::default(),
            settings_health: None,
        });

        assert_eq!(device.device_id.as_deref(), Some("local-device-id"));
        assert_eq!(device.name, "Desktop");
    }

    #[test]
    fn device_details_maps_safe_observations_without_pairing_material() {
        let details = device_details(copypaste_ipc::DeviceDetails {
            profile: Some(copypaste_ipc::DeviceProfileObservation {
                display_name: "Pixel 9".into(),
                app_version: Some("1.2.3".into()),
                protocol_version: Some(7),
                platform: copypaste_ipc::DevicePlatform::Android,
                device_class: copypaste_ipc::DeviceClass::Phone,
                os_name: Some("Android".into()),
                os_version: Some("16".into()),
                model: Some("Pixel 9".into()),
                provenance: copypaste_ipc::DeviceObservationProvenance::SelfReported,
                trust: copypaste_ipc::DeviceObservationTrust::Authenticated,
                observed_at_ms: 10,
                fresh_until_ms: Some(20),
            }),
            endpoint: Some(copypaste_ipc::DeviceEndpointObservation {
                lan_endpoint: "192.0.2.10:47654".into(),
                provenance: copypaste_ipc::DeviceObservationProvenance::Observed,
                trust: copypaste_ipc::DeviceObservationTrust::Authenticated,
                observed_at_ms: 11,
                fresh_until_ms: Some(21),
            }),
            latency: Some(copypaste_ipc::DeviceLatencyObservation {
                round_trip_latency_ms: 24,
                provenance: copypaste_ipc::DeviceObservationProvenance::Measured,
                trust: copypaste_ipc::DeviceObservationTrust::Authenticated,
                observed_at_ms: 12,
                fresh_until_ms: Some(22),
            }),
            presence: Some(copypaste_ipc::DevicePresenceObservation {
                state: copypaste_ipc::DevicePresence::Online,
                last_seen_ms: 13,
                provenance: copypaste_ipc::DeviceObservationProvenance::Observed,
                trust: copypaste_ipc::DeviceObservationTrust::Local,
                observed_at_ms: 14,
                fresh_until_ms: Some(24),
            }),
            ..copypaste_ipc::DeviceDetails::default()
        });

        let profile = details.profile.expect("profile");
        assert_eq!(profile.platform, DevicePlatform::Android);
        assert_eq!(profile.device_class, DeviceClass::Phone);
        assert_eq!(profile.os_name.as_deref(), Some("Android"));
        assert_eq!(profile.model.as_deref(), Some("Pixel 9"));
        assert_eq!(profile.trust, DeviceObservationTrust::Authenticated);
        assert_eq!(
            details.endpoint.expect("endpoint").lan_endpoint,
            "192.0.2.10:47654"
        );
        assert_eq!(details.latency.expect("latency").round_trip_latency_ms, 24);
        assert_eq!(
            details.presence.expect("presence").state,
            DevicePresenceState::Online
        );
    }

    #[test]
    fn history_query_preserves_semantic_filters() {
        let query = history_query(ClipQuery {
            search: Some("needle".into()),
            content_classes: Vec::new(),
            semantic_kinds: vec![ClipSemanticKind::Link, ClipSemanticKind::Color],
            pinned_only: true,
            origin_device_id: None,
            source_app_bundle_id: None,
            sort: ClipSort::Newest,
        });

        assert_eq!(
            query.semantic_kinds,
            vec![
                copypaste_ipc::SemanticKind::Link,
                copypaste_ipc::SemanticKind::Color
            ]
        );
        assert!(query.content_classes.is_empty());
        assert!(query.pinned_only);
    }

    #[test]
    fn clip_maps_original_image_metadata_and_sync_warning() {
        let mapped = clip(copypaste_ipc::Item {
            id: "image-1".into(),
            content: "[image]".into(),
            content_type: copypaste_ipc::content_type::IMAGE_PNG.into(),
            content_class: copypaste_ipc::ContentClass::Image,
            semantic_kind: None,
            color_rgba: None,
            created_at: 1,
            pinned: false,
            file_details: None,
            image_details: Some(copypaste_ipc::ImageDetails {
                width: 1_920,
                height: 1_080,
                size_bytes: 4_096,
            }),
            origin_device_id: "device-1".into(),
            origin_device_name: Some("Phone".into()),
            origin_device_class: copypaste_ipc::DeviceClass::Phone,
            source_app_bundle_id: None,
            source_app_name: Some("Camera".into()),
            too_large_to_sync: true,
            truncated: false,
        });

        let metadata = mapped.image_details.expect("image metadata");
        assert_eq!((metadata.width, metadata.height), (1_920, 1_080));
        assert_eq!(metadata.size_bytes, 4_096);
        assert!(mapped.too_large_to_sync);
        assert_eq!(mapped.origin_device_class, DeviceClass::Phone);
    }
}

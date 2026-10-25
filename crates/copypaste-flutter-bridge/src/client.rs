//! One private daemon IPC request for the Flutter bridge.
//!
//! This uses the existing typed `copypaste-ipc` request/response framing. It
//! never opens a network listener and intentionally collapses transport details
//! to pathless errors before a Flutter caller can see them.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use copypaste_ipc::transport;
use copypaste_ipc::{Method, Request, Response, MAX_FRAME_BYTES, PROTOCOL_VERSION};
use futures_util::{SinkExt, StreamExt};
use tokio::time::timeout;
use tokio_util::codec::{Framed, LinesCodec};

use crate::api::RuntimeError;
use crate::{api::RuntimeEvent, frb_generated::StreamSink};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(180);
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

pub(crate) async fn request(method: Method) -> Result<Response, RuntimeError> {
    #[cfg(target_os = "android")]
    {
        return crate::runtime_android::request(method)
            .await
            .and_then(response_to_result);
    }

    #[cfg(not(target_os = "android"))]
    {
        let timeout_budget = if method.is_long_running() {
            TRANSFER_TIMEOUT
        } else {
            REQUEST_TIMEOUT
        };
        timeout(timeout_budget, request_once(method))
            .await
            .map_err(|_| RuntimeError::timeout())?
    }
}

#[cfg(not(target_os = "android"))]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) async fn request_at(
    path: &std::path::Path,
    method: Method,
) -> Result<Response, RuntimeError> {
    let timeout_budget = if method.is_long_running() {
        TRANSFER_TIMEOUT
    } else {
        REQUEST_TIMEOUT
    };
    timeout(timeout_budget, request_once_at(path, method))
        .await
        .map_err(|_| RuntimeError::timeout())?
}

#[cfg(not(target_os = "android"))]
async fn request_once(method: Method) -> Result<Response, RuntimeError> {
    let path = crate::runtime::socket_path()?;
    request_once_at(&path, method).await
}

#[cfg(not(target_os = "android"))]
async fn request_once_at(path: &std::path::Path, method: Method) -> Result<Response, RuntimeError> {
    let stream = transport::connect(path)
        .await
        .map_err(|_| RuntimeError::daemon_unreachable())?;
    let mut framed = Framed::new(stream, LinesCodec::new_with_max_length(MAX_FRAME_BYTES));
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let request = Request {
        id,
        protocol_version: PROTOCOL_VERSION,
        method,
    };
    let line = serde_json::to_string(&request).map_err(|_| RuntimeError::internal())?;
    framed
        .send(line)
        .await
        .map_err(|_| RuntimeError::daemon_unreachable())?;
    let reply = framed
        .next()
        .await
        .ok_or_else(RuntimeError::daemon_unreachable)?
        .map_err(|_| RuntimeError::daemon_unreachable())?;
    let response: Response = serde_json::from_str(&reply).map_err(|_| RuntimeError::internal())?;
    if response.id != id {
        return Err(RuntimeError::internal());
    }
    response_to_result(response)
}

fn response_to_result(response: Response) -> Result<Response, RuntimeError> {
    if response.ok {
        return Ok(response);
    }
    let code = response.error_code.map_or_else(
        || {
            response
                .raw_error_code
                .unwrap_or_else(|| "unknown".to_string())
        },
        |code| code.as_str().to_string(),
    );
    let message = response
        .error
        .unwrap_or_else(|| "The runtime refused the request.".to_string());
    Err(RuntimeError::from_daemon(code, message))
}

/// Keeps a dedicated daemon watch connection open until Flutter cancels its
/// generated stream. Events hold no clip content; callers reload through the
/// ordinary typed methods after a signal.
pub(crate) async fn watch(
    watch_id: u64,
    sink: StreamSink<RuntimeEvent>,
) -> Result<(), RuntimeError> {
    #[cfg(target_os = "android")]
    {
        return crate::runtime_android::watch(watch_id, sink).await;
    }

    #[cfg(not(target_os = "android"))]
    {
        let typed = match request(Method::Status).await?.data {
            Some(copypaste_ipc::ResponseData::Status(status)) => status.runtime_watch,
            _ => return Err(RuntimeError::internal()),
        };
        let stream = transport::connect(&crate::runtime::socket_path()?)
            .await
            .map_err(|_| RuntimeError::daemon_unreachable())?;
        let mut framed = Framed::new(stream, LinesCodec::new_with_max_length(MAX_FRAME_BYTES));
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let request = Request {
            id,
            protocol_version: PROTOCOL_VERSION,
            method: if typed {
                Method::WatchRuntime
            } else {
                Method::Watch
            },
        };
        let line = serde_json::to_string(&request).map_err(|_| RuntimeError::internal())?;
        framed
            .send(line)
            .await
            .map_err(|_| RuntimeError::daemon_unreachable())?;
        let ack = next_response(&mut framed, id).await?;
        if !matches!(ack.data, Some(copypaste_ipc::ResponseData::Empty {})) {
            return Err(RuntimeError::internal());
        }
        if sink.add(runtime_ready(typed)).is_err() {
            return Ok(());
        }
        let mut cancelled = crate::runtime::watch_stop_rx(watch_id)?;
        loop {
            let line = tokio::select! {
                changed = cancelled.changed() => {
                    changed.map_err(|_| RuntimeError::internal())?;
                    return Ok(());
                }
                line = framed.next() => line,
            };
            let Some(line) = line else {
                return Ok(());
            };
            let line = line.map_err(|_| RuntimeError::daemon_unreachable())?;
            let response: Response =
                serde_json::from_str(&line).map_err(|_| RuntimeError::internal())?;
            if response.id != id {
                return Err(RuntimeError::internal());
            }
            let response = response_to_result(response)?;
            let event = match response.data {
                Some(copypaste_ipc::ResponseData::Event(event)) if !typed => runtime_event(event),
                Some(copypaste_ipc::ResponseData::RuntimeEvent(event)) if typed => {
                    typed_runtime_event(event)
                }
                _ => return Err(RuntimeError::internal()),
            };
            if sink.add(event).is_err() {
                return Ok(());
            }
        }
    }
}

fn runtime_event(event: copypaste_ipc::EventData) -> RuntimeEvent {
    RuntimeEvent {
        sync_status: event.sync_status.map(crate::api::runtime_sync_status),
        kind: match event.event {
            copypaste_ipc::EventKind::Items => "items".into(),
            copypaste_ipc::EventKind::Peers => "peers".into(),
        },
        item_count: event.item_count,
        captured: event.captured,
        captured_item_id: event.captured_item_id,
        file_operation: None,
        progress: None,
        capture: None,
        runtime_watch: false,
    }
}

pub(crate) fn runtime_ready(typed: bool) -> RuntimeEvent {
    RuntimeEvent {
        sync_status: None,
        kind: "ready".into(),
        item_count: 0,
        captured: false,
        captured_item_id: None,
        file_operation: None,
        progress: None,
        capture: None,
        runtime_watch: typed,
    }
}

pub(crate) fn typed_runtime_event(event: copypaste_ipc::RuntimeEventData) -> RuntimeEvent {
    let mut mapped = event
        .change
        .map(runtime_event)
        .unwrap_or_else(|| RuntimeEvent {
            sync_status: None,
            kind: "progress".into(),
            item_count: 0,
            captured: false,
            captured_item_id: None,
            file_operation: None,
            progress: None,
            capture: None,
            runtime_watch: true,
        });
    mapped.capture = event.capture.map(crate::api::runtime_capture_state);
    if mapped.capture.is_some() && mapped.kind == "progress" {
        mapped.kind = "capture".into();
    }
    mapped.progress = event.progress.map(|p| crate::api::FileProgress {
        token: p.token,
        written: p.written,
        total: p.total,
    });
    mapped.file_operation = event
        .file_operation
        .map(|value| crate::api::FileOperationUpdate {
            id: value.id,
            written: value.written,
            total: value.total,
            phase: value.phase.map(|phase| {
                match phase {
                    copypaste_ipc::FileOperationPhase::Running => "running",
                    copypaste_ipc::FileOperationPhase::Completed => "completed",
                    copypaste_ipc::FileOperationPhase::Failed => "failed",
                    copypaste_ipc::FileOperationPhase::Cancelled => "cancelled",
                    copypaste_ipc::FileOperationPhase::Interrupted => "interrupted",
                }
                .into()
            }),
        });
    if mapped.file_operation.is_some() {
        mapped.kind = "file_operations".into();
    }
    mapped.runtime_watch = true;
    mapped
}

async fn next_response(
    framed: &mut Framed<transport::Stream, LinesCodec>,
    expected_id: u64,
) -> Result<Response, RuntimeError> {
    let line = framed
        .next()
        .await
        .ok_or_else(RuntimeError::daemon_unreachable)?
        .map_err(|_| RuntimeError::daemon_unreachable())?;
    let response: Response = serde_json::from_str(&line).map_err(|_| RuntimeError::internal())?;
    (response.id == expected_id)
        .then_some(response)
        .ok_or_else(RuntimeError::internal)
        .and_then(response_to_result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_does_not_look_like_a_history_change() {
        let mapped = typed_runtime_event(copypaste_ipc::RuntimeEventData {
            file_operation: None,
            change: None,
            capture: None,
            progress: Some(copypaste_ipc::FileProgress {
                token: "opaque".into(),
                written: 2,
                total: 4,
            }),
        });
        assert_eq!(mapped.kind, "progress");
        let progress = mapped.progress.unwrap();
        assert_eq!(
            (progress.token.as_str(), progress.written, progress.total),
            ("opaque", 2, 4)
        );
        assert!(!mapped.captured);
    }

    #[test]
    fn items_and_peers_keep_distinct_runtime_event_kinds() {
        for (event, kind) in [
            (copypaste_ipc::EventKind::Items, "items"),
            (copypaste_ipc::EventKind::Peers, "peers"),
        ] {
            let mapped = runtime_event(copypaste_ipc::EventData {
                sync_status: Some(copypaste_ipc::SyncStatus {
                    revision: 7,
                    phase: copypaste_ipc::SyncPhase::Syncing,
                    peers: Vec::new(),
                    log: Vec::new(),
                    diagnostics_available: true,
                }),
                event,
                item_count: 3,
                captured: true,
                captured_item_id: Some("captured-item".into()),
            });
            assert_eq!(mapped.kind, kind);
            assert_eq!(mapped.item_count, 3);
            assert!(mapped.captured);
            assert_eq!(mapped.captured_item_id.as_deref(), Some("captured-item"));
            let sync_status = mapped.sync_status.unwrap();
            assert_eq!(sync_status.revision, 7);
            assert!(matches!(sync_status.phase, crate::api::SyncPhase::Syncing));
        }
    }
}

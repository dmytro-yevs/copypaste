//! What a connection becomes once it has subscribed.
//!
//! # Why a subscriber is accounted for differently
//!
//! A watcher is *supposed* to be silent for hours, which is exactly what the
//! 30-second read deadline exists to kill. So the deadline is dropped once a
//! connection has subscribed, and the write deadline is kept — a subscriber that
//! stops draining is the failure that deadline was written for, and it still
//! applies.
//!
//! Dropping the read deadline means a watcher holds its connection permit
//! indefinitely, so watchers get a second, smaller cap of their own
//! (`MAX_WATCHERS` in [`super::listener`]). Without it, sixty-four idle windows
//! would take the whole connection budget; with it, the worst a client can do by
//! subscribing is exhaust the watcher budget, and ordinary requests still get
//! through.
//!
//! # Events are signals, not data
//!
//! An event says what changed and how many items there now are. It carries no
//! clipboard content, so there is one set of rules about what a client may see —
//! the ordinary methods' — and a push channel cannot quietly become a second,
//! laxer one.

use copypaste_ipc::transport::OwnedWriteHalf;
use copypaste_ipc::{EventData, Response, ResponseData};
use tokio::sync::broadcast::{error::RecvError, Receiver};
use tokio::sync::watch;
use tracing::debug;

use super::listener::{send, Reader};

/// Send events until the client hangs up or the daemon stops.
///
/// `reader` is polled only so that a client going away is noticed promptly. A
/// subscriber may not issue further requests; anything it sends is ignored
/// rather than answered, because a reply interleaved with events would make the
/// stream ambiguous for no gain — a client that wants both opens two
/// connections.
pub(super) async fn pump(
    mut events: Receiver<EventData>,
    id: u64,
    reader: &mut Reader,
    writer: &mut OwnedWriteHalf,
    shutdown: &mut watch::Receiver<bool>,
) {
    loop {
        let event = tokio::select! {
            biased;
            _ = shutdown.changed() => break,
            next = reader.next() => match next {
                // EOF: the client closed the connection.
                None | Some(Err(_)) => break,
                Some(Ok(_)) => continue,
            },
            received = events.recv() => match received {
                Ok(event) => event,
                // The subscriber fell behind. The recovery for a lag is the
                // same as for any event — re-read — so one later event stands
                // in for the ones that were dropped and the loop continues.
                Err(RecvError::Lagged(skipped)) => {
                    debug!(skipped, "a watcher lagged; coalescing");
                    continue;
                }
                Err(RecvError::Closed) => break,
            },
        };

        if send(writer, &Response::ok(id, ResponseData::Event(event)))
            .await
            .is_err()
        {
            break;
        }
    }
}

/// Opt-in stream: legacy changes and separate runtime-only payloads share one
/// connection, while legacy watchers never see progress events.
pub(super) async fn pump_runtime(
    state: std::sync::Arc<crate::AppState>,
    mut events: Receiver<EventData>,
    mut runtime_events: Receiver<copypaste_ipc::RuntimeEventData>,
    id: u64,
    reader: &mut Reader,
    writer: &mut OwnedWriteHalf,
    shutdown: &mut watch::Receiver<bool>,
) {
    let mut transfers = state.store.subscribe_file_operations();
    if !send_capture_snapshot(&state, id, writer).await {
        return;
    }
    loop {
        let event = tokio::select! {
            biased;
            _ = shutdown.changed() => break,
            next = reader.next() => match next {
                None | Some(Err(_)) => break,
                Some(Ok(_)) => continue,
            },
            received = events.recv() => match received {
                Ok(change) => copypaste_ipc::RuntimeEventData { file_operation: None, change: Some(change), progress: None, capture: None },
                Err(RecvError::Lagged(_)) => {
                    if !send_recovery(&state, id, writer).await { break; }
                    continue;
                },
                Err(RecvError::Closed) => break,
            },
            changed = transfers.recv() => {
                if matches!(changed, Err(RecvError::Closed)) { break; }
                copypaste_ipc::RuntimeEventData { file_operation: Some(changed.unwrap_or_default()), change: None, capture: None, progress: None }
            },
            received = runtime_events.recv() => match received {
                Ok(event) => event,
                Err(RecvError::Lagged(_)) => {
                    if !send_recovery(&state, id, writer).await { break; }
                    continue;
                },
                Err(RecvError::Closed) => break,
            },
        };
        if send(writer, &Response::ok(id, ResponseData::RuntimeEvent(event)))
            .await
            .is_err()
        {
            break;
        }
    }
}

async fn send_capture_snapshot(
    state: &std::sync::Arc<crate::AppState>,
    id: u64,
    writer: &mut OwnedWriteHalf,
) -> bool {
    let state = std::sync::Arc::clone(state);
    let snapshot =
        tokio::task::spawn_blocking(move || crate::server::items::status(&state, 0).data).await;
    let Ok(Some(ResponseData::Status(status))) = snapshot else {
        return false;
    };
    let event = copypaste_ipc::RuntimeEventData {
        file_operation: None,
        change: None,
        progress: None,
        capture: Some(status),
    };
    send(writer, &Response::ok(id, ResponseData::RuntimeEvent(event)))
        .await
        .is_ok()
}

async fn send_recovery(
    state: &std::sync::Arc<crate::AppState>,
    id: u64,
    writer: &mut OwnedWriteHalf,
) -> bool {
    if !send_capture_snapshot(state, id, writer).await {
        return false;
    }
    let state = std::sync::Arc::clone(state);
    let Ok((item_count, sync_status)) = tokio::task::spawn_blocking(move || {
        let count = state.store.count().unwrap_or(0);
        let sync = state
            .p2p
            .node()
            .sync_status(state.settings.get().sync_enabled);
        (count, sync)
    })
    .await
    else {
        return false;
    };
    // A lost invalidation can cover both history and peers. Re-read both once.
    for kind in [
        copypaste_ipc::EventKind::Items,
        copypaste_ipc::EventKind::Peers,
    ] {
        let change = EventData {
            event: kind,
            item_count,
            captured: false,
            captured_item_id: None,
            sync_status: (kind == copypaste_ipc::EventKind::Peers).then(|| sync_status.clone()),
        };
        let event = copypaste_ipc::RuntimeEventData {
            file_operation: None,
            change: Some(change),
            progress: None,
            capture: None,
        };
        if send(writer, &Response::ok(id, ResponseData::RuntimeEvent(event)))
            .await
            .is_err()
        {
            return false;
        }
    }
    true
}

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

use std::sync::atomic::{AtomicU64, Ordering};

use copypaste_ipc::transport::OwnedWriteHalf;
use copypaste_ipc::{
    EventData, EventKindFilter, Method, Response, ResponseData, RuntimeEventData, MAX_EVENT_FILTERS,
};
use tokio::sync::broadcast::{error::RecvError, Receiver};
use tokio::sync::watch;
use tracing::debug;

use super::dispatch::{gate, parse, CallerIdentity};
use super::listener::{send, Reader};

/// What a `watch_events` subscription receives, and which connection it is.
pub(super) struct WatchFilter {
    kinds: Vec<EventKindFilter>,
    connection: u64,
    caller: CallerIdentity,
}

impl WatchFilter {
    /// `None` when the kind list is empty or longer than the shared cap.
    pub(super) fn new(kinds: Vec<EventKindFilter>, caller: CallerIdentity) -> Option<Self> {
        static NEXT_CONNECTION: AtomicU64 = AtomicU64::new(1);
        (!kinds.is_empty() && kinds.len() <= MAX_EVENT_FILTERS).then(|| Self {
            kinds,
            connection: NEXT_CONNECTION.fetch_add(1, Ordering::Relaxed),
            caller,
        })
    }

    pub(super) fn connection(&self) -> u64 {
        self.connection
    }

    fn admits(filter: Option<&Self>, event: &RuntimeEventData) -> bool {
        filter.is_none_or(|filter| EventKindFilter::admits(&filter.kinds, event))
    }

    fn wants_transfers(filter: Option<&Self>) -> bool {
        filter.is_none_or(|filter| filter.kinds.contains(&EventKindFilter::Transfers))
    }
}

/// Answer the one request a filtered subscriber may send: attaching an AI
/// session to this connection. Anything else stays ignored, as on `watch`.
async fn answer_attach(
    state: &std::sync::Arc<crate::AppState>,
    filter: &WatchFilter,
    line: &str,
    writer: &mut OwnedWriteHalf,
) -> bool {
    let Ok(request) = parse(line) else {
        return true;
    };
    let Method::Ai {
        op: copypaste_ipc::ai::AiOp::SessionAttach { session_id },
    } = &request.method
    else {
        return true;
    };
    let response = match gate(state, &request, filter.caller) {
        Some(rejection) => *rejection.response,
        None => state.attach_ai_session(request.id, filter.connection, session_id),
    };
    send(writer, &response).await.is_ok()
}

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
///
/// With a [`WatchFilter`] (`watch_events`) only the requested kinds are sent,
/// and file-operation changes are not even received unless asked for.
#[allow(clippy::too_many_arguments)]
pub(super) async fn pump_runtime(
    state: std::sync::Arc<crate::AppState>,
    mut events: Receiver<EventData>,
    mut runtime_events: Receiver<copypaste_ipc::RuntimeEventData>,
    id: u64,
    reader: &mut Reader,
    writer: &mut OwnedWriteHalf,
    shutdown: &mut watch::Receiver<bool>,
    filter: Option<&WatchFilter>,
) {
    let mut transfers =
        WatchFilter::wants_transfers(filter).then(|| state.store.subscribe_file_operations());
    if !send_capture_snapshot(&state, id, writer, filter).await {
        return;
    }
    loop {
        let event = tokio::select! {
            biased;
            _ = shutdown.changed() => break,
            next = reader.next() => match next {
                None | Some(Err(_)) => break,
                Some(Ok(line)) => {
                    if let Some(filter) = filter {
                        if !answer_attach(&state, filter, &line.text, writer).await { break; }
                    }
                    continue;
                }
            },
            received = events.recv() => match received {
                Ok(change) => copypaste_ipc::RuntimeEventData { file_operation: None, change: Some(change), progress: None, capture: None },
                Err(RecvError::Lagged(_)) => {
                    if !send_recovery(&state, id, writer, filter).await { break; }
                    continue;
                },
                Err(RecvError::Closed) => break,
            },
            changed = async {
                match transfers.as_mut() {
                    Some(transfers) => transfers.recv().await,
                    None => std::future::pending().await,
                }
            } => {
                if matches!(changed, Err(RecvError::Closed)) { break; }
                copypaste_ipc::RuntimeEventData { file_operation: Some(changed.unwrap_or_default()), change: None, capture: None, progress: None }
            },
            received = runtime_events.recv() => match received {
                Ok(event) => event,
                Err(RecvError::Lagged(_)) => {
                    if !send_recovery(&state, id, writer, filter).await { break; }
                    continue;
                },
                Err(RecvError::Closed) => break,
            },
        };
        if !send_runtime_event(writer, id, event, filter).await {
            break;
        }
    }
}

/// Send one frame unless the subscription filters it out.
async fn send_runtime_event(
    writer: &mut OwnedWriteHalf,
    id: u64,
    event: RuntimeEventData,
    filter: Option<&WatchFilter>,
) -> bool {
    if !WatchFilter::admits(filter, &event) {
        return true;
    }
    send(writer, &Response::ok(id, ResponseData::RuntimeEvent(event)))
        .await
        .is_ok()
}

async fn send_capture_snapshot(
    state: &std::sync::Arc<crate::AppState>,
    id: u64,
    writer: &mut OwnedWriteHalf,
    filter: Option<&WatchFilter>,
) -> bool {
    let state = std::sync::Arc::clone(state);
    let snapshot = tokio::task::spawn_blocking(move || {
        Some(copypaste_ipc::ResponseData::Status(state.status_data()))
    })
    .await;
    let Ok(Some(ResponseData::Status(status))) = snapshot else {
        return false;
    };
    let event = copypaste_ipc::RuntimeEventData {
        file_operation: None,
        change: None,
        progress: None,
        capture: Some(status),
    };
    send_runtime_event(writer, id, event, filter).await
}

async fn send_recovery(
    state: &std::sync::Arc<crate::AppState>,
    id: u64,
    writer: &mut OwnedWriteHalf,
    filter: Option<&WatchFilter>,
) -> bool {
    if !send_capture_snapshot(state, id, writer, filter).await {
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
            sync_status: (kind == copypaste_ipc::EventKind::Peers).then(|| sync_status.clone()),
            ..EventData::change(kind, item_count)
        };
        let event = copypaste_ipc::RuntimeEventData {
            file_operation: None,
            change: Some(change),
            progress: None,
            capture: None,
        };
        if !send_runtime_event(writer, id, event, filter).await {
            return false;
        }
    }
    true
}

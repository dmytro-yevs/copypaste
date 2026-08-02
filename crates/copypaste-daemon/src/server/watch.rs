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

use copypaste_ipc::{EventData, Response, ResponseData};
use futures_util::StreamExt;
use tokio::net::unix::OwnedWriteHalf;
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
                None => break,
                Some(_) => continue,
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

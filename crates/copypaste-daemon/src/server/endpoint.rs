//! What a connection becomes once it opened a module endpoint.
//!
//! After the `Connect` acknowledgement the connection carries only
//! `EndpointFrame` lines in both directions. Like a watcher it is exempt from
//! the read deadline and counted against its own cap; the write deadline still
//! applies. Client frames are delivered one at a time on a blocking worker
//! while module frames keep flowing to the client. Whoever closes, the module
//! receives `EndpointClose` exactly once and the instance lease is released.

use copypaste_ipc::transport::OwnedWriteHalf;
use copypaste_ipc::{EndpointFrame, MAX_DELIVERY_BYTES};
use copypaste_modules::EndpointSession;
use tokio::sync::watch;
use tracing::debug;

use super::listener::{send_value, Reader};

/// Relay frames until either side closes or the daemon stops.
pub(super) async fn pump(
    mut session: EndpointSession,
    reader: &mut Reader,
    writer: &mut OwnedWriteHalf,
    shutdown: &mut watch::Receiver<bool>,
) {
    let link = session.link();
    let mut delivering: Option<tokio::task::JoinHandle<bool>> = None;
    // `Some` sends a final `close` frame; `None` means the client is gone.
    let close: Option<Option<&str>> = loop {
        tokio::select! {
            biased;
            _ = shutdown.changed() => break Some(Some("CopyPaste is stopping.")),
            delivered = async { delivering.as_mut().expect("guarded").await }, if delivering.is_some() => {
                delivering = None;
                if !delivered.unwrap_or(false) {
                    break Some(Some("The module did not accept the message."));
                }
            }
            frame = session.outbound() => match frame {
                Some(data) => {
                    if send_value(writer, &EndpointFrame::Data { data }).await.is_err() {
                        break None;
                    }
                }
                None => break Some(None),
            },
            line = reader.next(), if delivering.is_none() => {
                let Some(Ok(line)) = line else { break None };
                if line.text.trim().is_empty() {
                    continue;
                }
                let frame = (line.text.len() <= MAX_DELIVERY_BYTES)
                    .then(|| serde_json::from_str::<EndpointFrame>(&line.text).ok())
                    .flatten();
                drop(line);
                match frame {
                    Some(EndpointFrame::Data { data }) => {
                        let link = link.clone();
                        delivering = Some(tokio::task::spawn_blocking(move || {
                            link.deliver(data).is_ok()
                        }));
                    }
                    Some(EndpointFrame::Close { .. }) => break None,
                    None => break Some(Some("The message is not a valid endpoint frame.")),
                }
            }
        }
    };
    if let Some(delivering) = delivering {
        let _ = delivering.await;
    }
    if let Some(reason) = close {
        let frame = EndpointFrame::Close {
            reason: reason.map(str::to_owned),
        };
        let _ = send_value(writer, &frame).await;
    }
    drop(session);
    // Delivering `EndpointClose` waits for the module like any other call.
    let _ = tokio::task::spawn_blocking(move || link.close()).await;
    debug!("module endpoint connection closed");
}

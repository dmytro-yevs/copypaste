//! The endpoint a daemon serves when its history cannot be opened at all.
//!
//! Exiting instead is what makes an unrecoverable startup failure look like a
//! crash. The app finds no socket, says "the background service is not
//! running", and offers **Start** — which starts a daemon that fails in exactly
//! the same way, forever. Holding the socket and answering with one code turns
//! that loop into a sentence the app can render, and stops launchd restarting a
//! process whose problem no restart touches.
//!
//! Only the failures in [`super::messages::Refusal`] come here. Everything else
//! still exits: a daemon that cannot bind its port or cannot build its detector
//! has no fixed sentence to offer, and pretending it has one would be worse
//! than the exit code.
//!
//! **Every request gets the same answer.** There is no history to read, no
//! settings to change and no identity to pair with, so a per-method reply would
//! be twenty spellings of one fact. `Shutdown` is the exception, for ADR-0004's
//! reason: an app that did not start this daemon cannot signal it, and a daemon
//! nobody can use must still be one somebody can stop.

use std::sync::Arc;

use copypaste_ipc::transport::{Listener, Stream};
use copypaste_ipc::{ErrorCode, Method, Request, Response, ResponseData, MAX_FRAME_BYTES};
use futures_util::StreamExt;
use tokio::sync::{watch, Semaphore};
use tokio_util::codec::{FramedRead, LinesCodec};
use tracing::{debug, warn};

use super::listener::{send, MAX_CONCURRENT_CONNECTIONS, READ_TIMEOUT};

/// Accept connections and refuse each one, until told to stop.
pub async fn serve(
    listener: Listener,
    refusal: (ErrorCode, &'static str),
    stop: watch::Sender<bool>,
) {
    let mut shutdown = stop.subscribe();
    let mut connections = tokio::task::JoinSet::new();
    let permits = Arc::new(Semaphore::new(MAX_CONCURRENT_CONNECTIONS));

    loop {
        tokio::select! {
            _ = shutdown.changed() => break,
            accepted = listener.accept() => match accepted {
                Ok(stream) => {
                    let Ok(permit) = Arc::clone(&permits).try_acquire_owned() else {
                        debug!("ipc connection cap reached; dropping a connection");
                        continue;
                    };
                    let stop = stop.clone();
                    connections.spawn(async move {
                        let _permit = permit;
                        refuse(stream, refusal, stop).await;
                    });
                }
                Err(e) => warn!(error = %e, "could not accept an ipc connection"),
            },
        }
    }

    connections.shutdown().await;
}

async fn refuse(
    stream: Stream,
    (code, message): (ErrorCode, &'static str),
    stop: watch::Sender<bool>,
) {
    let (reader, mut writer) = stream.into_split();
    let mut lines = FramedRead::new(reader, LinesCodec::new_with_max_length(MAX_FRAME_BYTES));

    loop {
        let Ok(next) = tokio::time::timeout(READ_TIMEOUT, lines.next()).await else {
            break;
        };
        let line = match next {
            Some(Ok(line)) => line,
            // A frame past the cap, a decode failure and EOF are all the same
            // here: there is nothing this daemon could have done with the
            // request anyway.
            _ => break,
        };
        if line.trim().is_empty() {
            continue;
        }

        // An unparseable line still gets the refusal rather than `malformed`:
        // the condition the caller needs to hear about is this one, and a
        // client cannot fix its request into a working call.
        let request: Option<Request> = serde_json::from_str(&line).ok();
        let id = request.as_ref().map_or(0, |request| request.id);

        if matches!(
            request.map(|request| request.method),
            Some(Method::Shutdown)
        ) {
            // Acknowledged first and acted on second, as `listener` does: a
            // client learns the request was accepted rather than inferring it
            // from a closed socket.
            let _ = send(&mut writer, &Response::ok(id, ResponseData::Empty {})).await;
            let _ = stop.send(true);
            break;
        }

        if send(&mut writer, &Response::err(id, code, message))
            .await
            .is_err()
        {
            break;
        }
    }
}

// Every test here binds a real endpoint, which only Unix has one of yet.
#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    use crate::server::messages::{Refusal, MSG_KEY_LOCKED};
    use copypaste_core::CryptoError;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixListener;

    /// Round-trip one request against a halted daemon on a real socket.
    async fn ask(request: &str) -> Response {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("daemon.sock");
        let listener: Listener = UnixListener::bind(&path).unwrap().into();
        let stop = watch::channel(false).0;
        let refusal = CryptoError::KeystoreUnavailable("locked")
            .refusal()
            .unwrap();
        let server = tokio::spawn(serve(listener, refusal, stop.clone()));

        let stream = tokio::net::UnixStream::connect(&path).await.unwrap();
        let (reader, mut writer) = stream.into_split();
        writer.write_all(request.as_bytes()).await.unwrap();
        writer.write_all(b"\n").await.unwrap();

        let mut line = String::new();
        BufReader::new(reader).read_line(&mut line).await.unwrap();

        let _ = stop.send(true);
        server.await.unwrap();
        serde_json::from_str(&line).unwrap()
    }

    #[tokio::test]
    async fn an_ordinary_request_is_refused_with_the_startup_code() {
        let response = ask(r#"{"id":7,"method":"list","params":{"limit":50}}"#).await;
        assert_eq!(response.id, 7);
        assert!(!response.ok);
        assert_eq!(response.error_code, Some(ErrorCode::KeyLocked));
        assert_eq!(response.error.as_deref(), Some(MSG_KEY_LOCKED));
    }

    /// `Status` is how a client asks what state the daemon is in, so answering
    /// it successfully would report a working service and send the app to the
    /// wrong screen.
    #[tokio::test]
    async fn status_is_refused_too_rather_than_reporting_health() {
        let response = ask(r#"{"id":1,"method":"status"}"#).await;
        assert!(!response.ok);
        assert_eq!(response.error_code, Some(ErrorCode::KeyLocked));
    }

    #[tokio::test]
    async fn a_malformed_line_hears_the_same_condition() {
        let response = ask("not json at all").await;
        assert_eq!(response.error_code, Some(ErrorCode::KeyLocked));
    }

    /// The one verb that still works, and the reason the socket is safe to
    /// hold: a daemon nobody can use must still be one somebody can stop.
    #[tokio::test]
    async fn shutdown_is_acknowledged_and_ends_the_server() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("daemon.sock");
        let listener: Listener = UnixListener::bind(&path).unwrap().into();
        let stop = watch::channel(false).0;
        let server = tokio::spawn(serve(
            listener,
            CryptoError::KeystoreUnavailable("locked")
                .refusal()
                .unwrap(),
            stop.clone(),
        ));

        let stream = tokio::net::UnixStream::connect(&path).await.unwrap();
        let (reader, mut writer) = stream.into_split();
        writer
            .write_all(b"{\"id\":3,\"method\":\"shutdown\"}\n")
            .await
            .unwrap();
        let mut line = String::new();
        BufReader::new(reader).read_line(&mut line).await.unwrap();
        let response: Response = serde_json::from_str(&line).unwrap();
        assert!(response.ok, "{line}");

        // The server stops on its own, without the test signalling it.
        tokio::time::timeout(std::time::Duration::from_secs(5), server)
            .await
            .expect("the halted server did not stop on a shutdown request")
            .unwrap();
        assert!(*stop.borrow());
    }
}

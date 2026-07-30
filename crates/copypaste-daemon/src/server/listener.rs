//! The socket itself: binding it, guarding it, and framing what arrives on it.
//!
//! Framing is `tokio_util::codec::LinesCodec`. v1 hand-rolled a two-pass,
//! byte-scanning partial-JSON reader — it had to, because it wanted a
//! method-aware size cap before the method was parseable. v2 has one cap
//! (`copypaste_ipc::MAX_FRAME_BYTES`) and therefore no reason to look at bytes
//! at all.
//!
//! Only the read half goes through the codec. `SinkExt` lives behind
//! futures-util's `sink` feature, which this crate's dependency line does not
//! enable, so a reply is written as `to_string` plus a newline straight to the
//! socket. That is not the thing v1 got wrong: the parsing was.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use copypaste_ipc::{ErrorCode, Response, MAX_FRAME_BYTES};
use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;
use tokio::net::unix::OwnedWriteHalf;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::watch;
use tokio_util::codec::{FramedRead, LinesCodec, LinesCodecError};
use tracing::{debug, error, info, warn};

use super::dispatch::dispatch_line;
use super::messages::MSG_TOO_LARGE;
use crate::AppState;

/// Create the socket directory, clear a stale socket, bind, and lock the socket
/// down to `0600`.
///
/// The socket is the only authentication boundary — there is no in-band auth
/// (manifest 04 I14) — so the `chmod` is a hard error, while tightening the
/// parent directory is warn-only (it may be a pre-existing shared data dir).
pub fn bind(path: &Path) -> anyhow::Result<UnixListener> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("create the socket directory")?;
        if let Err(e) = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700)) {
            warn!(error = %e, "could not restrict the socket directory to the owner");
        }
    }

    clear_stale_socket(path)?;
    let listener = UnixListener::bind(path).context("bind the daemon socket")?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .context("restrict the daemon socket to the owner")?;

    info!("ipc socket listening");
    Ok(listener)
}

/// A socket file left behind by a crashed daemon is removed; one with a live
/// listener behind it is not.
///
/// Refusing to steal a live socket is dual-daemon prevention: two daemons on
/// one database is a data-loss shape, and the second one exiting is the safe
/// outcome.
fn clear_stale_socket(path: &Path) -> anyhow::Result<()> {
    if std::fs::symlink_metadata(path).is_err() {
        return Ok(());
    }
    if std::os::unix::net::UnixStream::connect(path).is_ok() {
        anyhow::bail!("another copypaste daemon is already listening on the socket");
    }
    std::fs::remove_file(path).context("remove the stale socket")?;
    debug!("removed a stale socket file");
    Ok(())
}

/// Accept connections until shutdown.
pub async fn run(
    listener: UnixListener,
    state: Arc<AppState>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut connections = tokio::task::JoinSet::new();

    loop {
        tokio::select! {
            _ = shutdown.changed() => break,
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => {
                    let state = Arc::clone(&state);
                    connections.spawn(async move { handle_connection(stream, state).await });
                }
                Err(e) => warn!(error = %e, "could not accept an ipc connection"),
            },
        }
    }

    connections.shutdown().await;
}

/// One connection: read lines, answer each with exactly one line, until EOF.
async fn handle_connection(stream: UnixStream, state: Arc<AppState>) {
    let (reader, mut writer) = stream.into_split();
    let mut lines = FramedRead::new(reader, LinesCodec::new_with_max_length(MAX_FRAME_BYTES));

    loop {
        let line = match lines.next().await {
            None => break,
            Some(Ok(line)) => line,
            Some(Err(LinesCodecError::MaxLineLengthExceeded)) => {
                // The id lived somewhere in the discarded bytes, so `0` is the
                // best that can be echoed. The codec resumes at the next
                // newline, so the connection stays usable.
                let response = Response::err(0, ErrorCode::InvalidRequest, MSG_TOO_LARGE);
                if send(&mut writer, &response).await.is_err() {
                    break;
                }
                continue;
            }
            Some(Err(e)) => {
                debug!(error = %e, "ipc connection read failed");
                break;
            }
        };

        // Blank lines are keep-alives.
        if line.trim().is_empty() {
            continue;
        }

        let response = dispatch_line(&state, &line).await;

        if send(&mut writer, &response).await.is_err() {
            break;
        }
    }
}

/// One request, one line, one response line.
async fn send(writer: &mut OwnedWriteHalf, response: &Response) -> Result<(), ()> {
    let mut encoded = match serde_json::to_string(response) {
        Ok(encoded) => encoded,
        Err(e) => {
            error!(error = %e, "could not serialise a response");
            return Err(());
        }
    };
    encoded.push('\n');
    writer.write_all(encoded.as_bytes()).await.map_err(|e| {
        debug!(error = %e, "ipc connection write failed");
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2p::tests::test_state;
    use copypaste_ipc::{Method, Request, ResponseData, PROTOCOL_VERSION};

    fn request(id: u64, method: Method) -> Request {
        Request {
            id,
            protocol_version: PROTOCOL_VERSION,
            method,
        }
    }

    #[tokio::test]
    async fn a_stale_socket_file_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("daemon.sock");
        std::fs::write(&path, b"left behind by a crashed daemon").unwrap();

        let listener = bind(&path).expect("bind must clear the stale file");
        drop(listener);
    }

    #[tokio::test]
    async fn the_socket_is_owner_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("daemon.sock");
        let _listener = bind(&path).expect("bind");

        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "the socket is the only auth boundary");
    }

    /// Full round trip: a real socket, a real client, one connection, several
    /// pipelined requests.
    #[tokio::test]
    async fn requests_round_trip_over_a_socket() {
        let (state, dir) = test_state("server");
        let path = dir.path().join("daemon.sock");
        let listener = bind(&path).expect("bind");

        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let server = tokio::spawn(run(listener, Arc::clone(&state), shutdown_rx));

        let stream = UnixStream::connect(&path).await.expect("connect");
        let mut client = Client::new(stream);

        // status answers, and reports the fake backend rather than a real one.
        let response = client.call(request(1, Method::Status)).await;
        assert!(response.ok);
        assert_eq!(response.id, 1);
        match response.data {
            Some(ResponseData::Status(status)) => {
                assert_eq!(status.protocol_version, PROTOCOL_VERSION);
                assert_eq!(status.clipboard_backend, "fake");
            }
            other => panic!("expected status data, got {other:?}"),
        }

        // add stores an item and returns it decrypted.
        let response = client
            .call(request(
                2,
                Method::Add {
                    content: "round trip".into(),
                },
            ))
            .await;
        assert!(response.ok, "add failed: {:?}", response.error);
        let added = match response.data {
            Some(ResponseData::Item(item)) => item,
            other => panic!("expected an item, got {other:?}"),
        };
        assert_eq!(added.content, "round trip");
        assert!(!added.is_sensitive);

        // list sees it.
        let response = client
            .call(request(
                3,
                Method::List {
                    limit: 10,
                    offset: 0,
                },
            ))
            .await;
        let items = match response.data {
            Some(ResponseData::Items(items)) => items,
            other => panic!("expected items, got {other:?}"),
        };
        assert!(items.iter().any(|item| item.id == added.id));

        // copy writes to the clipboard source.
        let response = client
            .call(request(
                4,
                Method::Copy {
                    id: added.id.clone(),
                },
            ))
            .await;
        assert!(response.ok, "copy failed: {:?}", response.error);

        // pin round-trips through the store.
        let response = client
            .call(request(
                5,
                Method::Pin {
                    id: added.id.clone(),
                    pinned: true,
                },
            ))
            .await;
        match response.data {
            Some(ResponseData::Item(item)) => assert!(item.pinned),
            other => panic!("expected the updated item, got {other:?}"),
        }

        // An unknown id is not_found, not a silent success.
        let response = client
            .call(request(
                6,
                Method::Delete {
                    id: "00000000-0000-0000-0000-000000000000".into(),
                },
            ))
            .await;
        assert!(!response.ok);
        assert_eq!(response.error_code, Some(ErrorCode::NotFound));

        // A mismatched protocol version is rejected before any handler runs.
        let mismatched = serde_json::json!({
            "id": 7,
            "protocol_version": PROTOCOL_VERSION + 1,
            "method": "status",
        });
        let response = client.call_raw(&mismatched.to_string()).await;
        assert_eq!(response.id, 7);
        assert_eq!(response.error_code, Some(ErrorCode::ProtocolMismatch));

        // Garbage is answered, not fatal to the connection.
        let response = client.call_raw("not json").await;
        assert_eq!(response.error_code, Some(ErrorCode::InvalidRequest));

        // ...and the connection still works afterwards.
        let response = client.call(request(9, Method::Status)).await;
        assert!(response.ok);
        assert_eq!(response.id, 9);

        shutdown_tx.send(true).unwrap();
        let _ = server.await;
    }

    /// A minimal newline-JSON client, mirroring what the CLI does.
    struct Client {
        writer: OwnedWriteHalf,
        lines: FramedRead<tokio::net::unix::OwnedReadHalf, LinesCodec>,
    }

    impl Client {
        fn new(stream: UnixStream) -> Self {
            let (reader, writer) = stream.into_split();
            Self {
                writer,
                lines: FramedRead::new(reader, LinesCodec::new()),
            }
        }

        async fn call(&mut self, request: Request) -> Response {
            self.call_raw(&serde_json::to_string(&request).unwrap())
                .await
        }

        async fn call_raw(&mut self, line: &str) -> Response {
            self.writer.write_all(line.as_bytes()).await.unwrap();
            self.writer.write_all(b"\n").await.unwrap();
            let reply = self
                .lines
                .next()
                .await
                .expect("a reply")
                .expect("a valid frame");
            serde_json::from_str(&reply).expect("a Response")
        }
    }
}

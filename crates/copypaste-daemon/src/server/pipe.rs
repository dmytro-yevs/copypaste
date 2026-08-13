//! Binding the daemon's owner-only Windows pipe.

use std::io;
use std::path::Path;

use copypaste_ipc::transport::{pipe, Listener};
use interprocess::os::windows::{
    named_pipe::{pipe_mode::Bytes, PipeListenerOptions},
    security_descriptor::SecurityDescriptor,
};
use tracing::info;
use widestring::U16CString;
use win_security_identifier::{GetCurrentSid, SecurityIdentifier};

/// Create the pipe, restricted to this account, and refuse to be the second
/// daemon on it.
pub fn bind(path: &Path) -> anyhow::Result<Listener> {
    let name = pipe::name_for(path);
    let security = owner_only()?;

    let listener = PipeListenerOptions::new()
        .path(name.as_os_str())
        .accept_remote(false)
        .security_descriptor(Some(security))
        .create_tokio_duplex::<Bytes>()
        .map_err(|e| {
            if e.kind() == io::ErrorKind::PermissionDenied {
                anyhow::anyhow!("another copypaste daemon is already listening")
            } else {
                anyhow::Error::new(e).context("create the daemon pipe")
            }
        })?;

    info!("ipc pipe listening");
    Ok(listener.into())
}

fn owner_only() -> anyhow::Result<SecurityDescriptor> {
    let sid = SecurityIdentifier::get_current_user_sid()
        .map_err(|_| anyhow::anyhow!("could not read this account's identifier"))?;
    let text = U16CString::from_str(sddl(&sid.to_string()))
        .map_err(|_| anyhow::anyhow!("could not build the daemon pipe's access list"))?;
    SecurityDescriptor::deserialize(&text)
        .map_err(|_| anyhow::anyhow!("could not build the daemon pipe's access list"))
}

/// One allow-everything entry for this account, in a protected DACL.
fn sddl(sid: &str) -> String {
    format!("D:P(A;;GA;;;{sid})")
}

// The framing, the caps and the deadlines are `super::listener`'s, and its own
// test module is `#[cfg(unix)]` — so on Windows they are reached only from
// here. The oversized-line path was reached from nowhere at all.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::listener::{MAX_CONCURRENT_CONNECTIONS, READ_TIMEOUT};
    use crate::server::messages::MSG_TOO_LARGE;
    use crate::testutil::test_state;
    use copypaste_ipc::{
        transport, ErrorCode, Method, Request, Response, MAX_FRAME_BYTES, PROTOCOL_VERSION,
    };
    use futures_util::StreamExt;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::io::AsyncWriteExt;
    use tokio_util::codec::{FramedRead, LinesCodec};

    type Lines = FramedRead<transport::OwnedReadHalf, LinesCodec>;

    /// A newline-JSON client, as the CLI is.
    struct Client {
        writer: transport::OwnedWriteHalf,
        lines: Lines,
    }

    impl Client {
        async fn connect(path: &Path) -> Self {
            let (reader, writer) = transport::connect(path)
                .await
                .expect("connect")
                .into_split();
            Self {
                writer,
                lines: FramedRead::new(reader, LinesCodec::new_with_max_length(MAX_FRAME_BYTES)),
            }
        }

        async fn send(&mut self, line: &[u8]) {
            self.writer.write_all(line).await.expect("send");
            self.writer.write_all(b"\n").await.expect("send");
        }

        async fn status(&mut self, id: u64) -> Response {
            let request = Request {
                id,
                protocol_version: PROTOCOL_VERSION,
                method: Method::Status,
            };
            self.send(serde_json::to_string(&request).unwrap().as_bytes())
                .await;
            self.reply().await
        }

        async fn reply(&mut self) -> Response {
            reply(&mut self.lines).await
        }
    }

    async fn reply(lines: &mut Lines) -> Response {
        let line = tokio::time::timeout(Duration::from_secs(10), lines.next())
            .await
            .expect("a reply")
            .expect("a frame")
            .expect("a valid frame");
        serde_json::from_str(&line).expect("a Response")
    }

    #[test]
    fn the_pipe_admits_the_owner_and_nobody_else() {
        let rule = sddl("S-1-5-21-1-2-3-1001");
        assert_eq!(rule, "D:P(A;;GA;;;S-1-5-21-1-2-3-1001)");
        for wider in ["WD", "BA", "SY", "AU", "IU"] {
            assert!(!rule.contains(wider), "{rule} admits {wider}");
        }
    }

    #[test]
    fn this_account_has_an_identifier_the_parser_accepts() {
        owner_only().expect("the descriptor must parse");
    }

    #[tokio::test]
    async fn a_second_daemon_on_one_pipe_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("daemon.sock");

        let first = bind(&path).expect("the first daemon binds");
        let second = bind(&path).expect_err("the second must not");

        let shown = format!("{second:#}");
        assert!(shown.contains("already listening"), "{shown}");
        assert!(!shown.contains('\\'), "rule 4: no path in a user message");

        drop(first);
        bind(&path).expect("the name is free once the listener is dropped");
    }

    #[tokio::test]
    async fn two_data_directories_bind_at_the_same_time() {
        let dir = tempfile::tempdir().unwrap();
        let one = bind(&dir.path().join("a/daemon.sock")).expect("bind");
        let two = bind(&dir.path().join("b/daemon.sock")).expect("bind");
        drop((one, two));
    }

    #[tokio::test]
    async fn a_request_round_trips_over_the_pipe() {
        let (state, dir) = test_state("pipe");
        let path = dir.path().join("daemon.sock");
        let listener = bind(&path).expect("bind");
        let server = tokio::spawn(super::super::run(
            listener,
            Arc::clone(&state),
            state.shutdown_rx(),
        ));

        let mut client = Client::connect(&path).await;
        let response = client.status(1).await;
        assert!(response.ok, "{:?}", response.error);
        assert_eq!(response.id, 1);

        state.request_shutdown();
        let _ = tokio::time::timeout(Duration::from_secs(5), server).await;
    }

    #[tokio::test]
    async fn a_second_client_connects_while_the_first_is_open() {
        let (state, dir) = test_state("pipe-second");
        let path = dir.path().join("daemon.sock");
        let listener = bind(&path).expect("bind");
        let server = tokio::spawn(super::super::run(
            listener,
            Arc::clone(&state),
            state.shutdown_rx(),
        ));

        let first = transport::connect(&path).await.expect("connect");
        let second = tokio::time::timeout(Duration::from_secs(5), transport::connect(&path))
            .await
            .expect("the next instance must already be listening")
            .expect("connect");

        drop((first, second));
        state.request_shutdown();
        let _ = tokio::time::timeout(Duration::from_secs(5), server).await;
    }

    /// Manifest 04 §6.2 over the pipe: the refusal is delivered, it names no
    /// path, and the connection is then closed. `listener` used to resume at
    /// the next newline instead; the pipe could not keep that promise, and
    /// DMY-158 settled it the other way, so this and
    /// `listener::tests::an_oversized_line_is_refused_and_the_connection_is_closed`
    /// now assert one behaviour on two transports.
    ///
    /// The reply echoes id `0` because the real id was in the discarded bytes.
    #[tokio::test]
    async fn an_oversized_line_is_refused_and_the_connection_is_closed() {
        let (state, dir) = test_state("pipe-oversized");
        let path = dir.path().join("daemon.sock");
        let listener = bind(&path).expect("bind");
        let server = tokio::spawn(super::super::run(
            listener,
            Arc::clone(&state),
            state.shutdown_rx(),
        ));

        let mut client = Client::connect(&path).await;
        assert!(client.status(1).await.ok);

        // The refusal is sent as soon as the cap is passed, which is while the
        // client is still writing, so the two have to be concurrent. The write
        // itself may end in a broken pipe — that is the connection closing
        // under it, which is the point.
        let Client {
            mut writer,
            mut lines,
        } = client;
        let overshoot = tokio::spawn(async move {
            writer.write_all(&vec![b'a'; MAX_FRAME_BYTES + 1]).await?;
            writer
                .write_all(
                    b"
",
                )
                .await
        });
        let response = reply(&mut lines).await;
        assert!(!response.ok);
        assert_eq!(response.id, 0);
        assert_eq!(response.error_code, Some(ErrorCode::InvalidRequest));
        assert_eq!(response.error.as_deref(), Some(MSG_TOO_LARGE));
        assert!(
            !response.error.unwrap_or_default().contains('\\'),
            "rule 4: no path in a user message"
        );

        let next = tokio::time::timeout(Duration::from_secs(10), lines.next())
            .await
            .expect("the refused connection must not hang");
        assert!(
            matches!(next, None | Some(Err(_))),
            "the refusal must be the last frame on this connection: {next:?}"
        );
        let _ = tokio::time::timeout(Duration::from_secs(10), overshoot).await;

        let mut fresh = Client::connect(&path).await;
        let response = fresh.status(3).await;
        assert!(response.ok, "one client's oversized frame ended the daemon");
        assert_eq!(response.id, 3);

        state.request_shutdown();
        let _ = tokio::time::timeout(Duration::from_secs(10), server).await;
    }

    /// A client that stops reading is not a client the daemon waits on: it sees
    /// EOF, which is what its retry already handles, and no response at all
    /// (`CopyPaste-cce1`). The clock is virtual, so this costs nothing.
    #[tokio::test(start_paused = true)]
    async fn a_silent_client_is_closed_at_the_read_deadline_without_a_reply() {
        let (state, dir) = test_state("pipe-idle");
        let path = dir.path().join("daemon.sock");
        let listener = bind(&path).expect("bind");
        let server = tokio::spawn(super::super::run(
            listener,
            Arc::clone(&state),
            state.shutdown_rx(),
        ));

        let mut client = Client::connect(&path).await;
        tokio::time::advance(READ_TIMEOUT + Duration::from_secs(1)).await;

        let next = tokio::time::timeout(Duration::from_secs(60), client.lines.next())
            .await
            .expect("the daemon must not hold an idle connection open");
        assert!(
            matches!(next, None | Some(Err(_))),
            "an idle connection was answered: {next:?}"
        );

        state.request_shutdown();
        let _ = tokio::time::timeout(Duration::from_secs(60), server).await;
    }

    /// EOF is an ordinary end, not an error: the daemon closes its side and
    /// keeps serving, which is what makes a CLI invocation per command cheap.
    #[tokio::test]
    async fn a_client_that_hangs_up_ends_its_connection_and_no_others() {
        let (state, dir) = test_state("pipe-eof");
        let path = dir.path().join("daemon.sock");
        let listener = bind(&path).expect("bind");
        let server = tokio::spawn(super::super::run(
            listener,
            Arc::clone(&state),
            state.shutdown_rx(),
        ));

        let mut long_lived = Client::connect(&path).await;
        assert!(long_lived.status(1).await.ok);

        let mut departing = Client::connect(&path).await;
        assert!(departing.status(2).await.ok);
        drop(departing);

        let response = long_lived.status(3).await;
        assert!(response.ok, "another client's EOF ended this connection");
        assert_eq!(response.id, 3);

        state.request_shutdown();
        let _ = tokio::time::timeout(Duration::from_secs(10), server).await;
    }

    /// `CopyPaste-6ot5`: the permits are taken without blocking, so the
    /// connection past the cap is dropped rather than queued behind one that
    /// may never come free. Which symptom the client sees — a broken write, a
    /// reset, or a clean EOF — is timing; that it is never *served* is not.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn connections_past_the_cap_are_dropped_rather_than_queued() {
        let (state, dir) = test_state("pipe-cap");
        let path = dir.path().join("daemon.sock");
        let listener = bind(&path).expect("bind");
        let server = tokio::spawn(super::super::run(
            listener,
            Arc::clone(&state),
            state.shutdown_rx(),
        ));

        // Each one answers first: a connect alone only reaches the pipe's
        // pending instance, so without a round trip this races the accept loop
        // rather than testing the cap.
        let mut idle = Vec::new();
        for n in 0..MAX_CONCURRENT_CONNECTIONS {
            let mut client = Client::connect(&path).await;
            assert!(client.status(n as u64 + 100).await.ok);
            idle.push(client);
        }

        let mut client = Client::connect(&path).await;
        let request = Request {
            id: 1,
            protocol_version: PROTOCOL_VERSION,
            method: Method::Status,
        };
        let line = serde_json::to_string(&request).unwrap();
        let served = tokio::time::timeout(Duration::from_secs(10), async {
            client.writer.write_all(line.as_bytes()).await?;
            client.writer.write_all(b"\n").await?;
            Ok::<_, std::io::Error>(client.lines.next().await)
        })
        .await
        .expect("the daemon must not leave an over-cap client hanging");
        assert!(
            matches!(served, Err(_) | Ok(None) | Ok(Some(Err(_)))),
            "an over-cap connection was served: {served:?}"
        );

        drop(idle);
        state.request_shutdown();
        let _ = tokio::time::timeout(Duration::from_secs(10), server).await;
    }
}

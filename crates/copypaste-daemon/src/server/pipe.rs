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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::test_state;
    use copypaste_ipc::{transport, Method, Request, Response, PROTOCOL_VERSION};
    use futures_util::StreamExt;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::io::AsyncWriteExt;
    use tokio_util::codec::{FramedRead, LinesCodec};

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

        let stream = transport::connect(&path).await.expect("connect");
        let (reader, mut writer) = stream.into_split();
        let mut lines = FramedRead::new(reader, LinesCodec::new());
        let request = Request {
            id: 1,
            protocol_version: PROTOCOL_VERSION,
            method: Method::Status,
        };
        let mut line = serde_json::to_string(&request).unwrap();
        line.push('\n');
        writer.write_all(line.as_bytes()).await.expect("send");

        let line = tokio::time::timeout(Duration::from_secs(5), lines.next())
            .await
            .expect("a reply")
            .expect("a frame")
            .expect("a valid frame");
        let response: Response = serde_json::from_str(&line).unwrap();
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
}

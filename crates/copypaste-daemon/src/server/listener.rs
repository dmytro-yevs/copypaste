//! The socket itself: binding it, guarding it, bounding it, and framing what
//! arrives on it.
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
//! socket.
//!
//! # Three bounds, and what each is for
//!
//! * **`flock` around bind** (`CopyPaste-ah1m`) — probe, remove, bind is a
//!   TOCTOU window: a second starter can unlink a socket the first has just
//!   bound and put its own there, and two daemons on one SQLCipher database is
//!   a data-loss shape.
//! * **A connection cap** (`CopyPaste-6ot5`) — 64 permits, acquired
//!   non-blockingly. Queueing behind a full cap only moves the stall to the
//!   accept loop.
//! * **Read and write deadlines** (`CopyPaste-cce1`, `CopyPaste-c4q2.24`) — a
//!   same-UID client that connects and never drains otherwise pins a permit and
//!   the database mutex for as long as it likes.

use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use copypaste_ipc::{ErrorCode, Method, Response, ResponseData, MAX_FRAME_BYTES};
use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{watch, Semaphore};
use tokio_util::codec::{FramedRead, LinesCodec, LinesCodecError};
use tracing::{debug, error, info, warn};

use super::dispatch::dispatch_line;
use super::messages::{MSG_TOO_LARGE, MSG_WATCHERS_FULL};
use crate::AppState;

/// Connections served at once (manifest 04 §3.5). Over-cap connections are
/// dropped at the OS level with no error frame: an unauthenticated dialler
/// learns nothing from a refusal that it did not already know from the socket
/// being there, and writing an error needs a task, which is the resource being
/// rationed.
pub(super) const MAX_CONCURRENT_CONNECTIONS: usize = 64;

/// How many of those may be `watch` subscribers.
///
/// A watcher is exempt from the read deadline, so it holds its permit for as
/// long as it likes; without a cap of its own, idle subscribers could take the
/// whole connection budget. Eight is several app windows and a CLI `watch`.
const MAX_WATCHERS: usize = 8;

/// Per-read deadline. A connection that says nothing for this long is dropped
/// **without a response** — the client sees EOF, which its retry logic already
/// handles (manifest 04 §3.4).
pub(super) const READ_TIMEOUT: Duration = Duration::from_secs(30);

/// Per-write deadline. Shorter than the read deadline because a stalled write
/// means the client is not draining, and the daemon is holding a reply.
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);

/// Create the socket directory, clear a stale socket, and bind.
///
/// Tightening the parent directory stays warn-only — it may be a pre-existing
/// shared data dir — because [`bind_owner_only`] does not rely on it.
pub fn bind(path: &Path) -> anyhow::Result<UnixListener> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("create the socket directory")?;
        if let Err(e) = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700)) {
            warn!(error = %e, "could not restrict the socket directory to the owner");
        }
    }

    // Held for the whole probe → remove → bind sequence and released when it
    // goes out of scope. Two starters therefore serialise here, and the loser
    // finds a live socket rather than a stale one (`CopyPaste-ah1m`).
    let _guard = BindLock::acquire(path)?;

    clear_stale_socket(path)?;
    let listener = bind_owner_only(path)?;

    info!("ipc socket listening");
    Ok(listener)
}

/// Bind the socket already restricted to `0600` (security review F-9).
///
/// `bind(2)` takes no mode, so a socket bound at its final path is
/// `0777 & ~umask` until a `chmod` lands — 0755 under the usual umask, and
/// `connect(2)` needs only write permission. The socket is the only
/// authentication boundary (manifest 04 I14), so it is bound inside a `0700`
/// staging directory and renamed into place: the rename is atomic, so the
/// final path never exists at any other mode. Not `umask(2)`, which is
/// process-wide and would apply to whatever other threads create meanwhile.
fn bind_owner_only(path: &Path) -> anyhow::Result<UnixListener> {
    let staging = staging_dir(path);
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&staging)
        .context("create the socket staging directory")?;
    // mkdir(2) subtracts the umask, so the directory is never wider than 0700
    // but can land narrower than this process can traverse.
    std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o700))
        .context("restrict the socket staging directory to the owner")?;

    let bound = bind_staged(&staging.join("s"), path);
    let _ = std::fs::remove_dir_all(&staging);
    bound
}

fn bind_staged(staged: &Path, path: &Path) -> anyhow::Result<UnixListener> {
    let listener = UnixListener::bind(staged).context("bind the daemon socket")?;
    std::fs::set_permissions(staged, std::fs::Permissions::from_mode(0o600))
        .context("restrict the daemon socket to the owner")?;
    std::fs::rename(staged, path).context("move the daemon socket into place")?;
    Ok(listener)
}

/// A sibling of the socket, so the rename into place stays within one
/// filesystem. Short names because `sun_path` is 104 bytes on macOS and the
/// staged path has to fit as well.
fn staging_dir(socket_path: &Path) -> PathBuf {
    let mut name = socket_path.as_os_str().to_os_string();
    name.push(".new");
    PathBuf::from(name)
}

/// An exclusive `flock(2)` on `<socket>.lock`, held for the bind sequence.
///
/// The lock file is created and never removed: unlinking it would let a second
/// starter create and lock a *different* inode while the first still holds the
/// old one, which is the race this exists to close.
struct BindLock {
    _file: std::fs::File,
}

impl BindLock {
    fn acquire(socket_path: &Path) -> anyhow::Result<Self> {
        let mut name = socket_path.as_os_str().to_os_string();
        name.push(".lock");
        let path = PathBuf::from(name);

        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .context("open the bind lock")?;
        // Blocking, not `NonBlocking`: the sequence it guards is three syscalls
        // and the loser is about to be told the socket is taken anyway. Failing
        // fast here would just reintroduce the race under a different name.
        rustix::fs::flock(&file, rustix::fs::FlockOperation::LockExclusive)
            .context("lock the bind lock")?;
        Ok(Self { _file: file })
    }
}

/// A socket file left behind by a crashed daemon is removed; one with a live
/// listener behind it is not.
///
/// Refusing to steal a live socket is dual-daemon prevention: two daemons on
/// one database is a data-loss shape, and the second one exiting is the safe
/// outcome. Correct only while [`BindLock`] is held — without it, the connect
/// probe and the `remove_file` straddle the moment another starter binds.
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
    let permits = Arc::new(Semaphore::new(MAX_CONCURRENT_CONNECTIONS));
    let watchers = Arc::new(Semaphore::new(MAX_WATCHERS));

    loop {
        tokio::select! {
            _ = shutdown.changed() => break,
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => {
                    // Non-blocking: waiting for a permit here would stop the
                    // accept loop, which turns a busy daemon into an
                    // unreachable one (`CopyPaste-6ot5`).
                    let Ok(permit) = Arc::clone(&permits).try_acquire_owned() else {
                        debug!("ipc connection cap reached; dropping a connection");
                        continue;
                    };
                    let state = Arc::clone(&state);
                    let watchers = Arc::clone(&watchers);
                    let shutdown = shutdown.clone();
                    connections.spawn(async move {
                        let _permit = permit;
                        handle_connection(stream, state, watchers, shutdown).await;
                    });
                }
                Err(e) => warn!(error = %e, "could not accept an ipc connection"),
            },
        }
    }

    connections.shutdown().await;
}

/// One connection: read lines, answer each with exactly one line, until EOF —
/// or until it subscribes, after which it only writes.
async fn handle_connection(
    stream: UnixStream,
    state: Arc<AppState>,
    watchers: Arc<Semaphore>,
    mut shutdown: watch::Receiver<bool>,
) {
    let (reader, mut writer) = stream.into_split();
    let mut lines = FramedRead::new(reader, LinesCodec::new_with_max_length(MAX_FRAME_BYTES));

    loop {
        // A client that connects and then says nothing holds a permit and, if
        // it is mid-request, the database mutex. The deadline is per read, so a
        // client that keeps talking is never cut off (`CopyPaste-cce1`).
        let next = match tokio::time::timeout(READ_TIMEOUT, lines.next()).await {
            Ok(next) => next,
            Err(_) => {
                // Deliberately no response: the client sees EOF, which its
                // retry logic already handles (manifest 04 §3.4).
                debug!("ipc connection idle past the read deadline; closing");
                break;
            }
        };

        let line = match next {
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

        // `shutdown` is taken here for the same reason `watch` is below: it
        // changes what the connection is — this one ends, and so does the
        // process. Handling it in the dispatcher could not order the two
        // correctly, because `run` aborts the connection tasks as soon as it
        // sees the signal, and the reply would be the thing aborted. Here the
        // acknowledgement is written *first*, and only then is the signal sent.
        if let Some(id) = shutdown_id(&line) {
            info!("shutdown requested over ipc");
            let _ = send(&mut writer, &Response::ok(id, ResponseData::Empty {})).await;
            state.request_shutdown();
            break;
        }

        // `watch` turns the connection into a stream, so it is taken here
        // rather than in the dispatcher: from this point the loop above no
        // longer owns the read half's deadline.
        if let Some(id) = subscribe_id(&line) {
            let Ok(_permit) = Arc::clone(&watchers).try_acquire_owned() else {
                let full = Response::err(id, ErrorCode::Internal, MSG_WATCHERS_FULL);
                let _ = send(&mut writer, &full).await;
                break;
            };
            // Subscribed *before* the acknowledgement, so the ack is a promise:
            // any change after a client sees it reaches this connection. Taking
            // the receiver afterwards would drop everything that happened in
            // between, which is a race a client cannot see or retry around.
            let events = state.subscribe();
            if send(&mut writer, &Response::ok(id, ResponseData::Empty {}))
                .await
                .is_err()
            {
                break;
            }
            super::watch::pump(events, id, &mut lines, &mut writer, &mut shutdown).await;
            break;
        }

        let response = dispatch_line(&state, &line).await;

        if send(&mut writer, &response).await.is_err() {
            break;
        }
    }
}

/// The request id when this line is a `watch` subscription, else `None`.
///
/// Parsed here rather than dispatched because the answer changes what the
/// connection *is*. A malformed line is left alone: the ordinary dispatcher
/// already knows how to reject one, and duplicating that judgement is how two
/// parsers disagree.
fn subscribe_id(line: &str) -> Option<u64> {
    let request: copypaste_ipc::Request = serde_json::from_str(line).ok()?;
    matches!(request.method, Method::Watch).then_some(request.id)
}

/// The request id when this line asks the daemon to stop, else `None`.
///
/// Deliberately not gated on readiness or on the protocol version. A client
/// that speaks a version this daemon does not is *the* caller for this verb —
/// that is the whole of ADR-0004's protocol-mismatch state — and refusing it
/// with `protocol_mismatch` would leave the mismatch with no way out. The
/// request shape is one word and cannot drift, and the socket is `0600`, so
/// there is nothing here a version could disagree about.
fn shutdown_id(line: &str) -> Option<u64> {
    let request: copypaste_ipc::Request = serde_json::from_str(line).ok()?;
    matches!(request.method, Method::Shutdown).then_some(request.id)
}

/// One request, one line, one response line.
pub(super) async fn send(writer: &mut OwnedWriteHalf, response: &Response) -> Result<(), ()> {
    let mut encoded = match serde_json::to_string(response) {
        Ok(encoded) => encoded,
        Err(e) => {
            error!(error = %e, "could not serialise a response");
            return Err(());
        }
    };
    encoded.push('\n');
    // A client that has stopped reading fills the socket buffer and this write
    // never returns. The task holds a connection permit while it waits
    // (`CopyPaste-c4q2.24`).
    match tokio::time::timeout(WRITE_TIMEOUT, writer.write_all(encoded.as_bytes())).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => {
            debug!(error = %e, "ipc connection write failed");
            Err(())
        }
        Err(_) => {
            debug!("ipc connection write deadline passed; closing");
            Err(())
        }
    }
}

/// The read half, once a connection has subscribed.
pub(super) type Reader = FramedRead<OwnedReadHalf, LinesCodec>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::test_state;
    use copypaste_ipc::{Method, Request, PROTOCOL_VERSION};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Barrier;

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

    /// `CopyPaste-ah1m`, and the reason the single-starter test above was not
    /// enough. Several starters race on one stale socket; exactly one may win,
    /// and the socket that is left on disk must be the winner's — not a file
    /// some loser unlinked and rebound after the winner was already listening.
    ///
    /// Ten rounds because the unguarded window is a few syscalls wide: one round
    /// can pass by luck, ten do not.
    #[test]
    fn two_starters_racing_on_one_socket_produce_exactly_one_daemon() {
        const STARTERS: usize = 12;
        const ROUNDS: usize = 10;

        let dir = tempfile::tempdir().unwrap();
        for round in 0..ROUNDS {
            let path = dir.path().join(format!("daemon-{round}.sock"));
            // The state every starter finds after a crash.
            std::fs::write(&path, b"left behind by a crashed daemon").unwrap();

            let barrier = Arc::new(Barrier::new(STARTERS));
            let winners = Arc::new(AtomicUsize::new(0));
            let mut threads = Vec::with_capacity(STARTERS);

            for _ in 0..STARTERS {
                let path = path.clone();
                let barrier = Arc::clone(&barrier);
                let winners = Arc::clone(&winners);
                threads.push(std::thread::spawn(move || {
                    // `bind` hands back a tokio listener, which needs a
                    // reactor on the thread that creates it.
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("runtime");
                    let _enter = runtime.enter();
                    barrier.wait();
                    match bind(&path) {
                        Ok(listener) => {
                            winners.fetch_add(1, Ordering::SeqCst);
                            // Hold it: releasing here would let a loser bind
                            // and the count would be right for the wrong
                            // reason.
                            Some(listener)
                        }
                        Err(_) => None,
                    }
                }));
            }

            let held: Vec<_> = threads
                .into_iter()
                .map(|t| t.join().expect("no panic"))
                .collect();

            assert_eq!(
                winners.load(Ordering::SeqCst),
                1,
                "round {round}: more than one starter bound the same socket"
            );
            // The winner is still reachable, so nobody unlinked its socket and
            // put an unbound file in its place.
            std::os::unix::net::UnixStream::connect(&path)
                .unwrap_or_else(|e| panic!("round {round}: the surviving socket is dead: {e}"));
            drop(held);
        }
    }

    #[tokio::test]
    async fn the_socket_is_owner_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("daemon.sock");
        let _listener = bind(&path).expect("bind");

        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "the socket is the only auth boundary");
    }

    /// Security review F-9. The mode must hold from the instant the path
    /// exists, not one `chmod` later: `bind(2)` takes no mode argument, so a
    /// socket bound at its final path is `0777 & ~umask` until it is tightened
    /// — 0755 under the usual umask, and `connect(2)` needs only write
    /// permission. The socket is the only authentication boundary (manifest 04
    /// I14), so that window is the whole of it.
    ///
    /// The parent is left deliberately traversable, because the guarantee must
    /// not rest on it: tightening the parent is warn-only, and it may pre-exist
    /// wider.
    ///
    /// An observer thread polls the path while `bind` runs. Rounds, because the
    /// window is two syscalls wide and any one round can miss it.
    #[tokio::test]
    async fn the_socket_is_never_visible_wider_than_owner_only() {
        const ROUNDS: usize = 200;

        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        let path = dir.path().join("daemon.sock");

        let stop = Arc::new(AtomicBool::new(false));
        let seen = Arc::new(AtomicUsize::new(NOTHING_SEEN));
        let observer = {
            let path = path.clone();
            let stop = Arc::clone(&stop);
            let seen = Arc::clone(&seen);
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    if let Ok(meta) = std::fs::symlink_metadata(&path) {
                        let mode = meta.permissions().mode() as usize & 0o777;
                        if mode != 0o600 {
                            let _ = seen.compare_exchange(
                                NOTHING_SEEN,
                                mode,
                                Ordering::Relaxed,
                                Ordering::Relaxed,
                            );
                        }
                    }
                }
            })
        };

        for _ in 0..ROUNDS {
            drop(bind(&path).expect("bind"));
            std::fs::remove_file(&path).expect("clear the socket for the next round");
        }

        stop.store(true, Ordering::Relaxed);
        observer.join().expect("no panic");

        let observed = seen.load(Ordering::Relaxed);
        assert_eq!(
            observed, NOTHING_SEEN,
            "the socket was reachable at {observed:o} before it was locked down"
        );
    }

    const NOTHING_SEEN: usize = usize::MAX;

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

        // list sees it, and reports that nothing was unreadable.
        let response = client
            .call(request(
                3,
                Method::List {
                    limit: 10,
                    cursor: None,
                },
            ))
            .await;
        let page = match response.data {
            Some(ResponseData::Page(page)) => page,
            other => panic!("expected a page, got {other:?}"),
        };
        assert!(page.items.iter().any(|item| item.id == added.id));
        assert_eq!(page.skipped_undecryptable, 0);

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

        // ⌥Enter reaches its own wire verb, even while text is the only
        // representation v2 captures.
        let response = client
            .call(request(
                41,
                Method::CopyPlainText {
                    id: added.id.clone(),
                },
            ))
            .await;
        assert!(response.ok, "plain-text copy failed: {:?}", response.error);

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

    /// A subscriber gets an ack, then a frame per change, on the same
    /// connection and in the same envelope.
    #[tokio::test]
    async fn a_watcher_is_pushed_a_frame_when_history_changes() {
        let (state, dir) = test_state("server");
        let path = dir.path().join("daemon.sock");
        let listener = bind(&path).expect("bind");
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let server = tokio::spawn(run(listener, Arc::clone(&state), shutdown_rx));

        let stream = UnixStream::connect(&path).await.expect("connect");
        let mut watcher = Client::new(stream);
        let ack = watcher.call(request(1, Method::Watch)).await;
        assert!(ack.ok, "{:?}", ack.error);

        // A capture on the daemon side, through the ordinary ingest path.
        crate::server::dispatch::dispatch_store(
            &state,
            99,
            Method::Add {
                content: "pushed".into(),
            },
        );

        let frame = tokio::time::timeout(Duration::from_secs(5), watcher.next_frame())
            .await
            .expect("an event must arrive without polling");
        assert_eq!(frame.id, 1);
        match frame.data {
            Some(ResponseData::Event(event)) => {
                assert_eq!(event.event, copypaste_ipc::EventKind::Items);
                assert_eq!(event.item_count, 1);
            }
            other => panic!("expected an event, got {other:?}"),
        }

        shutdown_tx.send(true).unwrap();
        let _ = server.await;
    }

    /// The order is the whole point: the acknowledgement is written *before*
    /// the signal, so a client learns the request was accepted rather than
    /// inferring it from a closed socket. Doing it the other way round races
    /// `run`'s teardown, which aborts the connection tasks — and the reply is
    /// what would be aborted.
    #[tokio::test]
    async fn shutdown_is_acknowledged_and_then_acted_on() {
        let (state, dir) = test_state("server");
        let path = dir.path().join("daemon.sock");
        let listener = bind(&path).expect("bind");
        let server = tokio::spawn(run(listener, Arc::clone(&state), state.shutdown_rx()));

        let mut stopping = state.shutdown_rx();
        assert!(!*stopping.borrow(), "nothing has asked yet");

        let stream = UnixStream::connect(&path).await.expect("connect");
        let reply = Client::new(stream).call(request(7, Method::Shutdown)).await;
        assert!(reply.ok, "{:?}", reply.error);
        assert_eq!(reply.id, 7, "a client matches replies by id");

        tokio::time::timeout(Duration::from_secs(5), stopping.changed())
            .await
            .expect("the signal must follow the reply")
            .expect("the sender outlives this");
        assert!(*stopping.borrow());

        // And the accept loop unwinds on it, exactly as it does on SIGTERM.
        tokio::time::timeout(Duration::from_secs(5), server)
            .await
            .expect("the server must stop")
            .expect("cleanly");
    }

    /// Answerable before readiness, deliberately: a daemon whose database will
    /// not open is exactly the one a user needs to be able to stop, and every
    /// other verb would answer `not_ready`.
    #[tokio::test]
    async fn shutdown_answers_even_when_the_daemon_is_not_ready() {
        let (state, dir) = test_state("server");
        state.set_ready(false);
        let path = dir.path().join("daemon.sock");
        let listener = bind(&path).expect("bind");
        let server = tokio::spawn(run(listener, Arc::clone(&state), state.shutdown_rx()));

        let stream = UnixStream::connect(&path).await.expect("connect");
        let mut client = Client::new(stream);
        let refused = client
            .call(request(
                1,
                Method::List {
                    limit: 10,
                    cursor: None,
                },
            ))
            .await;
        assert_eq!(refused.error_code, Some(ErrorCode::NotReady));

        let stream = UnixStream::connect(&path).await.expect("connect");
        let reply = Client::new(stream).call(request(2, Method::Shutdown)).await;
        assert!(reply.ok, "{:?}", reply.error);

        let _ = tokio::time::timeout(Duration::from_secs(5), server).await;
    }

    /// The cap is non-blocking, so an over-cap connection is dropped rather
    /// than queued behind a permit that may never come free.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn connections_past_the_cap_are_dropped_rather_than_queued() {
        let (state, dir) = test_state("server");
        let path = dir.path().join("daemon.sock");
        let listener = bind(&path).expect("bind");
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let server = tokio::spawn(run(listener, Arc::clone(&state), shutdown_rx));

        // Fill every permit. Each one is made to answer a request first: a
        // connect alone only reaches the listen backlog, so without a round
        // trip the test would race the accept loop rather than the cap.
        let mut idle = Vec::new();
        for n in 0..MAX_CONCURRENT_CONNECTIONS {
            let mut client = Client::new(UnixStream::connect(&path).await.expect("connect"));
            assert!(
                client
                    .call(request(n as u64 + 100, Method::Status))
                    .await
                    .ok
            );
            idle.push(client);
        }

        // The next one is accepted by the OS and then dropped, so it is never
        // served. Which of the three symptoms shows up — a broken pipe on the
        // write, a reset on the read, or a clean EOF — depends on timing; the
        // property is that no `Response` comes back.
        let stream = UnixStream::connect(&path).await.expect("connect");
        let mut client = Client::new(stream);
        let line = serde_json::to_string(&request(1, Method::Status)).unwrap();
        let served = tokio::time::timeout(Duration::from_secs(5), async {
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
        shutdown_tx.send(true).unwrap();
        let _ = server.await;
    }

    /// A minimal newline-JSON client, mirroring what the CLI does.
    struct Client {
        writer: OwnedWriteHalf,
        lines: FramedRead<OwnedReadHalf, LinesCodec>,
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
            self.next_frame().await
        }

        async fn next_frame(&mut self) -> Response {
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

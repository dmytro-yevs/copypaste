//! The socket itself: binding it, guarding it, bounding it, and framing what
//! arrives on it.
//!
//! Newline framing reserves a shared wire-buffer budget before allocation.
//! Control frames use a separate pool so bulk frames cannot starve status.
//! A connection closes on overload rather than waiting while retaining a
//! partial frame. Writes retain their reservation until completion or timeout.
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

use std::io::{self, Write};
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
#[cfg(unix)]
use std::path::Path;
#[cfg(unix)]
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

#[cfg(unix)]
use anyhow::Context;
use copypaste_ipc::transport::{Listener, OwnedReadHalf, OwnedWriteHalf, Stream};
use copypaste_ipc::{ErrorCode, Method, Response, ResponseData, MAX_FRAME_BYTES};
#[cfg(all(test, unix))]
use futures_util::StreamExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
#[cfg(unix)]
use tokio::net::UnixListener;
use tokio::sync::{watch, Semaphore};
#[cfg(all(test, unix))]
use tokio_util::codec::{FramedRead, LinesCodec, LinesCodecError};
use tracing::{debug, error, info, warn};

use super::dispatch::{dispatch_request, gate, parse};
use super::messages::{MSG_NOT_READY, MSG_TOO_LARGE, MSG_WATCHERS_FULL};
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
#[cfg(unix)]
pub fn bind(path: &Path) -> anyhow::Result<Listener> {
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
    Ok(listener.into())
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
#[cfg(unix)]
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

#[cfg(unix)]
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
#[cfg(unix)]
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
#[cfg(unix)]
struct BindLock {
    _file: std::fs::File,
}

#[cfg(unix)]
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
#[cfg(unix)]
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
pub async fn run(listener: Listener, state: Arc<AppState>, mut shutdown: watch::Receiver<bool>) {
    let mut connections = tokio::task::JoinSet::new();
    let permits = Arc::new(Semaphore::new(MAX_CONCURRENT_CONNECTIONS));
    let watchers = Arc::new(Semaphore::new(MAX_WATCHERS));

    let mut release = state.drain_release_rx();
    let mut draining = state.is_draining();
    loop {
        if release_requested(&mut release) {
            break;
        }

        tokio::select! {
            _ = shutdown.changed(), if !draining => draining = true,
            _ = release.changed(), if draining => break,
            Some(_) = connections.join_next() => {}
            accepted = listener.accept() => match accepted {
                Ok(stream) => {
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
                    let release = release.clone();
                    connections.spawn(async move {
                        let _permit = permit;
                        handle_connection(stream, state, watchers, shutdown, release).await;
                    });
                }
                Err(e) => warn!(error = %e, "could not accept an ipc connection"),
            },
        }
    }

    while connections.join_next().await.is_some() {}
}

/// One connection: read lines, answer each with exactly one line, until EOF —
/// or until it subscribes, after which it only writes.
async fn handle_connection(
    stream: Stream,
    state: Arc<AppState>,
    watchers: Arc<Semaphore>,
    mut shutdown: watch::Receiver<bool>,
    mut release: watch::Receiver<bool>,
) {
    let (reader, mut writer) = stream.into_split();
    let mut lines = Reader::new(reader);

    loop {
        if release_requested(&mut release) {
            break;
        }

        // A client that connects and then says nothing holds a permit and, if
        // it is mid-request, the database mutex. The deadline is per read, so a
        // client that keeps talking is never cut off (`CopyPaste-cce1`).
        let next = match tokio::select! {
            _ = release.changed() => break,
            next = tokio::time::timeout(READ_TIMEOUT, lines.next()) => next,
        } {
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
            Some(Err(FrameError::TooLarge)) => {
                // Answered, then closed — manifest 04 §6.2: after
                // `request too large` "the connection is already closed", which
                // is what `halted::refuse` and the Windows pipe already do.
                //
                // The codec *can* resume at the next newline, and this used to.
                // Resuming means reading and discarding an unbounded stream, at
                // the sender's rate, while holding one of 64 connection permits
                // — and it held on a socket and not on a pipe, so the same
                // client saw two protocols (DMY-158). The id lived somewhere in
                // the discarded bytes, so `0` is the best that can be echoed.
                let response = Response::err(0, ErrorCode::InvalidRequest, MSG_TOO_LARGE);
                let _ = send(&mut writer, &response).await;
                break;
            }
            Some(Err(e)) => {
                debug!(error = %e, "ipc connection read failed");
                break;
            }
        };

        // Blank lines are keep-alives.
        if line.text.trim().is_empty() {
            continue;
        }

        let request = match parse(&line.text) {
            Ok(request) => request,
            Err(rejection) => {
                if send(&mut writer, &rejection.response).await.is_err() {
                    break;
                }
                continue;
            }
        };

        // Parsing owns its data. Release the raw frame, but retain its credit
        // while the decoded request is queued or executing.
        let wire_lease = line.into_lease();

        // `shutdown` is taken here for the same reason `watch` is below: it
        // changes what the connection is — this one ends, and so does the
        // process. Handling it in the dispatcher could not order the two
        // correctly, because `run` aborts the connection tasks as soon as it
        // sees the signal, and the reply would be the thing aborted. Here the
        // acknowledgement is written *first*, and only then is the signal sent.
        //
        // Taken before the gates, never after: a client that speaks a version
        // this daemon does not is *the* caller for this verb — that is the
        // whole of ADR-0004's protocol-mismatch state — and refusing it with
        // `protocol_mismatch` would leave the mismatch with no way out. It is
        // exempt from readiness for the same reason: a daemon whose database
        // will not open is exactly the one a user needs to be able to stop.
        if matches!(request.method, Method::Shutdown) {
            info!("shutdown requested over ipc");
            let _ = send(
                &mut writer,
                &Response::ok(request.id, ResponseData::Empty {}),
            )
            .await;
            state.request_shutdown();
            break;
        }

        if let Some(rejection) = gate(&state, &request) {
            let close_connection = rejection.was_watch;
            if send(&mut writer, &rejection.response).await.is_err() || close_connection {
                break;
            }
            continue;
        }

        // `watch` turns the connection into a stream, so it is taken here
        // rather than in the dispatcher: from this point the loop above no
        // longer owns the read half's deadline.
        if matches!(&request.method, Method::Watch | Method::WatchRuntime) {
            let typed = matches!(&request.method, Method::WatchRuntime);
            drop(wire_lease);
            let id = request.id;
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
            let runtime_events = typed.then(|| state.subscribe_runtime());
            if send(&mut writer, &Response::ok(id, ResponseData::Empty {}))
                .await
                .is_err()
            {
                break;
            }
            if let Some(runtime_events) = runtime_events {
                super::watch::pump_runtime(
                    Arc::clone(&state),
                    events,
                    runtime_events,
                    id,
                    &mut lines,
                    &mut writer,
                    &mut shutdown,
                )
                .await;
            } else {
                super::watch::pump(events, id, &mut lines, &mut writer, &mut shutdown).await;
            }
            break;
        }

        let admission = if matches!(request.method, Method::Status) {
            None
        } else {
            let Some(admission) = state.admit_request().await else {
                let response = Response::err(request.id, ErrorCode::NotReady, MSG_NOT_READY);
                let _ = send(&mut writer, &response).await;
                break;
            };
            Some(admission)
        };

        let is_interactive = super::dispatch::interactive(&request.method);
        let response = dispatch_request(&state, request, wire_lease).await;

        if send_in(&mut writer, &response, is_interactive, wire_budget())
            .await
            .is_err()
        {
            break;
        }
        drop(admission);
    }

    // `interprocess` puts dirty Windows pipes in limbo on drop. Flush before
    // closing so a response the client already consumed cannot keep the pipe
    // alive and hide the EOF promised by the read deadline.
    let _ = tokio::time::timeout(WRITE_TIMEOUT, writer.shutdown()).await;
}

fn release_requested(release: &mut watch::Receiver<bool>) -> bool {
    *release.borrow_and_update()
}

// Allocation limits for wire buffers, not process memory budgets. Small frames
// have a separate pool so partially sent large frames cannot exhaust status.
const CONTROL_FRAME: usize = 4096;
const CONTROL_BYTES: usize = 4 * 1024 * 1024;
const BULK_BYTES: usize = 128 * 1024 * 1024;
const INTERACTIVE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Default)]
struct WireBudget {
    control: AtomicUsize,
    bulk: AtomicUsize,
    interactive: AtomicUsize,
}

fn wire_budget() -> &'static WireBudget {
    static BUDGET: OnceLock<WireBudget> = OnceLock::new();
    BUDGET.get_or_init(WireBudget::default)
}

pub(super) struct WireLease {
    used: &'static AtomicUsize,
    bytes: usize,
}
impl WireLease {
    fn acquire_in(budget: &'static WireBudget, bytes: usize) -> Option<Self> {
        let (used, limit) = if bytes <= CONTROL_FRAME * 2 {
            (&budget.control, CONTROL_BYTES)
        } else {
            (&budget.bulk, BULK_BYTES)
        };
        Self::reserve(used, limit, bytes)
    }

    fn response(budget: &'static WireBudget, bytes: usize, interactive: bool) -> Option<Self> {
        if interactive && bytes > CONTROL_FRAME * 2 {
            Self::reserve(&budget.interactive, INTERACTIVE_BYTES, bytes)
        } else {
            Self::acquire_in(budget, bytes)
        }
    }

    pub(super) fn reserve(used: &'static AtomicUsize, limit: usize, bytes: usize) -> Option<Self> {
        used.fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
            n.checked_add(bytes).filter(|n| *n <= limit)
        })
        .ok()?;
        Some(Self { used, bytes })
    }
}
impl Drop for WireLease {
    fn drop(&mut self) {
        self.used.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}

#[derive(Debug)]
pub(super) enum FrameError {
    TooLarge,
    Overloaded,
    InvalidUtf8,
    Io(io::Error),
}
impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            _ => write!(f, "{self:?}"),
        }
    }
}
pub(super) struct Frame {
    pub(super) text: String,
    _lease: Option<WireLease>,
}
impl Frame {
    fn into_lease(self) -> Option<WireLease> {
        let Self { text, _lease } = self;
        drop(text);
        _lease
    }
}

/// Fixed-size lookahead plus a charged frame buffer. A growth never waits
/// while holding partial data: overload closes the connection instead.
pub(super) struct Reader {
    reader: OwnedReadHalf,
    budget: &'static WireBudget,
    lookahead: [u8; CONTROL_FRAME],
    pos: usize,
    end: usize,
    frame: Vec<u8>,
    lease: Option<WireLease>,
}
impl Reader {
    pub(super) fn new(reader: OwnedReadHalf) -> Self {
        Self {
            reader,
            budget: wire_budget(),
            lookahead: [0; CONTROL_FRAME],
            pos: 0,
            end: 0,
            frame: Vec::new(),
            lease: None,
        }
    }
    #[cfg(all(test, unix))]
    fn append(&mut self, bytes: &[u8]) -> Result<(), FrameError> {
        Self::append_to(&mut self.frame, &mut self.lease, self.budget, bytes)
    }
    fn append_to(
        frame: &mut Vec<u8>,
        current_lease: &mut Option<WireLease>,
        budget: &'static WireBudget,
        bytes: &[u8],
    ) -> Result<(), FrameError> {
        let required = frame
            .len()
            .checked_add(bytes.len())
            .ok_or(FrameError::TooLarge)?;
        if required > MAX_FRAME_BYTES {
            return Err(FrameError::TooLarge);
        }
        if required > frame.capacity() {
            let target = required
                .next_power_of_two()
                .clamp(CONTROL_FRAME, MAX_FRAME_BYTES);
            // Reserve for the new buffer while the previous buffer remains live.
            let lease = WireLease::acquire_in(budget, target * 2).ok_or(FrameError::Overloaded)?;
            let mut replacement = Vec::new();
            replacement
                .try_reserve_exact(target)
                .map_err(|_| FrameError::Overloaded)?;
            if replacement.capacity() > target * 2 {
                return Err(FrameError::Overloaded);
            }
            replacement.extend_from_slice(frame);
            *frame = replacement;
            *current_lease = Some(lease);
        }
        frame.extend_from_slice(bytes);
        Ok(())
    }
    pub(super) async fn next(&mut self) -> Option<Result<Frame, FrameError>> {
        loop {
            if self.pos == self.end {
                match self.reader.read(&mut self.lookahead).await {
                    Ok(0) if self.frame.is_empty() => return None,
                    Ok(0) => return Some(self.finish()),
                    Ok(n) => {
                        self.pos = 0;
                        self.end = n;
                    }
                    Err(e) => return Some(Err(FrameError::Io(e))),
                }
            }
            let end = self.lookahead[self.pos..self.end]
                .iter()
                .position(|b| *b == b'\n')
                .map(|n| self.pos + n);
            let stop = end.unwrap_or(self.end);
            let result = Self::append_to(
                &mut self.frame,
                &mut self.lease,
                self.budget,
                &self.lookahead[self.pos..stop],
            );
            self.pos = stop + usize::from(end.is_some());
            if let Err(e) = result {
                return Some(Err(e));
            }
            if end.is_some() {
                return Some(self.finish());
            }
        }
    }
    fn finish(&mut self) -> Result<Frame, FrameError> {
        if self.frame.last() == Some(&b'\r') {
            self.frame.pop();
        }
        let bytes = std::mem::take(&mut self.frame);
        let lease = self.lease.take();
        let text = String::from_utf8(bytes).map_err(|_| FrameError::InvalidUtf8)?;
        Ok(Frame {
            text,
            _lease: lease,
        })
    }
}

struct ByteCount(usize);
impl Write for ByteCount {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 = self
            .0
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::other("response size overflow"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct AdmittedWriter(Vec<u8>);
impl Write for AdmittedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.0.capacity() - self.0.len() {
            return Err(io::Error::other("response exceeded its admission"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Serialize into an admitted buffer, then retain its lease through the write.
pub(super) async fn send(writer: &mut OwnedWriteHalf, response: &Response) -> Result<(), ()> {
    send_in(writer, response, false, wire_budget()).await
}

async fn send_in(
    writer: &mut OwnedWriteHalf,
    response: &Response,
    interactive: bool,
    budget: &'static WireBudget,
) -> Result<(), ()> {
    let mut count = ByteCount(0);
    if let Err(e) = serde_json::to_writer(&mut count, response) {
        error!(error = %e, "could not serialise a response");
        return Err(());
    }
    let capacity = count.0.checked_add(1).ok_or(())?;
    let _lease =
        WireLease::response(budget, capacity.checked_mul(2).ok_or(())?, interactive).ok_or(())?;
    let mut encoded = Vec::new();
    encoded.try_reserve_exact(capacity).map_err(|_| ())?;
    if encoded.capacity() > capacity * 2 {
        return Err(());
    }
    let mut admitted = AdmittedWriter(encoded);
    serde_json::to_writer(&mut admitted, response).map_err(|_| ())?;
    admitted.write_all(b"\n").map_err(|_| ())?;
    match tokio::time::timeout(WRITE_TIMEOUT, writer.write_all(&admitted.0)).await {
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

#[cfg(all(test, unix))]
pub(super) use crate::testutil::test_socket_path;

// Every test here binds a real endpoint, which only Unix has one of yet.
#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    use crate::testutil::test_state;
    use copypaste_ipc::transport;
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

    #[test]
    fn reservations_release_on_drop_and_reject_overload() {
        static USED: AtomicUsize = AtomicUsize::new(0);
        let first = WireLease::reserve(&USED, 100, 60).unwrap();
        assert!(WireLease::reserve(&USED, 100, 41).is_none());
        let second = WireLease::reserve(&USED, 100, 40).unwrap();
        assert_eq!(USED.load(Ordering::Acquire), 100);
        drop(first);
        assert_eq!(USED.load(Ordering::Acquire), 40);
        drop(second);
        assert_eq!(USED.load(Ordering::Acquire), 0);
    }

    #[tokio::test]
    async fn partial_bulk_overload_does_not_consume_control_reserve() {
        static TEST_BUDGET: OnceLock<WireBudget> = OnceLock::new();
        let budget = TEST_BUDGET.get_or_init(WireBudget::default);
        let bulk = WireLease::acquire_in(budget, BULK_BYTES).unwrap();
        let mut readers = Vec::new();
        let mut clients = Vec::new();
        for _ in 0..MAX_CONCURRENT_CONNECTIONS {
            let (client, server) = transport::Stream::pair().unwrap();
            let (read, _) = server.into_split();
            let mut reader = Reader::new(read);
            reader.budget = budget;
            reader.append(&[b'a'; CONTROL_FRAME]).unwrap();
            assert!(matches!(reader.append(b"a"), Err(FrameError::Overloaded)));
            readers.push(reader);
            clients.push(client);
        }
        let control = WireLease::acquire_in(budget, 512).unwrap();
        drop(control);
        drop(readers);
        drop(clients);
        drop(bulk);
        assert_eq!(budget.control.load(Ordering::Acquire), 0);
        assert_eq!(budget.bulk.load(Ordering::Acquire), 0);
    }

    #[tokio::test]
    async fn bulk_wire_pressure_leaves_interactive_and_control_replies() {
        static TEST_BUDGET: OnceLock<WireBudget> = OnceLock::new();
        let budget = TEST_BUDGET.get_or_init(WireBudget::default);
        let bulk = WireLease::acquire_in(budget, BULK_BYTES).unwrap();
        let (server, client) = transport::Stream::pair().unwrap();
        let (_, mut writer) = server.into_split();
        let (mut reader, _) = client.into_split();
        let response = Response::ok(
            7,
            ResponseData::Modules {
                json: "x".repeat(8192),
            },
        );
        let task = tokio::spawn(async move { send_in(&mut writer, &response, true, budget).await });
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await.unwrap();
        task.await.unwrap().unwrap();
        let parsed: Response = serde_json::from_slice(&bytes).unwrap();
        assert!(parsed.ok);
        let control = WireLease::response(budget, 512, false).unwrap();
        drop(control);
        drop(bulk);
        assert_eq!(budget.bulk.load(Ordering::Acquire), 0);
        assert_eq!(budget.interactive.load(Ordering::Acquire), 0);
        assert_eq!(budget.control.load(Ordering::Acquire), 0);
    }

    #[tokio::test]
    async fn max_legal_frame_and_pipelined_next_frame_are_preserved() {
        let (client, server) = transport::Stream::pair().unwrap();
        let (read, _) = server.into_split();
        let (_, mut write) = client.into_split();
        let mut reader = Reader::new(read);
        // Isolate this parser contract from concurrent response-budget tests.
        static TEST_BUDGET: OnceLock<WireBudget> = OnceLock::new();
        reader.budget = TEST_BUDGET.get_or_init(WireBudget::default);
        let task = tokio::spawn(async move {
            let prefix = br#"{"id":1,"protocol_version":6,"method":"status","unknown":""#;
            let suffix = br#""}"#;
            let mut remaining = MAX_FRAME_BYTES - prefix.len() - suffix.len();
            let chunk = [b'x'; CONTROL_FRAME];
            write.write_all(prefix).await.unwrap();
            while remaining > 0 {
                let n = remaining.min(chunk.len());
                write.write_all(&chunk[..n]).await.unwrap();
                remaining -= n;
            }
            write.write_all(suffix).await.unwrap();
            write.write_all(b"\nnext\n").await.unwrap();
        });
        let frame = reader.next().await.unwrap().unwrap();
        assert_eq!(frame.text.len(), MAX_FRAME_BYTES);
        assert!(matches!(
            parse(&frame.text).ok().unwrap().method,
            Method::Status
        ));
        drop(frame);
        assert_eq!(reader.next().await.unwrap().unwrap().text, "next");
        task.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn stalled_response_writer_times_out() {
        let (server, client) = transport::Stream::pair().unwrap();
        let (_, mut writer) = server.into_split();
        let response = Response::ok(
            1,
            ResponseData::Modules {
                json: "x".repeat(4 * 1024 * 1024),
            },
        );
        assert!(send(&mut writer, &response).await.is_err());
        drop(client);
    }

    #[tokio::test]
    async fn eof_and_invalid_utf8_release_the_frame_lease() {
        let (client, server) = transport::Stream::pair().unwrap();
        let (read, _) = server.into_split();
        let (_, mut write) = client.into_split();
        let mut reader = Reader::new(read);
        write.write_all(&[0xff, b'\n']).await.unwrap();
        assert!(matches!(
            reader.next().await.unwrap(),
            Err(FrameError::InvalidUtf8)
        ));
        assert!(reader.lease.is_none());
        write.write_all(b"final").await.unwrap();
        write.shutdown().await.unwrap();
        assert_eq!(reader.next().await.unwrap().unwrap().text, "final");
        assert!(reader.next().await.is_none());
    }

    #[tokio::test]
    async fn a_stale_socket_file_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = test_socket_path(&dir, "daemon.sock");
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
            let path = test_socket_path(&dir, format!("daemon-{round}.sock"));
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
        let path = test_socket_path(&dir, "daemon.sock");
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
        let path = test_socket_path(&dir, "daemon.sock");

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
        let path = test_socket_path(&dir, "daemon.sock");
        let listener = bind(&path).expect("bind");

        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let server = tokio::spawn(run(listener, Arc::clone(&state), shutdown_rx));

        let stream = transport::connect(&path).await.expect("connect");
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
        let copied = match response.data {
            Some(ResponseData::Item(item)) => item,
            other => panic!("expected the copied item, got {other:?}"),
        };
        assert_eq!(copied.id, added.id);
        assert!(copied.created_at > added.created_at);

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
        match response.data {
            Some(ResponseData::Item(item)) => assert!(item.created_at > copied.created_at),
            other => panic!("expected the copied item, got {other:?}"),
        }

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
        state.release_drain_listener();
        let _ = server.await;
    }

    #[tokio::test]
    async fn shutdown_keeps_the_endpoint_and_refuses_new_mutations_until_final_release() {
        let (state, dir) = test_state("draining-endpoint");
        let path = test_socket_path(&dir, "daemon.sock");
        let listener = bind(&path).expect("bind");
        let server = tokio::spawn(run(listener, Arc::clone(&state), state.shutdown_rx()));

        state.request_shutdown();
        tokio::task::yield_now().await;

        assert!(
            !server.is_finished(),
            "shutdown released the live endpoint before accepted capture work finished"
        );
        assert!(
            bind(&path).is_err(),
            "a second daemon could bind while the first one was draining"
        );

        let stream = transport::connect(&path).await.expect("draining endpoint");
        let mut client = Client::new(stream);
        let status = client.call(request(80, Method::Status)).await;
        assert!(
            status.ok,
            "status must stay usable while draining: {status:?}"
        );

        let stream = transport::connect(&path).await.expect("draining endpoint");
        let mut client = Client::new(stream);
        let response = client
            .call(request(
                81,
                Method::Add {
                    content: "must not be accepted while draining".into(),
                },
            ))
            .await;
        assert_eq!(response.id, 81);
        assert_eq!(response.error_code, Some(ErrorCode::NotReady));
        assert_eq!(state.store.count().unwrap(), 0);

        let stream = transport::connect(&path).await.expect("draining endpoint");
        let mut client = Client::new(stream);
        let mut wrong_protocol = request(
            82,
            Method::Add {
                content: "must fail protocol validation first".into(),
            },
        );
        wrong_protocol.protocol_version += 1;
        let response = client.call(wrong_protocol).await;
        assert_eq!(response.id, 82);
        assert_eq!(response.error_code, Some(ErrorCode::ProtocolMismatch));

        server.abort();
        let _ = server.await;
    }

    #[tokio::test]
    async fn final_release_closes_idle_and_keepalive_clients_before_listener_join() {
        let (state, dir) = test_state("drain-client-cleanup");
        let path = test_socket_path(&dir, "daemon.sock");
        let listener = bind(&path).expect("bind");
        let server = tokio::spawn(run(listener, Arc::clone(&state), state.shutdown_rx()));

        state.request_shutdown();

        // A Unix connect may finish while the peer is still only queued on the
        // listener. A status round trip proves that each connection task owns
        // its accepted stream before final release starts closing clients.
        let mut idle = Client::new(transport::connect(&path).await.expect("draining endpoint"));
        assert!(idle.call(request(83, Method::Status)).await.ok);

        let mut keepalive =
            Client::new(transport::connect(&path).await.expect("draining endpoint"));
        assert!(keepalive.call(request(84, Method::Status)).await.ok);
        keepalive
            .writer
            .write_all(b"\n\n")
            .await
            .expect("keepalive");
        keepalive.writer.flush().await.expect("flush keepalive");

        state.release_drain_listener();
        for client in [&mut idle, &mut keepalive] {
            let next = tokio::time::timeout(Duration::from_secs(1), client.lines.next())
                .await
                .expect("release must close the client");
            match next {
                None => {}
                Some(Err(LinesCodecError::Io(error)))
                    if error.kind() == std::io::ErrorKind::ConnectionReset => {}
                other => {
                    panic!("accepted client remained open after final listener release: {other:?}")
                }
            }
        }
        tokio::time::timeout(Duration::from_secs(1), server)
            .await
            .expect("listener joins after client cleanup")
            .expect("listener task must not panic");
    }

    #[test]
    fn a_release_before_a_connection_handler_starts_is_observed() {
        let (release_tx, _release_rx) = watch::channel(false);
        release_tx
            .send(true)
            .expect("release receiver remains held");

        assert!(release_requested(&mut release_tx.subscribe()));
    }

    #[tokio::test]
    async fn watch_rejects_unsupported_protocol_versions_and_closes() {
        let (state, dir) = test_state("watch-version");
        let path = test_socket_path(&dir, "daemon.sock");
        let listener = bind(&path).expect("bind");
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let server = tokio::spawn(run(listener, Arc::clone(&state), shutdown_rx));

        for (id, protocol_version) in [(20, 0), (21, PROTOCOL_VERSION + 1)] {
            let stream = transport::connect(&path).await.expect("connect");
            let mut client = Client::new(stream);
            let response = client
                .call(Request {
                    id,
                    protocol_version,
                    method: Method::Watch,
                })
                .await;

            assert_eq!(response.id, id);
            assert!(!response.ok);
            assert_eq!(response.error_code, Some(ErrorCode::ProtocolMismatch));
            assert_connection_closed(&mut client).await;
        }

        shutdown_tx.send(true).unwrap();
        state.release_drain_listener();
        let _ = server.await;
    }

    #[tokio::test]
    async fn watch_rejects_a_not_ready_daemon_and_closes() {
        let (state, dir) = test_state("watch-readiness");
        state.set_ready(false);
        let path = test_socket_path(&dir, "daemon.sock");
        let listener = bind(&path).expect("bind");
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let server = tokio::spawn(run(listener, Arc::clone(&state), shutdown_rx));

        let stream = transport::connect(&path).await.expect("connect");
        let mut client = Client::new(stream);
        let response = client.call(request(22, Method::Watch)).await;

        assert_eq!(response.id, 22);
        assert!(!response.ok);
        assert_eq!(response.error_code, Some(ErrorCode::NotReady));
        assert_connection_closed(&mut client).await;

        shutdown_tx.send(true).unwrap();
        state.release_drain_listener();
        let _ = server.await;
    }

    #[tokio::test]
    async fn malformed_watch_is_rejected_without_changing_the_connection() {
        let (state, dir) = test_state("watch-malformed");
        let path = test_socket_path(&dir, "daemon.sock");
        let listener = bind(&path).expect("bind");
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let server = tokio::spawn(run(listener, Arc::clone(&state), shutdown_rx));

        let stream = transport::connect(&path).await.expect("connect");
        let mut client = Client::new(stream);
        let response = client
            .call_raw(r#"{"id":23,"protocol_version":"wrong","method":"watch"}"#)
            .await;
        assert_eq!(response.id, 23);
        assert_eq!(response.error_code, Some(ErrorCode::InvalidRequest));

        let response = client.call(request(24, Method::Status)).await;
        assert!(response.ok, "the malformed line must not start a stream");
        assert_eq!(response.id, 24);

        shutdown_tx.send(true).unwrap();
        state.release_drain_listener();
        let _ = server.await;
    }

    #[tokio::test]
    async fn watch_acknowledges_a_successful_subscription() {
        let (state, dir) = test_state("watch-ack");
        let path = test_socket_path(&dir, "daemon.sock");
        let listener = bind(&path).expect("bind");
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let server = tokio::spawn(run(listener, Arc::clone(&state), shutdown_rx));

        let stream = transport::connect(&path).await.expect("connect");
        let ack = Client::new(stream).call(request(25, Method::Watch)).await;

        assert!(ack.ok, "{:?}", ack.error);
        assert_eq!(ack.id, 25);
        assert!(matches!(ack.data, Some(ResponseData::Empty {})));

        shutdown_tx.send(true).unwrap();
        state.release_drain_listener();
        let _ = server.await;
    }

    #[tokio::test]
    async fn runtime_watch_sends_snapshot_and_capture_transitions() {
        let (state, dir) = test_state("runtime-capture-watch");
        let path = test_socket_path(&dir, "daemon.sock");
        let listener = bind(&path).expect("bind");
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let server = tokio::spawn(run(listener, Arc::clone(&state), shutdown_rx));
        let stream = transport::connect(&path).await.expect("connect");
        let mut watcher = Client::new(stream);
        let ack = watcher.call(request(1, Method::WatchRuntime)).await;
        assert!(ack.ok, "{:?}", ack.error);
        let initial = tokio::time::timeout(Duration::from_secs(5), watcher.next_frame())
            .await
            .unwrap();
        let Some(ResponseData::RuntimeEvent(initial)) = initial.data else {
            panic!("missing snapshot");
        };
        assert!(!initial.capture.unwrap().capture_running);
        state.set_capture_running(true);
        let frame = tokio::time::timeout(Duration::from_secs(5), watcher.next_frame())
            .await
            .unwrap();
        let Some(ResponseData::RuntimeEvent(event)) = frame.data else {
            panic!("missing event");
        };
        let capture = event.capture.unwrap();
        assert!(capture.capture_running);
        assert_eq!(capture.capture_revision, 1);
        shutdown_tx.send(true).unwrap();
        state.release_drain_listener();
        let _ = server.await;
    }

    /// A subscriber gets an ack, then a frame per change, on the same
    /// connection and in the same envelope.
    #[tokio::test]
    async fn a_watcher_is_pushed_a_frame_when_history_changes() {
        let (state, dir) = test_state("server");
        let path = test_socket_path(&dir, "daemon.sock");
        let listener = bind(&path).expect("bind");
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let server = tokio::spawn(run(listener, Arc::clone(&state), shutdown_rx));

        let stream = transport::connect(&path).await.expect("connect");
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
        state.release_drain_listener();
        let _ = server.await;
    }

    fn progress_socket_path(dir: &tempfile::TempDir) -> std::path::PathBuf {
        test_socket_path(dir, "daemon.sock")
    }

    #[tokio::test]
    async fn saving_a_local_file_emits_token_progress() {
        let (state, dir) = test_state("file-progress");
        let original = dir.path().join("original.bin");
        let bytes = vec![9; copypaste_core::file_transfer::CHUNK_BYTES * 2 + 3];
        std::fs::write(&original, &bytes).unwrap();
        let row = copypaste_core::file_import::import_file(
            &state.store,
            &state.keyring,
            &original,
            "original.bin",
            "application/octet-stream",
            original.to_str(),
            &state.settings.get(),
        )
        .unwrap();
        let path = progress_socket_path(&dir);
        let listener = bind(&path).expect("bind");
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let server = tokio::spawn(run(listener, Arc::clone(&state), shutdown_rx));
        let mut typed = Client::new(transport::connect(&path).await.unwrap());
        assert!(typed.call(request(1, Method::WatchRuntime)).await.ok);
        let destination = dir.path().join("saved.bin");
        let mut client = Client::new(transport::connect(&path).await.unwrap());
        let save = tokio::spawn(async move {
            client
                .call(request(
                    2,
                    Method::SaveFile {
                        id: row.id,
                        dest_path: destination.to_string_lossy().into_owned(),
                        download_token: Some("test-token".into()),
                    },
                ))
                .await
        });
        let mut last = 0;
        while last < bytes.len() as u64 {
            let frame = tokio::time::timeout(Duration::from_secs(5), typed.next_frame())
                .await
                .unwrap();
            if let Some(ResponseData::RuntimeEvent(event)) = frame.data {
                if let Some(progress) = event.progress {
                    assert_eq!(progress.token, "test-token");
                    assert!(progress.written > last);
                    assert_eq!(progress.total, bytes.len() as u64);
                    last = progress.written;
                }
            }
        }
        assert!(save.await.unwrap().ok);
        assert_eq!(std::fs::read(dir.path().join("saved.bin")).unwrap(), bytes);
        shutdown_tx.send(true).unwrap();
        state.release_drain_listener();
        let _ = server.await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn managed_download_outlives_request_connection_and_keeps_its_file_link() {
        use std::io::Write;
        let (state, dir) = test_state("managed-download");
        let original = dir.path().join("original.bin");
        let mut source = std::fs::File::create(&original).unwrap();
        for _ in 0..512 {
            source.write_all(&[11; 64 * 1024]).unwrap();
        }
        drop(source);
        let row = copypaste_core::file_reference::import(
            &state.store,
            &state.keyring,
            original.to_str().unwrap(),
            "original.bin",
            "application/octet-stream",
            &state.settings.get(),
            None,
        )
        .unwrap();
        let path = progress_socket_path(&dir);
        let listener = bind(&path).unwrap();
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let server = tokio::spawn(run(listener, Arc::clone(&state), shutdown_rx));
        let mut changes = state.store.subscribe_file_operations();
        let destination = dir.path().join("downloaded.bin");
        let mut client = Client::new(transport::connect(&path).await.unwrap());
        let response = client
            .call(request(
                1,
                Method::StartFileDownload {
                    id: row.id.clone(),
                    dest_path: destination.to_string_lossy().into_owned(),
                    token: "detached-operation".into(),
                },
            ))
            .await;
        assert!(response.ok, "{response:?}");
        drop(client);
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let records = state
                    .store
                    .file_operations(false, None, Some("detached-operation"))
                    .unwrap();
                if let Some(record) = records.first() {
                    if record.phase != copypaste_ipc::FileOperationPhase::Running {
                        assert_eq!(
                            record.phase,
                            copypaste_ipc::FileOperationPhase::Completed,
                            "{record:?}"
                        );
                        assert_eq!(record.local_path.as_deref(), destination.to_str());
                        break;
                    }
                }
                match changes.recv().await {
                    // The durable operation record is the assertion source;
                    // broadcast only wakes this loop. A faster transfer can
                    // legitimately overwrite its bounded notification buffer.
                    Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        panic!("file-operation updates closed before completion")
                    }
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(
            std::fs::metadata(&destination).unwrap().len(),
            32 * 1024 * 1024
        );
        let mut reopened = Client::new(transport::connect(&path).await.unwrap());
        let link = reopened
            .call(request(2, Method::DownloadedFile { id: row.id }))
            .await;
        assert!(
            matches!(link.data, Some(ResponseData::DownloadedFile(Some(value))) if value == destination.to_string_lossy())
        );
        shutdown_tx.send(true).unwrap();
        state.release_drain_listener();
        let _ = server.await;
    }

    #[tokio::test]
    async fn progress_only_reaches_opted_in_watchers() {
        let (state, dir) = test_state("progress-watch");
        let path = progress_socket_path(&dir);
        let listener = bind(&path).expect("bind");
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let server = tokio::spawn(run(listener, Arc::clone(&state), shutdown_rx));
        let mut legacy = Client::new(transport::connect(&path).await.unwrap());
        let mut typed = Client::new(transport::connect(&path).await.unwrap());
        assert!(legacy.call(request(1, Method::Watch)).await.ok);
        assert!(typed.call(request(2, Method::WatchRuntime)).await.ok);
        state
            .runtime_event_sender()
            .send(copypaste_ipc::RuntimeEventData {
                file_operation: None,
                change: None,
                capture: None,
                progress: Some(copypaste_ipc::FileProgress {
                    token: "opaque".into(),
                    written: 5,
                    total: 10,
                }),
            })
            .unwrap();
        let progress = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let frame = typed.next_frame().await;
                if let Some(ResponseData::RuntimeEvent(event)) = frame.data {
                    if let Some(progress) = event.progress {
                        break progress;
                    }
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(
            (progress.token.as_str(), progress.written, progress.total),
            ("opaque", 5, 10)
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(100), legacy.next_frame())
                .await
                .is_err()
        );
        state.note_remote_change();
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(5), legacy.next_frame())
                .await
                .unwrap()
                .data,
            Some(ResponseData::Event(_))
        ));
        shutdown_tx.send(true).unwrap();
        state.release_drain_listener();
        let _ = server.await;
    }

    #[tokio::test]
    async fn runtime_watch_lag_resynchronizes_capture_history_and_peers() {
        let (state, _dir) = test_state("runtime-watch-lag");
        let (_legacy_tx, legacy) = tokio::sync::broadcast::channel(1);
        let (runtime_tx, events) = tokio::sync::broadcast::channel(1);
        let empty = copypaste_ipc::RuntimeEventData {
            file_operation: None,
            change: None,
            progress: None,
            capture: None,
        };
        runtime_tx.send(empty.clone()).unwrap();
        runtime_tx.send(empty).unwrap();
        let (server_stream, client_stream) = Stream::pair().expect("socket pair");
        let (reader, mut writer) = server_stream.into_split();
        let mut reader = Reader::new(reader);
        let (_shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let pump = tokio::spawn(async move {
            crate::server::watch::pump_runtime(
                state,
                legacy,
                events,
                26,
                &mut reader,
                &mut writer,
                &mut shutdown_rx,
            )
            .await;
        });
        let mut client = Client::new(client_stream);
        let mut frames = Vec::new();
        for _ in 0..4 {
            frames.push(
                tokio::time::timeout(Duration::from_secs(5), client.next_frame())
                    .await
                    .unwrap(),
            );
        }
        let Some(ResponseData::RuntimeEvent(first)) = &frames[0].data else {
            panic!("missing initial state")
        };
        assert!(first.capture.is_some());
        let Some(ResponseData::RuntimeEvent(recovery)) = &frames[1].data else {
            panic!("missing recovery state")
        };
        assert!(recovery.capture.is_some());
        let Some(ResponseData::RuntimeEvent(items)) = &frames[2].data else {
            panic!("missing history refresh")
        };
        assert_eq!(
            items.change.as_ref().unwrap().event,
            copypaste_ipc::EventKind::Items
        );
        let Some(ResponseData::RuntimeEvent(peers)) = &frames[3].data else {
            panic!("missing peer refresh")
        };
        let peers = peers.change.as_ref().unwrap();
        assert_eq!(peers.event, copypaste_ipc::EventKind::Peers);
        assert!(peers.sync_status.is_some());
        drop(client);
        tokio::time::timeout(Duration::from_secs(5), pump)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn a_watcher_continues_after_lagging() {
        let (events_tx, events) = tokio::sync::broadcast::channel(1);
        events_tx
            .send(copypaste_ipc::EventData {
                sync_status: None,
                event: copypaste_ipc::EventKind::Items,
                item_count: 1,
                captured: false,
                captured_item_id: None,
            })
            .unwrap();
        events_tx
            .send(copypaste_ipc::EventData {
                sync_status: None,
                event: copypaste_ipc::EventKind::Items,
                item_count: 2,
                captured: true,
                captured_item_id: Some("captured-item".into()),
            })
            .unwrap();

        let (server_stream, client_stream) = Stream::pair().expect("socket pair");
        let (reader, mut writer) = server_stream.into_split();
        let mut reader = Reader::new(reader);
        let (_shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let pump = tokio::spawn(async move {
            crate::server::watch::pump(events, 26, &mut reader, &mut writer, &mut shutdown_rx)
                .await;
        });

        let mut client = Client::new(client_stream);
        let frame = tokio::time::timeout(Duration::from_secs(5), client.next_frame())
            .await
            .expect("the retained event must follow the lag");
        assert_eq!(frame.id, 26);
        match frame.data {
            Some(ResponseData::Event(event)) => {
                assert_eq!(event.item_count, 2);
                assert!(event.captured);
            }
            other => panic!("expected an event, got {other:?}"),
        }

        drop(client);
        drop(events_tx);
        tokio::time::timeout(Duration::from_secs(5), pump)
            .await
            .expect("the watcher must stop")
            .expect("the pump must not panic");
    }

    #[tokio::test]
    async fn a_watcher_stops_when_the_client_disconnects() {
        let (events_tx, events) = tokio::sync::broadcast::channel(1);
        let (server_stream, client_stream) = Stream::pair().expect("socket pair");
        let (reader, mut writer) = server_stream.into_split();
        let mut reader = Reader::new(reader);
        let (_shutdown_tx, mut shutdown_rx) = watch::channel(false);
        drop(client_stream);

        let pump = tokio::spawn(async move {
            crate::server::watch::pump(events, 27, &mut reader, &mut writer, &mut shutdown_rx)
                .await;
        });

        tokio::time::timeout(Duration::from_secs(5), pump)
            .await
            .expect("the disconnect must stop the watcher")
            .expect("the pump must not panic");
        drop(events_tx);
    }

    /// The order is the whole point: the acknowledgement is written *before*
    /// the signal, so a client learns the request was accepted rather than
    /// inferring it from a closed socket. Doing it the other way round races
    /// `run`'s teardown, which aborts the connection tasks — and the reply is
    /// what would be aborted.
    #[tokio::test]
    async fn shutdown_is_acknowledged_and_then_acted_on() {
        let (state, dir) = test_state("server");
        let path = test_socket_path(&dir, "daemon.sock");
        let listener = bind(&path).expect("bind");
        let server = tokio::spawn(run(listener, Arc::clone(&state), state.shutdown_rx()));

        let mut stopping = state.shutdown_rx();
        assert!(!*stopping.borrow(), "nothing has asked yet");

        let stream = transport::connect(&path).await.expect("connect");
        let reply = Client::new(stream).call(request(7, Method::Shutdown)).await;
        assert!(reply.ok, "{:?}", reply.error);
        assert_eq!(reply.id, 7, "a client matches replies by id");

        tokio::time::timeout(Duration::from_secs(5), stopping.changed())
            .await
            .expect("the signal must follow the reply")
            .expect("the sender outlives this");
        assert!(*stopping.borrow());

        // And the accept loop unwinds on it, exactly as it does on SIGTERM.
        state.release_drain_listener();
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
        let path = test_socket_path(&dir, "daemon.sock");
        let listener = bind(&path).expect("bind");
        let server = tokio::spawn(run(listener, Arc::clone(&state), state.shutdown_rx()));

        let stream = transport::connect(&path).await.expect("connect");
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

        let stream = transport::connect(&path).await.expect("connect");
        let reply = Client::new(stream).call(request(2, Method::Shutdown)).await;
        assert!(reply.ok, "{:?}", reply.error);

        state.release_drain_listener();
        let _ = tokio::time::timeout(Duration::from_secs(5), server).await;
    }

    /// Manifest 04 §6.2 / ADR-0004: a client speaking a version this daemon
    /// does not is *the* caller for this verb, so `protocol_mismatch` must not
    /// stand between it and the acknowledgement — that would leave the mismatch
    /// state with no way out.
    #[tokio::test]
    async fn shutdown_answers_through_a_protocol_mismatch() {
        let (state, dir) = test_state("server");
        let path = test_socket_path(&dir, "daemon.sock");
        let listener = bind(&path).expect("bind");
        let server = tokio::spawn(run(listener, Arc::clone(&state), state.shutdown_rx()));
        let mut stopping = state.shutdown_rx();

        let stream = transport::connect(&path).await.expect("connect");
        let mismatched = serde_json::json!({
            "id": 9,
            "protocol_version": PROTOCOL_VERSION + 1,
            "method": "shutdown",
        });
        let reply = Client::new(stream).call_raw(&mismatched.to_string()).await;
        assert!(reply.ok, "{:?}", reply.error);
        assert_eq!(reply.id, 9);

        tokio::time::timeout(Duration::from_secs(5), stopping.changed())
            .await
            .expect("the signal must follow the reply")
            .expect("the sender outlives this");
        assert!(*stopping.borrow());
        state.release_drain_listener();
        let _ = tokio::time::timeout(Duration::from_secs(5), server).await;
    }

    /// Manifest 04 §6.2: `request too large` is answered and the connection is
    /// then closed. It resumed at the next newline until DMY-158, which is a
    /// promise the Windows pipe could not keep — and the daemon's own
    /// `halted::refuse` never made it. The daemon keeps serving; only this
    /// connection ends, which is the state `request_too_large` already tells
    /// clients to expect.
    #[tokio::test]
    async fn an_oversized_line_is_refused_and_the_connection_is_closed() {
        let (state, dir) = test_state("server-oversized");
        let path = test_socket_path(&dir, "daemon.sock");
        let listener = bind(&path).expect("bind");
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let server = tokio::spawn(run(listener, Arc::clone(&state), shutdown_rx));

        let mut client = Client::new(transport::connect(&path).await.expect("connect"));
        assert!(client.call(request(1, Method::Status)).await.ok);

        client
            .writer
            .write_all(&vec![b'a'; MAX_FRAME_BYTES + 1])
            .await
            .expect("write");
        client.writer.write_all(b"\n").await.expect("write");

        let response = client.next_frame().await;
        assert_eq!(response.id, 0);
        assert_eq!(response.error_code, Some(ErrorCode::InvalidRequest));
        assert_eq!(response.error.as_deref(), Some(MSG_TOO_LARGE));
        assert!(
            !response.error.unwrap_or_default().contains('/'),
            "the privacy contract: no path in a user message"
        );
        assert_connection_closed(&mut client).await;

        let mut fresh = Client::new(transport::connect(&path).await.expect("connect"));
        assert!(
            fresh.call(request(2, Method::Status)).await.ok,
            "one client's oversized frame ended the daemon"
        );

        shutdown_tx.send(true).unwrap();
        state.release_drain_listener();
        let _ = server.await;
    }

    /// The cap is non-blocking, so an over-cap connection is dropped rather
    /// than queued behind a permit that may never come free.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn connections_past_the_cap_are_dropped_rather_than_queued() {
        let (state, dir) = test_state("server");
        let path = test_socket_path(&dir, "daemon.sock");
        let listener = bind(&path).expect("bind");
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let server = tokio::spawn(run(listener, Arc::clone(&state), shutdown_rx));

        // Fill every permit. Each one is made to answer a request first: a
        // connect alone only reaches the listen backlog, so without a round
        // trip the test would race the accept loop rather than the cap.
        let mut idle = Vec::new();
        for n in 0..MAX_CONCURRENT_CONNECTIONS {
            let mut client = Client::new(transport::connect(&path).await.expect("connect"));
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
        let stream = transport::connect(&path).await.expect("connect");
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
        state.release_drain_listener();
        let _ = server.await;
    }

    /// A minimal newline-JSON client, mirroring what the CLI does.
    struct Client {
        writer: OwnedWriteHalf,
        lines: FramedRead<OwnedReadHalf, LinesCodec>,
    }

    impl Client {
        fn new(stream: Stream) -> Self {
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

    async fn assert_connection_closed(client: &mut Client) {
        let next = tokio::time::timeout(Duration::from_secs(5), client.lines.next())
            .await
            .expect("the rejected watch must not remain open");
        assert!(
            matches!(next, None | Some(Err(_))),
            "the rejected watch produced another frame: {next:?}"
        );
    }

    #[tokio::test]
    async fn finished_connection_tasks_are_drained_from_the_join_set() {
        let mut connections = tokio::task::JoinSet::new();
        for _ in 0..8 {
            connections.spawn(async {});
        }
        for _ in 0..20 {
            if connections.is_empty() {
                break;
            }
            tokio::task::yield_now().await;
            while connections.try_join_next().is_some() {}
        }
        assert!(
            connections.is_empty(),
            "completed JoinSet tasks must be joinable so they do not leak"
        );
    }
}

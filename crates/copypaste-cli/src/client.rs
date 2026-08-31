//! The daemon client: one [`transport`] connection, one newline-delimited JSON
//! request, one typed response.
//!
//! Framing is [`tokio_util::codec::LinesCodec`], not a byte-scanning read loop —
//! the same codec the daemon uses, so there is exactly one framing
//! implementation on each side of the socket.
//!
//! Responses are deserialised straight into [`Response`] / [`ResponseData`]; a
//! missing field is a typed decode failure, never a silent default.

use crate::error::CliError;
use backon::{ConstantBuilder, Retryable as _, Sleeper};
use copypaste_ipc::transport;
use copypaste_ipc::{
    socket_path, BackupData, CloudStatusData, CloudSyncData, ConfigApplied, DiscoveredData,
    ErrorCode, EventData, ExportData, ImportData, Item, ItemPage, Method, PairingInviteData,
    PairingProgressData, PeerInfo, Request, Response, ResponseData, StatusData, SyncResult,
    MAX_FRAME_BYTES, PROTOCOL_VERSION,
};
use futures_util::{SinkExt, StreamExt};
use std::future::Future;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::time::{timeout_at, Instant};
use tokio_util::codec::{Framed, LinesCodec};

#[cfg(any(windows, test))]
use crate::shutdown::{self, Completion};

/// Port manifest 04 §6.2: `not_ready` is retried with a fixed 500 ms backoff,
/// at most three times, reconnecting each time. The cap is deliberately tight —
/// a persistently degraded daemon returns this code forever, and hanging is a
/// worse answer than "try again in a few seconds".
const NOT_READY_MAX_RETRIES: u32 = 3;
const NOT_READY_BACKOFF: Duration = Duration::from_millis(500);

/// One end-to-end budget covers connect, write, reply and readiness retries.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Manifest 04's long client budget, for the verbs
/// [`Method::is_long_running`] names: local I/O across a whole history, a round
/// trip over the network, or a step waiting on a human.
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(180);

/// A CLI invocation is one command, but a `not_ready` retry reconnects, so ids
/// still have to differ within the process.
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

struct Connection {
    framed: Framed<transport::Stream, LinesCodec>,
}

impl Connection {
    fn from_stream(stream: transport::Stream) -> Self {
        Self {
            framed: Framed::new(stream, LinesCodec::new_with_max_length(MAX_FRAME_BYTES)),
        }
    }

    async fn open(path: &Path, deadline: Instant) -> Result<Self, CliError> {
        // The path is never surfaced to the user: it discloses the local
        // username (AGENTS.md rule 4). Every failure here collapses to one
        // pathless variant.
        let stream = before(deadline, transport::connect(path))
            .await?
            .map_err(|_| CliError::DaemonUnreachable)?;
        Ok(Self::from_stream(stream))
    }

    async fn call(&mut self, method: Method, deadline: Instant) -> Result<Response, CliError> {
        let id = next_id();
        let request = Request {
            id,
            protocol_version: PROTOCOL_VERSION,
            method,
        };
        let line = serde_json::to_string(&request)
            .map_err(|e| CliError::local(format!("could not encode the request: {e}")))?;

        before(deadline, self.framed.send(line))
            .await?
            .map_err(|_| CliError::DaemonUnreachable)?;

        let line = match before(deadline, self.framed.next()).await? {
            Some(Ok(line)) => line,
            // A dropped connection with no reply is the daemon's read-timeout
            // behaviour, and is indistinguishable from it having gone away.
            Some(Err(_)) | None => return Err(CliError::DaemonUnreachable),
        };

        let response: Response = serde_json::from_str(&line).map_err(|e| {
            CliError::local(format!("could not understand the daemon's reply: {e}"))
        })?;

        if response.id != id {
            return Err(CliError::local(format!(
                "the daemon answered request {} with a reply for request {}",
                id, response.id
            )));
        }
        Ok(response)
    }
}

/// Run one step of a request against the deadline the whole request shares.
///
/// A blown deadline is [`CliError::DaemonTimeout`], never `DaemonUnreachable`.
/// Reaching this point means a listener took the connection, so the daemon is
/// there; only an outright connect *error* means it is not, and that is mapped
/// by the callers below.
async fn before<T>(deadline: Instant, future: impl Future<Output = T>) -> Result<T, CliError> {
    timeout_at(deadline, future)
        .await
        .map_err(|_| CliError::DaemonTimeout)
}

/// The classification is [`copypaste_ipc`]'s, not this crate's.
///
/// It used to be a `match` here naming three methods, so `sync`, `cloud sync`
/// and `pair` ran on the five-second budget: a sync over a slow LAN reported a
/// failure the daemon had not had — and reported it as *unreachable*, about a
/// daemon that was working. One list, shared with the app (AGENTS.md rule 1).
fn timeout_for(method: &Method) -> Duration {
    if method.is_long_running() {
        TRANSFER_TIMEOUT
    } else {
        REQUEST_TIMEOUT
    }
}

fn not_ready_policy() -> ConstantBuilder {
    ConstantBuilder::new()
        .with_delay(NOT_READY_BACKOFF)
        .with_max_times(NOT_READY_MAX_RETRIES as usize)
}

enum RequestAttemptError {
    NotReady(Box<Response>),
    Fatal(CliError),
}

async fn one_request_attempt<C>(
    connect: C,
    method: Method,
    deadline: Instant,
) -> Result<Response, RequestAttemptError>
where
    C: Future<Output = io::Result<transport::Stream>>,
{
    let stream = before(deadline, connect)
        .await
        .map_err(RequestAttemptError::Fatal)?
        .map_err(|_| RequestAttemptError::Fatal(CliError::DaemonUnreachable))?;
    let mut connection = Connection::from_stream(stream);
    let response = connection
        .call(method, deadline)
        .await
        .map_err(RequestAttemptError::Fatal)?;

    if response.error_code == Some(ErrorCode::NotReady) {
        Err(RequestAttemptError::NotReady(Box::new(response)))
    } else {
        Ok(response)
    }
}

/// Send one request to the running daemon, honouring the `not_ready` retry
/// contract.
pub async fn request(method: Method) -> Result<Response, CliError> {
    request_at(&socket_path(), method).await
}

/// Ask the Windows daemon to stop and prove that its connected process exited.
#[cfg(windows)]
pub async fn shutdown_and_wait() -> Result<Completion, CliError> {
    shutdown_and_wait_at(&socket_path()).await
}

#[cfg(windows)]
async fn shutdown_and_wait_at(path: &Path) -> Result<Completion, CliError> {
    shutdown_and_wait_with(|| transport::connect(path), shutdown::open_for_stream).await
}

#[cfg(any(windows, test))]
async fn shutdown_and_wait_with<C, F, P>(
    connect: C,
    open_process: impl FnOnce(&transport::Stream, Instant) -> Result<P, CliError>,
) -> Result<Completion, CliError>
where
    C: FnOnce() -> F,
    F: Future<Output = io::Result<transport::Stream>>,
    P: shutdown::ProcessWait,
{
    let deadline = Instant::now() + REQUEST_TIMEOUT;
    let stream = match before(deadline, connect()).await? {
        Ok(stream) => stream,
        Err(err) => return initial_shutdown_connect_error(err),
    };
    let mut connection = Connection::from_stream(stream);
    let process = open_process(connection.framed.get_ref(), deadline)?;
    let response = connection
        .call(Method::Shutdown, deadline)
        .await
        .map_err(completion_error)?;

    validate_shutdown_ack(response)?;
    shutdown::wait_for_exit_with(process, deadline)
}

#[cfg(any(windows, test))]
fn initial_shutdown_connect_error(error: io::Error) -> Result<Completion, CliError> {
    if error.kind() == io::ErrorKind::NotFound {
        Ok(Completion::AlreadyStopped)
    } else if error.kind() == io::ErrorKind::TimedOut {
        Err(CliError::DaemonTimeout)
    } else {
        Err(shutdown::completion_failure())
    }
}

#[cfg(any(windows, test))]
fn validate_shutdown_ack(response: Response) -> Result<(), CliError> {
    match into_data(response).map_err(completion_error)? {
        Some(ResponseData::Empty {}) => Ok(()),
        _ => Err(shutdown::completion_failure()),
    }
}

#[cfg(any(windows, test))]
fn completion_error(error: CliError) -> CliError {
    match error {
        CliError::DaemonTimeout => error,
        _ => shutdown::completion_failure(),
    }
}

async fn request_at(path: &Path, method: Method) -> Result<Response, CliError> {
    let timeout = timeout_for(&method);
    request_at_with_timeout(path, method, timeout).await
}

async fn request_at_with_timeout(
    path: &Path,
    method: Method,
    timeout: Duration,
) -> Result<Response, CliError> {
    request_with_connector_and_sleep(
        method,
        timeout,
        || transport::connect(path),
        tokio::time::sleep,
    )
    .await
}

async fn request_with_connector_and_sleep<C, F, S>(
    method: Method,
    timeout: Duration,
    mut connect: C,
    sleep: S,
) -> Result<Response, CliError>
where
    C: FnMut() -> F,
    F: Future<Output = io::Result<transport::Stream>>,
    S: Sleeper,
{
    let deadline = Instant::now() + timeout;
    let call = || {
        let method = method.clone();
        one_request_attempt(connect(), method, deadline)
    };
    let result = timeout_at(
        deadline,
        call.retry(not_ready_policy())
            .when(|err| matches!(err, RequestAttemptError::NotReady(_)))
            .sleep(sleep),
    )
    .await
    .map_err(|_| CliError::DaemonTimeout)?;

    match result {
        Ok(response) => Ok(response),
        Err(RequestAttemptError::NotReady(response)) => Ok(*response),
        Err(RequestAttemptError::Fatal(err)) => Err(err),
    }
}

/// Subscribe to the daemon's change stream and hand each event to `on_event`.
///
/// Returns when the daemon stops or the callback asks to stop. The connection
/// is deliberately not the one [`request`] uses: a subscription lives for as
/// long as the client wants it to, and mixing it with request/response would
/// make replies and events ambiguous on one stream.
pub async fn watch(mut on_event: impl FnMut(EventData) -> bool) -> Result<(), CliError> {
    watch_at(&socket_path(), REQUEST_TIMEOUT, &mut on_event).await
}

async fn watch_at(
    path: &Path,
    handshake_timeout: Duration,
    on_event: &mut impl FnMut(EventData) -> bool,
) -> Result<(), CliError> {
    let deadline = Instant::now() + handshake_timeout;
    let mut connection = Connection::open(path, deadline).await?;
    // The ack proves the daemon subscribed before answering, so nothing that
    // happens after this line is missed.
    let ack = connection.call(Method::Watch, deadline).await?;
    let subscription_id = ack.id;
    into_data(ack)?;

    // The subscription owns no deadline after its acknowledgement. Silence is
    // normal here; user interruption or the callback ends the stream.
    loop {
        let line = match connection.framed.next().await {
            Some(Ok(line)) => line,
            Some(Err(_)) | None => return Err(CliError::DaemonUnreachable),
        };
        let response: Response = serde_json::from_str(&line).map_err(|e| {
            CliError::local(format!("could not understand the daemon's reply: {e}"))
        })?;
        if response.id != subscription_id {
            return Err(CliError::local(format!(
                "the daemon answered request {} with a reply for request {}",
                subscription_id, response.id
            )));
        }
        match into_data(response)? {
            Some(ResponseData::Event(event)) => {
                if !on_event(event) {
                    return Ok(());
                }
            }
            _ => return Err(shape_error("a change event")),
        }
    }
}

/// Split a response into success payload or typed failure.
///
/// Branches on `ok` and `error_code`, never on the `error` string
/// (port manifest 04, I9).
pub fn into_data(response: Response) -> Result<Option<ResponseData>, CliError> {
    if response.ok {
        return Ok(response.data);
    }
    Err(CliError::Daemon {
        code: response.error_code,
        raw_code: response.raw_error_code,
        message: response
            .error
            .unwrap_or_else(|| "the daemon reported a failure but gave no reason".to_string()),
    })
}

fn shape_error(expected: &str) -> CliError {
    CliError::local(format!(
        "the daemon replied with something other than {expected}; \
         the daemon and this CLI may be different versions"
    ))
}

/// A response that must carry a page of items.
pub fn expect_page(data: Option<ResponseData>) -> Result<ItemPage, CliError> {
    match data {
        Some(ResponseData::Page(page)) => Ok(page),
        _ => Err(shape_error("a page of items")),
    }
}

/// A response that must carry an export.
pub fn expect_export(data: Option<ResponseData>) -> Result<ExportData, CliError> {
    match data {
        Some(ResponseData::Export(export)) => Ok(export),
        _ => Err(shape_error("an export")),
    }
}

/// A response that must carry import counts.
pub fn expect_import(data: Option<ResponseData>) -> Result<ImportData, CliError> {
    match data {
        Some(ResponseData::Import(result)) => Ok(result),
        _ => Err(shape_error("import results")),
    }
}

/// A response that must carry a completed backup.
pub fn expect_backup(data: Option<ResponseData>) -> Result<BackupData, CliError> {
    match data {
        Some(ResponseData::Backup(backup)) => Ok(backup),
        _ => Err(shape_error("a backup result")),
    }
}

/// A response that must carry discovered devices.
pub fn expect_discovered(data: Option<ResponseData>) -> Result<DiscoveredData, CliError> {
    match data {
        Some(ResponseData::Discovered(found)) => Ok(found),
        _ => Err(shape_error("discovered devices")),
    }
}

/// A response that must carry the daemon's settings.
pub fn expect_config(data: Option<ResponseData>) -> Result<ConfigApplied, CliError> {
    match data {
        Some(ResponseData::Config(applied)) => Ok(applied),
        _ => Err(shape_error("daemon settings")),
    }
}

/// A response that must carry daemon status.
pub fn expect_status(data: Option<ResponseData>) -> Result<StatusData, CliError> {
    match data {
        Some(ResponseData::Status(status)) => Ok(status),
        _ => Err(shape_error("daemon status")),
    }
}

/// The item a mutation returned, when it returned one.
///
/// `add` may answer with the stored item or with an acknowledgement; both are
/// valid, so this is an option rather than a requirement.
pub fn optional_item(data: &Option<ResponseData>) -> Option<&Item> {
    match data {
        Some(ResponseData::Item(item)) => Some(item),
        _ => None,
    }
}

/// The count a mutation returned, when it returned one.
pub fn optional_count(data: &Option<ResponseData>) -> Option<u64> {
    match data {
        Some(ResponseData::Count(count)) => Some(*count),
        _ => None,
    }
}

/// A response that must carry a freshly minted pairing.
pub fn expect_pairing(data: Option<ResponseData>) -> Result<PairingInviteData, CliError> {
    match data {
        Some(ResponseData::PairingInvite(pairing)) => Ok(pairing),
        _ => Err(shape_error("a pairing")),
    }
}

pub fn expect_pairing_progress(
    data: Option<ResponseData>,
) -> Result<PairingProgressData, CliError> {
    match data {
        Some(ResponseData::PairingProgress(progress)) => Ok(progress),
        _ => Err(shape_error("pairing progress")),
    }
}

/// A response that must carry a list of peers.
pub fn expect_peers(data: Option<ResponseData>) -> Result<Vec<PeerInfo>, CliError> {
    match data {
        Some(ResponseData::Peers(peers)) => Ok(peers),
        _ => Err(shape_error("a list of peers")),
    }
}

/// A response that must carry cloud sync status.
pub fn expect_cloud_status(data: Option<ResponseData>) -> Result<CloudStatusData, CliError> {
    match data {
        Some(ResponseData::CloudStatus(status)) => Ok(status),
        _ => Err(shape_error("cloud sync status")),
    }
}

/// A response that must carry the counts from one cloud round.
pub fn expect_cloud_sync(data: Option<ResponseData>) -> Result<CloudSyncData, CliError> {
    match data {
        Some(ResponseData::CloudSync(stats)) => Ok(stats),
        _ => Err(shape_error("cloud sync results")),
    }
}

/// A response that must carry per-peer sync results. Empty lists as above.
pub fn expect_sync(data: Option<ResponseData>) -> Result<Vec<SyncResult>, CliError> {
    match data {
        Some(ResponseData::Sync(results)) => Ok(results),
        _ => Err(shape_error("sync results")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use backon::BackoffBuilder;
    use std::collections::VecDeque;
    use std::future::{pending, ready};
    use std::sync::{Arc, Mutex};

    // The stub daemon binds a real endpoint, which only Unix has one of yet.
    // Everything below that decodes a reply is platform-independent and stays
    // ungated, so the wire contract is still covered on Windows.
    #[cfg(unix)]
    use std::path::PathBuf;
    #[cfg(unix)]
    use tokio::io::AsyncWriteExt;
    #[cfg(unix)]
    use tokio::net::UnixListener;

    enum PairAction {
        Reply(String),
        ReplyThenHold(String, tokio::sync::oneshot::Sender<()>),
        NeverReply,
    }

    struct ExitedProcess(Arc<Mutex<Vec<&'static str>>>);

    impl shutdown::ProcessWait for ExitedProcess {
        fn wait(&mut self, _milliseconds: u32) -> Result<shutdown::WaitResult, ()> {
            self.0.lock().unwrap().push("wait");
            Ok(shutdown::WaitResult::Object)
        }

        fn exit_code(&self) -> Result<u32, ()> {
            Ok(0)
        }
    }

    async fn stream_pair() -> io::Result<(transport::Stream, transport::Stream)> {
        #[cfg(unix)]
        {
            transport::Stream::pair()
        }
        #[cfg(windows)]
        {
            transport::Stream::pair().await
        }
        #[cfg(not(any(unix, windows)))]
        {
            Err(transport::unsupported())
        }
    }

    async fn pair_client(action: PairAction, ids: Arc<Mutex<Vec<u64>>>) -> transport::Stream {
        let (server, client) = stream_pair().await.expect("stream pair");
        tokio::spawn(async move {
            let mut framed = Framed::new(server, LinesCodec::new());
            let Some(Ok(line)) = framed.next().await else {
                return;
            };
            let request: Request = serde_json::from_str(&line).expect("valid request");
            ids.lock().unwrap().push(request.id);
            match action {
                PairAction::Reply(reply) => {
                    let reply = reply.replace("{id}", &request.id.to_string());
                    let _ = framed.send(reply).await;
                }
                PairAction::ReplyThenHold(reply, acknowledged) => {
                    let reply = reply.replace("{id}", &request.id.to_string());
                    let _ = framed.send(reply).await;
                    let _ = acknowledged.send(());
                    hold(framed).await;
                }
                PairAction::NeverReply => {
                    hold(framed).await;
                }
            }
        });
        client
    }

    async fn paired_request(
        method: Method,
        actions: impl IntoIterator<Item = PairAction>,
        timeout: Duration,
        sleep: impl Sleeper,
    ) -> (Result<Response, CliError>, Vec<u64>) {
        let ids = Arc::new(Mutex::new(Vec::new()));
        let mut clients = VecDeque::new();
        for action in actions {
            clients.push_back(pair_client(action, Arc::clone(&ids)).await);
        }
        let clients = Arc::new(Mutex::new(clients));
        let result = request_with_connector_and_sleep(
            method,
            timeout,
            {
                let clients = Arc::clone(&clients);
                move || {
                    let client = clients
                        .lock()
                        .unwrap()
                        .pop_front()
                        .expect("one stream per attempt");
                    ready(Ok(client))
                }
            },
            sleep,
        )
        .await;
        let ids = ids.lock().unwrap().clone();
        (result, ids)
    }

    #[cfg(unix)]
    enum StubAction {
        Reply(String),
        Close,
        NeverRead,
        NeverReply,
        Partial(String),
        Watch {
            ack: String,
            event: String,
            event_delay: Duration,
        },
    }

    #[cfg(unix)]
    struct StubDaemon {
        path: PathBuf,
        request_ids: Arc<Mutex<Vec<u64>>>,
        task: tokio::task::JoinHandle<()>,
    }

    async fn hold<T>(value: T) {
        std::future::pending::<()>().await;
        drop(value);
    }

    #[cfg(unix)]
    impl StubDaemon {
        fn start(name: &str, actions: Vec<StubAction>) -> Self {
            let path = std::env::temp_dir().join(format!(
                "copypaste-cli-test-{}-{name}.sock",
                std::process::id()
            ));
            let _ = std::fs::remove_file(&path);
            let listener = UnixListener::bind(&path).expect("bind stub socket");
            let request_ids = Arc::new(Mutex::new(Vec::new()));
            let recorded_ids = Arc::clone(&request_ids);
            let task = tokio::spawn(async move {
                for action in actions {
                    let Ok((stream, _)) = listener.accept().await else {
                        return;
                    };
                    let action = match action {
                        StubAction::NeverRead => {
                            hold(stream).await;
                            return;
                        }
                        action => action,
                    };
                    let mut framed = Framed::new(stream, LinesCodec::new());
                    let Some(Ok(line)) = framed.next().await else {
                        return;
                    };
                    let request: Request = serde_json::from_str(&line).expect("valid request");
                    recorded_ids.lock().unwrap().push(request.id);
                    let fill_id = |line: String| line.replace("{id}", &request.id.to_string());
                    match action {
                        StubAction::Reply(reply) => {
                            let _ = framed.send(fill_id(reply)).await;
                        }
                        StubAction::Close => {}
                        StubAction::NeverReply => {
                            hold(framed).await;
                            return;
                        }
                        StubAction::Partial(partial) => {
                            let mut stream = framed.into_inner();
                            let _ = stream.write_all(fill_id(partial).as_bytes()).await;
                            hold(stream).await;
                            return;
                        }
                        StubAction::Watch {
                            ack,
                            event,
                            event_delay,
                        } => {
                            let _ = framed.send(fill_id(ack)).await;
                            tokio::time::sleep(event_delay).await;
                            let _ = framed.send(fill_id(event)).await;
                        }
                        StubAction::NeverRead => unreachable!(),
                    }
                }
            });
            Self {
                path,
                request_ids,
                task,
            }
        }

        fn ids(&self) -> Vec<u64> {
            self.request_ids.lock().unwrap().clone()
        }
    }

    #[cfg(unix)]
    impl Drop for StubDaemon {
        fn drop(&mut self) {
            self.task.abort();
            let _ = std::fs::remove_file(&self.path);
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_full_request_response_round_trip_yields_typed_items() {
        let stub = StubDaemon::start(
            "roundtrip",
            vec![StubAction::Reply(
                r#"{"id":{id},"ok":true,"data":{"page":{"items":[{"id":"a","content":"hi","content_type":"text/plain","created_at":5,"pinned":false,"is_sensitive":false}],"skipped_undecryptable":0}}}"#
                    .to_string(),
            )],
        );

        let response = request_at(
            &stub.path,
            Method::List {
                limit: 1,
                cursor: None,
            },
        )
        .await
        .expect("round trip");
        let response_id = response.id;
        let page = expect_page(into_data(response).unwrap()).unwrap();
        assert_eq!(page.items[0].content, "hi");
        assert_eq!(stub.ids(), [response_id]);
    }

    #[tokio::test]
    async fn destructive_calls_reject_wrong_id_and_payloadless_success_replies() {
        for reply in [
            r#"{"id":999,"ok":true,"data":{"empty":{}}}"#,
            r#"{"id":{id},"ok":true}"#,
        ] {
            let (result, ids) = paired_request(
                Method::DeleteAll { through: None },
                [PairAction::Reply(reply.into())],
                REQUEST_TIMEOUT,
                |_| ready(()),
            )
            .await;

            assert!(result.is_err(), "{reply}");
            assert_eq!(ids.len(), 1);
        }
    }

    #[tokio::test]
    async fn a_missing_socket_is_reported_as_an_unreachable_daemon() {
        let missing = std::env::temp_dir().join("copypaste-cli-test-absent.sock");
        let _ = std::fs::remove_file(&missing);
        let err = request_at(&missing, Method::Status).await.unwrap_err();
        assert_eq!(err.exit_code(), crate::error::EXIT_UNREACHABLE);
        assert!(!err.user_message().contains('/'), "{err}");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn hanging_up_without_answering_is_treated_as_unreachable() {
        let stub = StubDaemon::start("hangup", vec![StubAction::Close]);
        let err = request_at(&stub.path, Method::Status).await.unwrap_err();
        assert_eq!(err.exit_code(), crate::error::EXIT_UNREACHABLE);
    }

    /// A blown deadline against a live stub is a timeout, not an unreachable
    /// daemon: the stub accepted the connection, so it is plainly running.
    fn assert_deadline_error_is_prompt_and_pathless(err: &CliError, started: Instant, path: &Path) {
        assert_eq!(err.exit_code(), crate::error::EXIT_TIMEOUT);
        assert!(matches!(err, CliError::DaemonTimeout), "{err:?}");
        assert!(started.elapsed() < Duration::from_secs(2));
        let message = err.user_message();
        assert!(!message.contains(&*path.to_string_lossy()), "{message}");
        assert!(!message.contains('/'), "{message}");
        assert!(!message.contains("cannot reach"), "{message}");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn an_accepted_socket_that_never_reads_has_a_write_deadline() {
        let stub = StubDaemon::start("never-read", vec![StubAction::NeverRead]);
        let started = Instant::now();
        let err = request_at_with_timeout(
            &stub.path,
            Method::Add {
                content: "x".repeat(512 * 1024),
            },
            Duration::from_millis(100),
        )
        .await
        .unwrap_err();
        assert_deadline_error_is_prompt_and_pathless(&err, started, &stub.path);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn an_accepted_request_that_never_gets_a_reply_has_a_read_deadline() {
        let stub = StubDaemon::start("never-reply", vec![StubAction::NeverReply]);
        let started = Instant::now();
        let err = request_at_with_timeout(&stub.path, Method::Status, Duration::from_millis(100))
            .await
            .unwrap_err();
        assert_deadline_error_is_prompt_and_pathless(&err, started, &stub.path);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_partial_reply_without_a_newline_has_a_read_deadline() {
        let stub = StubDaemon::start(
            "partial",
            vec![StubAction::Partial(r#"{"id":{id},"ok":true"#.to_string())],
        );
        let started = Instant::now();
        let err = request_at_with_timeout(&stub.path, Method::Status, Duration::from_millis(100))
            .await
            .unwrap_err();
        assert_deadline_error_is_prompt_and_pathless(&err, started, &stub.path);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn watch_acknowledgement_has_a_deadline() {
        let stub = StubDaemon::start("watch-no-ack", vec![StubAction::NeverReply]);
        let started = Instant::now();
        let mut callback = |_| true;
        let err = watch_at(&stub.path, Duration::from_millis(100), &mut callback)
            .await
            .unwrap_err();
        assert_deadline_error_is_prompt_and_pathless(&err, started, &stub.path);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn watch_stream_is_unbounded_after_the_acknowledgement() {
        let stub = StubDaemon::start(
            "watch-stream",
            vec![StubAction::Watch {
                ack: r#"{"id":{id},"ok":true,"data":{"empty":{}}}"#.to_string(),
                event: r#"{"id":{id},"ok":true,"data":{"event":{"event":"items","item_count":7}}}"#
                    .to_string(),
                event_delay: Duration::from_millis(300),
            }],
        );
        let mut observed = None;
        let mut callback = |event| {
            observed = Some(event);
            false
        };

        watch_at(&stub.path, Duration::from_millis(150), &mut callback)
            .await
            .expect("post-ack silence must not time out");
        assert_eq!(observed.unwrap().item_count, 7);
    }

    /// The network verbs are the ones this test was missing. On the five-second
    /// budget a `sync` across a slow LAN, a cloud round or a pairing handshake
    /// failed while the daemon was still working, and failed as *unreachable*.
    #[test]
    fn history_transfers_and_network_rounds_receive_the_long_deadline() {
        for method in [
            Method::Import { items: Vec::new() },
            Method::Backup {
                dest_path: "backup.db".into(),
            },
            Method::Restore {
                src_path: "backup.db".into(),
                confirm: true,
            },
            Method::SyncNow { pairing_id: None },
            Method::CloudSyncNow,
            Method::CloudSignIn {
                email: "a@b.c".into(),
                password: "p".into(),
                passphrase: "s".into(),
            },
            Method::CloudSignUp {
                email: "a@b.c".into(),
                password: "p".into(),
                passphrase: "s".into(),
            },
            Method::PairJoin {
                code: "code".into(),
                addr: "127.0.0.1:1".into(),
            },
            Method::PairConfirm { accept: true },
        ] {
            assert_eq!(timeout_for(&method), TRANSFER_TIMEOUT, "{method:?}");
        }
        // Both read a cache the daemon already holds. `Rescan` republishes an
        // mDNS record on the way past and still blocks on nothing.
        for method in [
            Method::Status,
            Method::Discovered,
            Method::Rescan,
            Method::CloudStatus,
            Method::Export {
                limit: 0,
                include_sensitive: false,
            },
        ] {
            assert_eq!(timeout_for(&method), REQUEST_TIMEOUT, "{method:?}");
        }
    }

    #[test]
    fn not_ready_policy_is_fixed_and_attempt_exact() {
        let mut policy = not_ready_policy().build();
        assert_eq!(policy.next(), Some(NOT_READY_BACKOFF));
        assert_eq!(policy.next(), Some(NOT_READY_BACKOFF));
        assert_eq!(policy.next(), Some(NOT_READY_BACKOFF));
        assert_eq!(policy.next(), None);
    }

    #[tokio::test]
    async fn not_ready_backon_retry_reconnects_and_preserves_attempt_count() {
        let sleeps = Arc::new(Mutex::new(Vec::new()));
        let sleep_log = Arc::clone(&sleeps);
        let not_ready =
            r#"{"id":{id},"ok":false,"error":"still starting","error_code":"not_ready"}"#;
        let ok = r#"{"id":{id},"ok":true,"data":{"status":{"version":"2.0.0","protocol_version":1,"item_count":0,"capture_running":true,"clipboard_backend":"fake","private_mode_epoch":0}}}"#;

        let (response, ids) = paired_request(
            Method::Status,
            [
                PairAction::Reply(not_ready.to_string()),
                PairAction::Reply(not_ready.to_string()),
                PairAction::Reply(not_ready.to_string()),
                PairAction::Reply(ok.to_string()),
            ],
            REQUEST_TIMEOUT,
            move |delay| {
                sleep_log.lock().unwrap().push(delay);
                ready(())
            },
        )
        .await;

        let status = expect_status(into_data(response.expect("response")).unwrap()).unwrap();
        assert_eq!(status.item_count, 0);
        assert_eq!(ids.len(), 4);
        assert_eq!(
            *sleeps.lock().unwrap(),
            [NOT_READY_BACKOFF, NOT_READY_BACKOFF, NOT_READY_BACKOFF]
        );
    }

    #[tokio::test]
    async fn not_ready_deadline_caps_the_backon_sleep() {
        let not_ready =
            r#"{"id":{id},"ok":false,"error":"still starting","error_code":"not_ready"}"#;
        let started = Instant::now();
        let (err, ids) = paired_request(
            Method::Status,
            [PairAction::Reply(not_ready.to_string())],
            Duration::from_millis(10),
            |_| pending::<()>(),
        )
        .await;
        let err = err.unwrap_err();

        assert_deadline_error_is_prompt_and_pathless(
            &err,
            started,
            Path::new("C:/Users/someone/AppData/Roaming/CopyPaste/daemon.sock"),
        );
        assert_eq!(ids.len(), 1);
    }

    #[tokio::test]
    async fn not_ready_exhaustion_returns_the_daemon_response_after_four_attempts() {
        let reply = r#"{"id":{id},"ok":false,"error":"still starting","error_code":"not_ready"}"#;
        let (response, ids) = paired_request(
            Method::Status,
            (0..=NOT_READY_MAX_RETRIES).map(|_| PairAction::Reply(reply.to_string())),
            REQUEST_TIMEOUT,
            |_| ready(()),
        )
        .await;
        let err = into_data(response.expect("final not-ready response")).unwrap_err();

        assert_eq!(ids.len(), 4);
        assert_eq!(err.exit_code(), crate::error::EXIT_OTHER);
        assert!(err.user_message().contains("starting up"), "{err}");
    }

    #[tokio::test]
    async fn non_not_ready_errors_do_not_retry() {
        let mismatch = r#"{"id":{id},"ok":false,"error":"unsupported protocol version","error_code":"protocol_mismatch"}"#;
        let (response, ids) = paired_request(
            Method::Status,
            [PairAction::Reply(mismatch.to_string())],
            REQUEST_TIMEOUT,
            |_| async { panic!("protocol mismatch must not sleep for retry") },
        )
        .await;
        let err = into_data(response.expect("mismatch response")).unwrap_err();

        assert_eq!(ids.len(), 1);
        assert!(err.user_message().contains("versions differ"), "{err}");
    }

    #[tokio::test]
    async fn fatal_attempt_errors_do_not_retry() {
        let (err, ids) = paired_request(
            Method::Status,
            [PairAction::NeverReply],
            Duration::from_millis(10),
            |_| async { panic!("unreachable attempts must not sleep for retry") },
        )
        .await;
        let err = err.unwrap_err();

        assert_eq!(ids.len(), 1);
        // The stub answered the connection and then went quiet, so this is a
        // timeout — not "unreachable", which would send the user to start a
        // daemon that is plainly already there.
        assert_eq!(err.exit_code(), crate::error::EXIT_TIMEOUT);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn ordinary_shutdown_still_returns_after_its_acknowledgement() {
        let (acknowledged, waiting) = tokio::sync::oneshot::channel();
        let mut task = tokio::spawn(paired_request(
            Method::Shutdown,
            [PairAction::ReplyThenHold(
                r#"{"id":{id},"ok":true,"data":{"empty":{}}}"#.to_string(),
                acknowledged,
            )],
            REQUEST_TIMEOUT,
            |_| ready(()),
        ));
        waiting.await.expect("the daemon sent its shutdown ACK");
        let completed = tokio::time::timeout(Duration::from_millis(50), &mut task).await;
        task.abort();
        assert!(
            completed.is_ok(),
            "ordinary shutdown remains acknowledgement-only"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn installer_shutdown_opens_then_writes_acknowledges_and_waits_once() {
        let (server, client) = stream_pair().await.expect("stream pair");
        let events = Arc::new(Mutex::new(Vec::new()));
        let server_events = Arc::clone(&events);
        tokio::spawn(async move {
            let mut framed = Framed::new(server, LinesCodec::new());
            let line = framed.next().await.expect("request").expect("frame");
            let request: Request = serde_json::from_str(&line).expect("shutdown request");
            server_events.lock().unwrap().push("write");
            server_events.lock().unwrap().push("ack");
            framed
                .send(format!(
                    r#"{{"id":{},"ok":true,"data":{{"empty":{{}}}}}}"#,
                    request.id
                ))
                .await
                .expect("ack");
        });

        let opened_events = Arc::clone(&events);
        let completion = shutdown_and_wait_with(
            || ready(Ok(client)),
            move |_stream, _deadline| {
                opened_events.lock().unwrap().push("open");
                Ok(ExitedProcess(Arc::clone(&opened_events)))
            },
        )
        .await;

        assert!(matches!(completion, Ok(Completion::ExitedZero)));
        assert_eq!(*events.lock().unwrap(), ["open", "write", "ack", "wait"]);
    }

    #[test]
    fn only_an_initial_not_found_is_an_already_stopped_success() {
        assert!(matches!(
            initial_shutdown_connect_error(io::Error::from(io::ErrorKind::NotFound)),
            Ok(Completion::AlreadyStopped)
        ));
        for kind in [
            io::ErrorKind::PermissionDenied,
            io::ErrorKind::ConnectionRefused,
            io::ErrorKind::UnexpectedEof,
        ] {
            let err = initial_shutdown_connect_error(io::Error::from(kind)).unwrap_err();
            assert_eq!(err.exit_code(), crate::error::EXIT_OTHER, "{kind:?}");
        }
        let timed_out =
            initial_shutdown_connect_error(io::Error::from(io::ErrorKind::TimedOut)).unwrap_err();
        assert_eq!(timed_out.exit_code(), crate::error::EXIT_TIMEOUT);
    }

    #[test]
    fn shutdown_requires_the_typed_empty_acknowledgement() {
        assert!(validate_shutdown_ack(Response::ok(7, ResponseData::Empty {})).is_ok());
        for response in [
            Response::ok(7, ResponseData::Count(0)),
            Response::err(7, ErrorCode::Internal, "shutdown failed"),
        ] {
            assert!(validate_shutdown_ack(response).is_err());
        }
    }

    #[test]
    fn completion_preserves_only_the_timeout_exit_code() {
        assert_eq!(
            completion_error(CliError::DaemonTimeout).exit_code(),
            crate::error::EXIT_TIMEOUT
        );
        assert_eq!(
            completion_error(CliError::DaemonUnreachable).exit_code(),
            crate::error::EXIT_OTHER
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_reply_for_a_different_request_is_rejected() {
        let stub = StubDaemon::start(
            "idmismatch",
            vec![StubAction::Reply(
                r#"{"id":999999,"ok":true,"data":{"empty":{}}}"#.to_string(),
            )],
        );
        let err = request_at(&stub.path, Method::Status).await.unwrap_err();
        assert_eq!(err.exit_code(), crate::error::EXIT_OTHER);
        assert!(err.user_message().contains("reply for request"), "{err}");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn not_ready_is_retried_on_a_fresh_connection() {
        let stub = StubDaemon::start(
            "notready",
            vec![
                StubAction::Reply(
                    r#"{"id":{id},"ok":false,"error":"still starting","error_code":"not_ready"}"#
                        .to_string(),
                ),
                StubAction::Reply(
                    r#"{"id":{id},"ok":true,"data":{"status":{"version":"2.0.0","protocol_version":1,"item_count":0,"capture_running":true,"clipboard_backend":"fake","private_mode_epoch":0}}}"#
                        .to_string(),
                ),
            ],
        );

        let response = request_at(&stub.path, Method::Status).await.expect("retry");
        let status = expect_status(into_data(response).unwrap()).unwrap();
        assert_eq!(status.item_count, 0);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn not_ready_gives_up_rather_than_retrying_forever() {
        let reply = r#"{"id":{id},"ok":false,"error":"still starting","error_code":"not_ready"}"#
            .to_string();
        let stub = StubDaemon::start(
            "notready-exhausted",
            (0..=NOT_READY_MAX_RETRIES)
                .map(|_| StubAction::Reply(reply.clone()))
                .collect(),
        );

        let response = request_at(&stub.path, Method::Status).await.expect("reply");
        let err = into_data(response).unwrap_err();
        assert_eq!(err.exit_code(), crate::error::EXIT_OTHER);
        assert!(err.user_message().contains("starting up"), "{err}");
    }

    #[test]
    fn requests_serialise_to_the_documented_envelope() {
        let request = Request {
            id: 7,
            protocol_version: PROTOCOL_VERSION,
            method: Method::List {
                limit: 50,
                cursor: None,
            },
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains(r#""method":"list""#), "{json}");
        assert!(
            json.contains(r#""params":{"limit":50,"cursor":null}"#),
            "{json}"
        );
        assert!(!json.contains('\n'), "a request must be one line: {json}");
    }

    #[test]
    fn unit_methods_serialise_without_params() {
        let json = serde_json::to_string(&Request {
            id: 1,
            protocol_version: PROTOCOL_VERSION,
            method: Method::Status,
        })
        .unwrap();
        assert!(json.contains(r#""method":"status""#), "{json}");
    }

    #[test]
    fn pin_carries_the_desired_state() {
        let json = serde_json::to_string(&Method::Pin {
            id: "abc".into(),
            pinned: false,
        })
        .unwrap();
        assert!(json.contains(r#""pinned":false"#), "{json}");
    }

    #[test]
    fn item_pages_deserialise_into_typed_items_and_a_skip_count() {
        let line = r#"{"id":1,"ok":true,"data":{"page":{"items":[{"id":"a","content":"hi",
            "content_type":"text/plain","created_at":5,"pinned":true,"is_sensitive":false}],
            "skipped_undecryptable":3}}}"#;
        let response: Response = serde_json::from_str(line).unwrap();
        let page = expect_page(into_data(response).unwrap()).unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].content, "hi");
        assert!(page.items[0].pinned);
        assert_eq!(page.skipped_undecryptable, 3, "the skip count was dropped");
        assert_eq!(
            page.next_cursor, None,
            "a page with no marker ends the list"
        );
    }

    /// The marker is what `list --cursor` is given back, so it has to survive
    /// the decode. A page that carries one and loses it here would end the walk
    /// early and hide the rest of the history.
    #[test]
    fn a_page_marker_survives_the_decode() {
        let line = r#"{"id":1,"ok":true,"data":{"page":{"items":[],
            "skipped_undecryptable":0,"next_cursor":"7b22"}}}"#;
        let response: Response = serde_json::from_str(line).unwrap();
        let page = expect_page(into_data(response).unwrap()).unwrap();
        assert_eq!(page.next_cursor.as_deref(), Some("7b22"));
    }

    #[test]
    fn an_empty_page_is_not_a_shape_error() {
        let response: Response = serde_json::from_str(
            r#"{"id":1,"ok":true,"data":{"page":{"items":[],"skipped_undecryptable":0}}}"#,
        )
        .unwrap();
        assert!(expect_page(into_data(response).unwrap())
            .unwrap()
            .items
            .is_empty());
    }

    /// The export wrapper keeps its item array distinct from a history page.
    #[test]
    fn an_export_does_not_decode_as_a_page() {
        let line = r#"{"id":1,"ok":true,"data":{"export":{"items":[],"skipped_non_text":1,
            "skipped_sensitive":2,"skipped_undecryptable":3}}}"#;
        let response: Response = serde_json::from_str(line).unwrap();
        let export = expect_export(into_data(response).unwrap()).unwrap();
        assert_eq!(
            (
                export.skipped_non_text,
                export.skipped_sensitive,
                export.skipped_undecryptable
            ),
            (1, 2, 3)
        );
    }

    #[test]
    fn discovered_devices_read_back_as_devices() {
        let line = r#"{"id":1,"ok":true,"data":{"discovered":{"devices":[{"discovery_id":"abc","name":"phone",
            "addr":"192.168.1.9:47654","last_seen_ms":5,"paired":false}]}}}"#;
        let response: Response = serde_json::from_str(line).unwrap();
        let found = expect_discovered(into_data(response).unwrap()).unwrap();
        assert_eq!(found.devices.len(), 1);
        assert!(!found.devices[0].paired);
    }

    #[test]
    fn settings_read_back_with_their_restart_list() {
        let line = r#"{"id":1,"ok":true,"data":{"config":{"config":{"poll_interval_ms":250,
            "history_limit":10000,"retention_days":0,"dedup_window_secs":60,
            "storage_quota_bytes":10737418240,
            "max_text_size_bytes":10485760,"max_image_size_bytes":67108864,
            "max_file_size_bytes":104857600,"max_decoded_image_mb":50,
            "sensitive_ttl_secs":0,"excluded_app_bundle_ids":[],
            "lan_visibility":true,"sync_enabled":true},"restart_required":["lan_visibility"]}}}"#;
        let response: Response = serde_json::from_str(line).unwrap();
        let applied = expect_config(into_data(response).unwrap()).unwrap();
        assert_eq!(applied.config.poll_interval_ms, 250);
        assert_eq!(applied.config.storage_quota_bytes, 10 * 1024 * 1024 * 1024);
        assert_eq!(applied.config.max_text_size_bytes, 10 * 1024 * 1024);
        assert_eq!(applied.config.max_image_size_bytes, 64 * 1024 * 1024);
        assert_eq!(applied.config.max_file_size_bytes, 100 * 1024 * 1024);
        assert_eq!(applied.config.max_decoded_image_mb, 50);
        assert_eq!(applied.restart_required, ["lan_visibility"]);
    }

    #[test]
    fn status_deserialises_into_typed_status() {
        let line = r#"{"id":1,"ok":true,"data":{"status":{"version":"2.0.0","protocol_version":1,
            "item_count":3,"capture_running":true,"clipboard_backend":"macos",
            "private_mode_epoch":0}}}"#;
        let response: Response = serde_json::from_str(line).unwrap();
        let status = expect_status(into_data(response).unwrap()).unwrap();
        assert_eq!(status.item_count, 3);
        assert_eq!(status.clipboard_backend, "macos");
    }

    #[test]
    fn a_count_payload_reads_back_as_a_count() {
        let response: Response =
            serde_json::from_str(r#"{"id":1,"ok":true,"data":{"count":9}}"#).unwrap();
        let data = into_data(response).unwrap();
        assert_eq!(optional_count(&data), Some(9));
    }

    #[test]
    fn an_empty_payload_carries_neither_item_nor_count() {
        let response: Response =
            serde_json::from_str(r#"{"id":1,"ok":true,"data":{"empty":{}}}"#).unwrap();
        let data = into_data(response).unwrap();
        assert!(optional_item(&data).is_none());
        assert!(optional_count(&data).is_none());
    }

    #[test]
    fn a_failure_response_becomes_a_typed_daemon_error() {
        let line = r#"{"id":1,"ok":false,"error":"no such item","error_code":"not_found"}"#;
        let response: Response = serde_json::from_str(line).unwrap();
        let err = into_data(response).unwrap_err();
        assert_eq!(err.exit_code(), crate::error::EXIT_NOT_FOUND);
    }

    #[test]
    fn a_failure_without_an_error_code_is_rejected() {
        let line = r#"{"id":1,"ok":false,"error":"unknown method"}"#;
        assert!(serde_json::from_str::<Response>(line).is_err());
    }

    #[test]
    fn a_future_error_code_is_kept_and_reported() {
        let line = r#"{"id":1,"ok":false,"error":"new refusal",
            "error_code":"future_daemon_state"}"#;
        let response: Response = serde_json::from_str(line).unwrap();
        assert_eq!(
            response.raw_error_code.as_deref(),
            Some("future_daemon_state")
        );
        let err = into_data(response).unwrap_err();
        assert_eq!(err.exit_code(), crate::error::EXIT_OTHER);
        assert!(err.user_message().contains("future_daemon_state"), "{err}");
    }

    #[test]
    fn a_wrong_shape_is_reported_rather_than_silently_defaulted() {
        // `unwrap_or_default` would silently produce an empty list.
        let response: Response =
            serde_json::from_str(r#"{"id":1,"ok":true,"data":{"empty":{}}}"#).unwrap();
        assert!(expect_page(into_data(response).unwrap()).is_err());
    }

    #[test]
    fn a_pairing_payload_reads_back_as_a_pairing() {
        let line = r#"{"id":1,"ok":true,"data":{"pairing_invite":{"code":"ABCD-EFGH",
            "pairing_id":"0123456789abcdef","listen_addr":"192.168.1.24:47654",
            "expires_in_secs":120}}}"#;
        let response: Response = serde_json::from_str(line).unwrap();
        let pairing = expect_pairing(into_data(response).unwrap()).unwrap();
        assert_eq!(pairing.code, "ABCD-EFGH");
        assert_eq!(pairing.listen_addr.as_deref(), Some("192.168.1.24:47654"));
    }

    #[test]
    fn a_peer_list_reads_back_as_peers_and_not_as_items() {
        let line = r#"{"id":1,"ok":true,"data":{"peers":[{"pairing_id":"abc","name":"phone",
            "last_addr":null,"last_seen_ms":0,"online":true,"details":{
                "profile":null,"endpoint":null,"latency":null,
                "public_ip":{"availability":"unavailable"},"geo":{"availability":"unavailable"},
                "presence":{"state":"online","last_seen_ms":0,"provenance":"observed",
                    "trust":"local","observed_at_ms":0,"fresh_until_ms":9223372036854775807}
            }}]}}"#;
        let response: Response = serde_json::from_str(line).unwrap();
        let peers = expect_peers(into_data(response).unwrap()).unwrap();
        assert_eq!(peers.len(), 1);
        assert!(peers[0].online);
        assert!(peers[0].last_addr.is_none());
    }

    #[test]
    fn sync_results_read_back_as_sync_results() {
        let line = r#"{"id":1,"ok":true,"data":{"sync":[{"pairing_id":"abc","name":"phone",
            "sent":2,"received":3,"error":null}]}}"#;
        let response: Response = serde_json::from_str(line).unwrap();
        let results = expect_sync(into_data(response).unwrap()).unwrap();
        assert_eq!(results[0].sent, 2);
        assert_eq!(results[0].received, 3);
        assert!(results[0].error.is_none());
    }

    /// Empty arrays retain the response variant named by their wrapper.
    #[test]
    fn empty_peer_and_sync_lists_stay_distinct() {
        let response: Response =
            serde_json::from_str(r#"{"id":1,"ok":true,"data":{"peers":[]}}"#).unwrap();
        let data = into_data(response).unwrap();
        assert!(expect_peers(data.clone()).unwrap().is_empty());
        assert!(expect_sync(data).is_err());

        let response: Response =
            serde_json::from_str(r#"{"id":1,"ok":true,"data":{"sync":[]}}"#).unwrap();
        let data = into_data(response).unwrap();
        assert!(expect_sync(data.clone()).unwrap().is_empty());
        assert!(expect_peers(data).is_err());
    }

    #[test]
    fn a_wrongly_shaped_peer_reply_is_still_reported() {
        let response: Response =
            serde_json::from_str(r#"{"id":1,"ok":true,"data":{"count":9}}"#).unwrap();
        assert!(expect_peers(into_data(response).unwrap()).is_err());
    }

    #[test]
    fn peer_methods_serialise_to_the_documented_envelope() {
        let json = serde_json::to_string(&Method::PairJoin {
            code: "ABCD".into(),
            addr: "127.0.0.1:47654".into(),
        })
        .unwrap();
        assert!(json.contains(r#""method":"pair_join""#), "{json}");
        assert!(json.contains(r#""code":"ABCD""#), "{json}");

        let json = serde_json::to_string(&Method::SyncNow { pairing_id: None }).unwrap();
        assert!(json.contains(r#""method":"sync_now""#), "{json}");
    }

    #[test]
    fn protocol_mismatch_maps_to_the_version_message() {
        let line = r#"{"id":1,"ok":false,"error":"unsupported protocol version 4",
            "error_code":"protocol_mismatch"}"#;
        let response: Response = serde_json::from_str(line).unwrap();
        let err = into_data(response).unwrap_err();
        assert!(err.user_message().contains("versions differ"));
        assert_eq!(err.exit_code(), crate::error::EXIT_OTHER);
    }
}

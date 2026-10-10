use super::protocol::{read_frame, write_frame, InferenceReply, InferenceRequest};
use crate::ModuleError;
use copypaste_module_sdk::ModuleOutput;
use std::{
    io::{Read, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc, Condvar, Mutex,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

type ProcessTermination = Arc<dyn Fn() + Send + Sync>;
type ProcessControl = Option<(String, ProcessTermination)>;

pub(crate) const IDLE_DELAY: Duration = Duration::from_secs(60);
const REQUEST_LIMIT: Duration = Duration::from_secs(60);

/// Platform adapters create one private channel and own process termination.
pub trait InferenceLauncher: Send + Sync {
    fn launch(&self) -> Result<InferenceConnection, ModuleError>;
}

pub struct DesktopInferenceLauncher {
    executable: PathBuf,
}

impl DesktopInferenceLauncher {
    pub fn new(executable: PathBuf) -> Self {
        Self { executable }
    }
}

impl InferenceLauncher for DesktopInferenceLauncher {
    fn launch(&self) -> Result<InferenceConnection, ModuleError> {
        let mut command = Command::new(&self.executable);
        command
            .arg("--inference-worker")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        let mut child = command.spawn()?;
        let reader = child.stdout.take().ok_or(ModuleError::Load)?;
        let writer = child.stdin.take().ok_or(ModuleError::Load)?;
        let child = Arc::new(Mutex::new(Some(child)));
        let terminate = Arc::new(move || {
            if let Some(mut child) = child
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .take()
            {
                reap(&mut child);
            }
        });
        InferenceConnection::new(reader, writer, terminate)
    }
}

fn reap(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// The receiver thread has no model state and ends when termination closes IPC.
pub struct InferenceConnection {
    writer: Box<dyn Write + Send>,
    replies: mpsc::Receiver<Result<InferenceReply, ModuleError>>,
    terminate: Arc<dyn Fn() + Send + Sync>,
    receiver: Option<JoinHandle<()>>,
}

impl InferenceConnection {
    pub fn new(
        reader: impl Read + Send + 'static,
        writer: impl Write + Send + 'static,
        terminate: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Self, ModuleError> {
        let (send, replies) = mpsc::sync_channel(1);
        let receiver = std::thread::Builder::new()
            .name("inference-replies".into())
            .spawn(move || {
                let mut reader = reader;
                loop {
                    let reply = read_frame::<InferenceReply>(&mut reader)
                        .and_then(|reply| reply.ok_or(ModuleError::Load));
                    let failed = reply.is_err();
                    if send.send(reply).is_err() || failed {
                        break;
                    }
                }
            });
        let receiver = match receiver {
            Ok(receiver) => receiver,
            Err(error) => {
                terminate();
                return Err(error.into());
            }
        };
        Ok(Self {
            writer: Box::new(writer),
            replies,
            terminate,
            receiver: Some(receiver),
        })
    }

    fn invoke(&mut self, request: &InferenceRequest) -> Result<ModuleOutput, ModuleError> {
        write_frame(&mut self.writer, request)?;
        self.replies
            .recv_timeout(REQUEST_LIMIT)
            .map_err(|_| ModuleError::Load)??
            .map_err(ModuleError::Invalid)
    }
}

impl Drop for InferenceConnection {
    fn drop(&mut self) {
        (self.terminate)();
        // A terminated process cannot retain the read side. Drain a queued
        // reply before joining so an EOF publication cannot block behind it.
        let (_, empty) = mpsc::channel();
        drop(std::mem::replace(&mut self.replies, empty));
        if let Some(receiver) = self.receiver.take() {
            let _ = receiver.join();
        }
    }
}

struct Active {
    key: Vec<u8>,
    id: String,
    connection: InferenceConnection,
    finished: Instant,
}

struct ClientState {
    launcher: Mutex<Arc<dyn InferenceLauncher>>,
    active: Mutex<Option<Active>>,
    control: Mutex<ProcessControl>,
    generation: AtomicU64,
    stopped: AtomicBool,
    wake: Condvar,
}

pub(crate) struct InferenceClient {
    state: Arc<ClientState>,
    reaper: Option<JoinHandle<()>>,
}

impl InferenceClient {
    pub fn new() -> Self {
        let state = Arc::new(ClientState {
            launcher: Mutex::new(default_launcher()),
            active: Mutex::new(None),
            control: Mutex::new(None),
            generation: AtomicU64::new(0),
            stopped: AtomicBool::new(false),
            wake: Condvar::new(),
        });
        let owner = Arc::clone(&state);
        let reaper = std::thread::Builder::new()
            .name("inference-idle".into())
            .spawn(move || reap_idle(owner))
            .ok();
        Self { state, reaper }
    }

    pub fn set_launcher(&self, launcher: Arc<dyn InferenceLauncher>) {
        self.stop(None);
        *self
            .state
            .launcher
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = launcher;
    }

    pub fn generation(&self) -> u64 {
        self.state.generation.load(Ordering::Acquire)
    }

    pub fn invoke(
        &self,
        request: InferenceRequest,
        generation: u64,
    ) -> Result<ModuleOutput, ModuleError> {
        let key = serde_json::to_vec(&(
            &request.package_dir,
            &request.data_dir,
            &request.id,
            &request.invocation.command,
            &request.invocation.preferences,
        ))
        .map_err(|_| ModuleError::State)?;
        if self.reaper.is_none() {
            return Err(ModuleError::Load);
        }
        let mut active = self.state.active.lock().map_err(|_| ModuleError::State)?;
        if generation != self.generation() {
            return Err(ModuleError::Disabled);
        }
        if active.as_ref().is_some_and(|active| active.key != key) {
            active.take();
        }
        if active.is_none() {
            let mut control = self.state.control.lock().map_err(|_| ModuleError::State)?;
            let connection = self
                .state
                .launcher
                .lock()
                .map_err(|_| ModuleError::State)?
                .launch()?;
            if generation != self.generation() {
                return Err(ModuleError::Disabled);
            }
            *control = Some((request.id.clone(), Arc::clone(&connection.terminate)));
            *active = Some(Active {
                key,
                id: request.id.clone(),
                connection,
                finished: Instant::now(),
            });
        }
        let worker = active.as_mut().ok_or(ModuleError::Load)?;
        let result = worker.connection.invoke(&request);
        worker.finished = Instant::now();
        self.state.wake.notify_all();
        if result.is_err() {
            active.take();
            self.state
                .control
                .lock()
                .map_err(|_| ModuleError::State)?
                .take();
        }
        result
    }

    pub fn stop(&self, id: Option<&str>) {
        // Mutations of unrelated modules must not wait behind inference.
        if self
            .state
            .control
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
            .is_some_and(|(active, _)| id.is_some_and(|id| id != active))
        {
            return;
        }
        self.state.generation.fetch_add(1, Ordering::AcqRel);
        let control = {
            let mut control = self
                .state
                .control
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if control
                .as_ref()
                .is_some_and(|(active, _)| id.is_none_or(|id| id == active))
            {
                control.take()
            } else {
                None
            }
        };
        if let Some((_, terminate)) = control {
            terminate();
        }
        let mut active = self
            .state
            .active
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if active
            .as_ref()
            .is_some_and(|active| id.is_none_or(|id| id == active.id))
        {
            active.take();
        }
    }
}

#[cfg(not(target_os = "android"))]
fn default_launcher() -> Arc<dyn InferenceLauncher> {
    Arc::new(DesktopInferenceLauncher::new(
        std::env::current_exe().unwrap_or_default(),
    ))
}

#[cfg(target_os = "android")]
fn default_launcher() -> Arc<dyn InferenceLauncher> {
    struct Unconfigured;
    impl InferenceLauncher for Unconfigured {
        fn launch(&self) -> Result<InferenceConnection, ModuleError> {
            Err(ModuleError::Load)
        }
    }
    // Android must install its bound-service adapter before inference admission.
    Arc::new(Unconfigured)
}

impl Drop for InferenceClient {
    fn drop(&mut self) {
        self.state.stopped.store(true, Ordering::Release);
        self.stop(None);
        self.state.wake.notify_all();
        if let Some(reaper) = self.reaper.take() {
            let _ = reaper.join();
        }
    }
}

fn reap_idle(state: Arc<ClientState>) {
    let mut active = state
        .active
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    loop {
        if state.stopped.load(Ordering::Acquire) {
            return;
        }
        let remaining = active
            .as_ref()
            .map(|worker| IDLE_DELAY.saturating_sub(worker.finished.elapsed()));
        match remaining {
            Some(remaining) if remaining.is_zero() => {
                let retired = active.take();
                state
                    .control
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .take();
                // Keep startup serialized until the retired process is reaped.
                drop(retired);
            }
            Some(remaining) => {
                active = state
                    .wake
                    .wait_timeout(active, remaining)
                    .unwrap_or_else(|error| error.into_inner())
                    .0;
            }
            None => {
                active = state
                    .wake
                    .wait(active)
                    .unwrap_or_else(|error| error.into_inner());
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::inference::WorkerIdentity;
    use copypaste_module_sdk::{
        ModuleArchitecture, ModuleInvocation, ModulePlatform, ModuleTarget,
    };
    use std::{
        collections::BTreeMap,
        net::{TcpListener, TcpStream},
        sync::atomic::AtomicUsize,
    };

    struct FixtureLauncher {
        launched: Arc<AtomicUsize>,
        stopped: Arc<AtomicUsize>,
        entered: mpsc::Sender<()>,
    }

    impl InferenceLauncher for FixtureLauncher {
        fn launch(&self) -> Result<InferenceConnection, ModuleError> {
            self.launched.fetch_add(1, Ordering::SeqCst);
            let listener = TcpListener::bind("127.0.0.1:0")?;
            let client = TcpStream::connect(listener.local_addr()?)?;
            let (mut server, _) = listener.accept()?;
            let entered = self.entered.clone();
            let cancelled = Arc::new(AtomicBool::new(false));
            let gate = Arc::new((Mutex::new(()), Condvar::new()));
            let worker_cancelled = Arc::clone(&cancelled);
            let worker_gate = Arc::clone(&gate);
            std::thread::spawn(move || {
                while let Ok(Some(request)) = read_frame::<InferenceRequest>(&mut server) {
                    if request
                        .invocation
                        .arguments
                        .get("text")
                        .and_then(|value| value.as_str())
                        == Some("block")
                    {
                        let _ = entered.send(());
                        let lock = worker_gate.0.lock().unwrap();
                        drop(
                            worker_gate
                                .1
                                .wait_while(lock, |_| !worker_cancelled.load(Ordering::Acquire))
                                .unwrap(),
                        );
                        return;
                    }
                    let reply: InferenceReply = Ok(ModuleOutput::Embeddings {
                        model_id: "fixture".into(),
                        vectors: vec![vec![1.0, 0.0]],
                    });
                    if write_frame(&mut server, &reply).is_err() {
                        return;
                    }
                }
            });
            let control = client.try_clone()?;
            let stopped = Arc::clone(&self.stopped);
            let terminate = Arc::new(move || {
                if !cancelled.swap(true, Ordering::AcqRel) {
                    stopped.fetch_add(1, Ordering::SeqCst);
                    let _ = control.shutdown(std::net::Shutdown::Both);
                    gate.1.notify_all();
                }
            });
            InferenceConnection::new(client.try_clone()?, client, terminate)
        }
    }

    pub(crate) fn launcher_fixture() -> (
        Arc<dyn InferenceLauncher>,
        Arc<AtomicUsize>,
        Arc<AtomicUsize>,
        mpsc::Receiver<()>,
    ) {
        let launched = Arc::new(AtomicUsize::new(0));
        let stopped = Arc::new(AtomicUsize::new(0));
        let (entered, events) = mpsc::channel();
        let launcher = Arc::new(FixtureLauncher {
            launched: Arc::clone(&launched),
            stopped: Arc::clone(&stopped),
            entered,
        });
        (launcher, launched, stopped, events)
    }

    fn fixture() -> (
        Arc<InferenceClient>,
        Arc<AtomicUsize>,
        Arc<AtomicUsize>,
        mpsc::Receiver<()>,
    ) {
        let (launcher, launched, stopped, events) = launcher_fixture();
        let client = Arc::new(InferenceClient::new());
        client.set_launcher(launcher);
        (client, launched, stopped, events)
    }

    fn request(text: &str) -> InferenceRequest {
        InferenceRequest {
            identity: WorkerIdentity {
                app_version: "1.0.23".into(),
                target: ModuleTarget {
                    platform: ModulePlatform::Macos,
                    architecture: ModuleArchitecture::Aarch64,
                },
                public_key: String::new(),
            },
            id: "fixture".into(),
            package_dir: "package".into(),
            data_dir: "data".into(),
            invocation: ModuleInvocation {
                command: "embed".into(),
                arguments: BTreeMap::from([("text".into(), text.into())]),
                preferences: BTreeMap::new(),
            },
        }
    }

    #[test]
    fn idle_deadline_releases_worker_and_next_request_reloads_without_losing_results() {
        let (client, launched, stopped, _) = fixture();
        let expected = client
            .invoke(request("first"), client.generation())
            .unwrap();
        assert_eq!(
            client
                .invoke(request("second"), client.generation())
                .unwrap(),
            expected
        );
        assert_eq!(launched.load(Ordering::SeqCst), 1);
        assert_eq!(stopped.load(Ordering::SeqCst), 0);
        // Move the completed-work clock to the deadline; exercise the actual
        // reaper without making every portable test wait a minute.
        client
            .state
            .active
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .finished = Instant::now() - IDLE_DELAY;
        client.state.wake.notify_all();
        let started = Instant::now();
        while stopped.load(Ordering::SeqCst) == 0 {
            assert!(started.elapsed() < Duration::from_secs(2));
            std::thread::yield_now();
        }
        assert_eq!(
            client
                .invoke(request("third"), client.generation())
                .unwrap(),
            expected
        );
        assert_eq!(launched.load(Ordering::SeqCst), 2);
        drop(client);
        assert_eq!(stopped.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn cancellation_interrupts_active_inference_and_rejects_pre_disable_admission() {
        let (client, launched, stopped, entered) = fixture();
        let generation = client.generation();
        let caller = Arc::clone(&client);
        let pending = std::thread::spawn(move || caller.invoke(request("block"), generation));
        entered.recv_timeout(Duration::from_secs(2)).unwrap();
        let started = Instant::now();
        client.stop(Some("fixture"));
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(pending.join().unwrap().is_err());
        assert_eq!(stopped.load(Ordering::SeqCst), 1);
        assert!(matches!(
            client.invoke(request("stale"), generation),
            Err(ModuleError::Disabled)
        ));
        assert_eq!(launched.load(Ordering::SeqCst), 1);
        assert!(client.invoke(request("fresh"), client.generation()).is_ok());
    }

    #[test]
    fn changed_model_restarts_worker_before_loading_another_context() {
        let (client, launched, stopped, _) = fixture();
        client
            .invoke(request("first"), client.generation())
            .unwrap();
        let mut changed = request("second");
        changed
            .invocation
            .preferences
            .insert("languages".into(), serde_json::json!(["uk"]));
        client.invoke(changed, client.generation()).unwrap();
        assert_eq!(launched.load(Ordering::SeqCst), 2);
        assert_eq!(stopped.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn unrelated_module_mutation_does_not_wait_for_or_cancel_inference() {
        let (client, _, stopped, entered) = fixture();
        let caller = Arc::clone(&client);
        let pending =
            std::thread::spawn(move || caller.invoke(request("block"), caller.generation()));
        entered.recv_timeout(Duration::from_secs(2)).unwrap();
        let started = Instant::now();
        client.stop(Some("another-module"));
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(stopped.load(Ordering::SeqCst), 0);
        client.stop(Some("fixture"));
        assert!(pending.join().unwrap().is_err());
    }

    #[test]
    fn unexpected_worker_exit_is_reported_and_later_work_can_start_a_fresh_process() {
        let (client, launched, stopped, _) = fixture();
        client
            .invoke(request("first"), client.generation())
            .unwrap();
        let terminate = Arc::clone(&client.state.control.lock().unwrap().as_ref().unwrap().1);
        terminate();
        assert!(client
            .invoke(request("after-crash"), client.generation())
            .is_err());
        assert!(client.invoke(request("fresh"), client.generation()).is_ok());
        assert_eq!(launched.load(Ordering::SeqCst), 2);
        assert_eq!(stopped.load(Ordering::SeqCst), 1);
    }
}

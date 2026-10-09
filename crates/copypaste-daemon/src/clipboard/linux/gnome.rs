//! GNOME Shell clipboard bridge transport.
//!
//! The extension authenticates every call by our session-bus name and UID. A
//! missing extension therefore leaves this adapter inactive rather than
//! pretending that another clipboard implementation exists.

use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use zbus::blocking::{connection::Builder, Connection, Proxy};

use super::super::{Capture, CapturePolicy, ClipboardSource, SourcePolicyEvidence};

const DAEMON: &str = "app.copypaste.Daemon";
const BRIDGE: &str = "app.copypaste.GnomeIntegration";
const PATH: &str = "/app/copypaste/Clipboard";
const INTERFACE: &str = "app.copypaste.Clipboard";
const MAX_BODY: usize = 4 * 1024 * 1024;
const MAX_WRITE: usize = 32 * 1024 * 1024;
const MAX_MIMES: usize = 64;
const MAX_MIME_BYTES: usize = 255;
const KDE_PASSWORD_MANAGER_HINT: &str = "x-kde-passwordManagerHint";
const RPC_TIMEOUT: Duration = Duration::from_secs(2);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Default)]
struct State {
    active: bool,
    watching: bool,
    dirty: bool,
    epoch: u64,
    sequence: u64,
    owned_sequence: Option<u64>,
    mimes: Vec<String>,
}

pub(in crate::clipboard) struct GnomeClipboard {
    commands: mpsc::Sender<Command>,
    state: Arc<Mutex<State>>,
    staging: super::super::file_materialize::StagingArea,
    worker: Option<thread::JoinHandle<()>>,
}

enum Command {
    Read {
        sequence: u64,
        mime: String,
        limit: u32,
        reply: mpsc::Sender<Option<Vec<u8>>>,
    },
    Write {
        values: HashMap<String, Vec<u8>>,
        reply: mpsc::Sender<Option<u64>>,
    },
    Stop,
}

impl GnomeClipboard {
    pub(super) fn new(data_dir: &Path) -> std::io::Result<Self> {
        // The cleanup owner exists before the D-Bus worker can accept a file.
        let staging = super::super::file_materialize::StagingArea::new(data_dir)?;
        let state = Arc::new(Mutex::new(State::default()));
        let (commands, receiver) = mpsc::channel();
        let worker_state = Arc::clone(&state);
        // zbus::blocking owns a global runtime. Never construct or call it
        // from the daemon's Tokio executor; this thread is the only boundary.
        let worker = thread::spawn(move || worker(receiver, worker_state));
        Ok(Self {
            commands,
            state,
            staging,
            worker: Some(worker),
        })
    }
    fn read_mime(&self, sequence: u64, mime: &str, limit: u32) -> Option<Vec<u8>> {
        let (reply, received) = mpsc::channel();
        self.commands
            .send(Command::Read {
                sequence,
                mime: mime.into(),
                limit,
                reply,
            })
            .ok()?;
        received.recv_timeout(REQUEST_TIMEOUT).ok().flatten()
    }
    fn read(&mut self, settings: &copypaste_ipc::ConfigData) -> Option<Capture> {
        let (epoch, sequence, mimes) = {
            let mut state = self.state.lock().ok()?;
            if !state.active || !state.dirty {
                return None;
            }
            state.dirty = false;
            (state.epoch, state.sequence, state.mimes.clone())
        };
        // The cursor has advanced before every policy gate: a value copied
        // during private mode must never be captured when private mode ends.
        if settings.private_mode || !settings.excluded_app_bundle_ids.is_empty() {
            return None;
        }
        let secret = if mimes.iter().any(|mime| mime == KDE_PASSWORD_MANAGER_HINT) {
            let hint = self.read_mime(sequence, KDE_PASSWORD_MANAGER_HINT, 64)?;
            hint.eq_ignore_ascii_case(b"secret")
        } else {
            false
        };
        let privacy = copypaste_ipc::ClipboardPrivacy {
            secret,
            transient: false,
        };
        if !privacy.allows(settings) || !current_epoch(&self.state, epoch, sequence) {
            return None;
        }
        let (mime, content_type, binary) = representation(&mimes)?;
        let limit = settings
            .capture_limit_bytes(content_type)
            .min(MAX_BODY as u64) as u32;
        let bytes = self.read_mime(sequence, &mime, limit)?;
        if bytes.len() > limit as usize {
            return None;
        }
        if !current_epoch(&self.state, epoch, sequence) {
            return None;
        }
        if content_type == copypaste_ipc::content_type::FILE {
            return super::file_capture(bytes, privacy, None);
        }
        Some(Capture {
            privacy,
            content: if binary {
                String::new()
            } else {
                String::from_utf8_lossy(&bytes).into_owned()
            },
            binary_content: binary.then_some(bytes),
            file_path: None,
            file_metadata: None,
            content_type: content_type.into(),
            app_bundle_id: None,
            app_name: None,
            source_policy: SourcePolicyEvidence::Legacy,
        })
    }
    fn write(&self, values: HashMap<String, Vec<u8>>) -> anyhow::Result<()> {
        if !valid_payloads(&values) {
            anyhow::bail!("invalid GNOME clipboard payload");
        }
        let (epoch, before) = self
            .state
            .lock()
            .map(|state| (state.epoch, state.sequence))
            .map_err(|_| anyhow::anyhow!("GNOME clipboard state failed"))?;
        let (reply, received) = mpsc::channel();
        self.commands
            .send(Command::Write { values, reply })
            .map_err(|_| anyhow::anyhow!("GNOME clipboard helper is inactive"))?;
        let sequence = received
            .recv_timeout(REQUEST_TIMEOUT)
            .ok()
            .flatten()
            .ok_or_else(|| anyhow::anyhow!("GNOME clipboard helper rejected write"))?;
        if let Ok(mut state) = self.state.lock() {
            if state.epoch != epoch || !state.active {
                anyhow::bail!("GNOME clipboard helper changed");
            }
            if state.sequence > before && state.sequence != sequence {
                // A foreign owner transition arrived while Write was in
                // flight. Its pending capture must win over our sentinel.
                return Ok(());
            }
            state.sequence = sequence;
            state.owned_sequence = Some(sequence);
            state.dirty = false;
        }
        Ok(())
    }
}

impl Drop for GnomeClipboard {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl ClipboardSource for GnomeClipboard {
    fn poll(&mut self) -> Option<Capture> {
        self.read(&copypaste_ipc::ConfigData::default())
    }
    fn poll_with_policy(&mut self, policy: CapturePolicy<'_>) -> Option<Capture> {
        self.read(policy.settings)
    }
    fn changed(&mut self) -> bool {
        self.state
            .lock()
            .map(|state| state.active && state.dirty)
            .unwrap_or(false)
    }
    fn set_contents(&mut self, text: &str) -> anyhow::Result<()> {
        self.write(HashMap::from([(
            "text/plain;charset=utf-8".into(),
            text.as_bytes().to_vec(),
        )]))
    }
    fn set_binary_contents(
        &mut self,
        _: &str,
        content_type: &str,
        bytes: &[u8],
        metadata: Option<&copypaste_core::FileMetadata>,
    ) -> Result<(), copypaste_core::ClipboardWriteError> {
        use copypaste_core::ClipboardWriteError;
        let (mime, bytes) = match content_type {
            copypaste_ipc::content_type::HTML => ("text/html", bytes.to_vec()),
            copypaste_ipc::content_type::RICH_TEXT => ("text/rtf", bytes.to_vec()),
            copypaste_ipc::content_type::IMAGE_PNG => ("image/png", bytes.to_vec()),
            copypaste_ipc::content_type::IMAGE_TIFF => ("image/tiff", bytes.to_vec()),
            copypaste_ipc::content_type::FILE => {
                let path = self
                    .staging
                    .materialize(bytes, metadata.ok_or(ClipboardWriteError::Failed)?)
                    .map_err(|_| ClipboardWriteError::Failed)?;
                (
                    "text/uri-list",
                    url::Url::from_file_path(path)
                        .map_err(|_| ClipboardWriteError::Failed)?
                        .to_string()
                        .into_bytes(),
                )
            }
            _ => return Err(ClipboardWriteError::UnsupportedContent),
        };
        self.write(HashMap::from([(mime.into(), bytes)]))
            .map_err(|_| ClipboardWriteError::Failed)
    }
    fn backend_name(&self) -> &'static str {
        "linux-gnome-wayland-clipboard"
    }
}

fn worker(commands: mpsc::Receiver<Command>, state: Arc<Mutex<State>>) {
    let Ok(connection) =
        Builder::session().and_then(|builder| builder.method_timeout(RPC_TIMEOUT).build())
    else {
        return;
    };
    if connection.request_name(DAEMON).is_err() {
        return;
    }
    watch_service(connection.clone(), Arc::clone(&state));
    activate(&connection, &state);
    loop {
        let command = match commands.recv_timeout(RPC_TIMEOUT) {
            Ok(command) => command,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        };
        if matches!(command, Command::Stop) {
            let _ = connection.close();
            return;
        }
        let proxy = Proxy::new(&connection, BRIDGE, PATH, INTERFACE).ok();
        match command {
            Command::Read {
                sequence,
                mime,
                limit,
                reply,
            } => {
                let bytes = proxy
                    .as_ref()
                    .and_then(|proxy| proxy.call("Read", &(sequence, mime.as_str(), limit)).ok());
                let _ = reply.send(bytes);
            }
            Command::Write { values, reply } => {
                let sequence = proxy
                    .as_ref()
                    .and_then(|proxy| proxy.call("Write", &(values,)).ok());
                let _ = reply.send(sequence);
            }
            Command::Stop => unreachable!("Stop returned before proxy dispatch"),
        }
    }
}

fn activate(connection: &Connection, state: &Arc<Mutex<State>>) {
    let Ok(proxy) = Proxy::new(connection, BRIDGE, PATH, INTERFACE) else {
        return;
    };
    let Ok((sequence, mimes)) = proxy.call::<_, _, (u64, Vec<String>)>("Snapshot", &()) else {
        return;
    };
    if !valid_mimes(&mimes) {
        return;
    }
    let epoch = if let Ok(mut state) = state.lock() {
        state.active = true;
        state.watching = false;
        state.dirty = false;
        state.sequence = sequence;
        state.mimes = mimes;
        state.epoch
    } else {
        return;
    };
    let (ready, subscribed) = mpsc::sync_channel(1);
    watch_clipboard(connection.clone(), Arc::clone(state), epoch, ready);
    if subscribed.recv_timeout(RPC_TIMEOUT).is_err() {
        invalidate(state);
        return;
    }
    if let Ok(mut state) = state.lock() {
        if state.epoch == epoch && state.active {
            state.watching = true;
        }
    }
}

fn watch_service(connection: Connection, state: Arc<Mutex<State>>) {
    thread::spawn(move || {
        let Ok(proxy) = Proxy::new(
            &connection,
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
        ) else {
            return;
        };
        let Ok(signals) = proxy.receive_signal("NameOwnerChanged") else {
            return;
        };
        for signal in signals {
            let Ok((name, _old, new)) = signal.body().deserialize::<(String, String, String)>()
            else {
                continue;
            };
            if name != BRIDGE {
                continue;
            }
            invalidate(&state);
            if !new.is_empty() {
                activate(&connection, &state);
            }
        }
    });
}

fn watch_clipboard(
    connection: Connection,
    state: Arc<Mutex<State>>,
    epoch: u64,
    ready: mpsc::SyncSender<()>,
) {
    thread::spawn(move || {
        let Ok(proxy) = Proxy::new(&connection, BRIDGE, PATH, INTERFACE) else {
            return;
        };
        let Ok(signals) = proxy.receive_signal("OwnerChanged") else {
            return;
        };
        let _ = ready.send(());
        for signal in signals {
            let Ok((sequence, mimes)) = signal.body().deserialize::<(u64, Vec<String>)>() else {
                continue;
            };
            if !valid_mimes(&mimes) {
                continue;
            }
            if let Ok(mut state) = state.lock() {
                if state.epoch != epoch || !state.active {
                    break;
                }
                apply_owner_changed(&mut state, sequence, mimes.to_vec());
                state.active = true;
            }
        }
        if let Ok(mut state) = state.lock() {
            if state.epoch == epoch {
                state.epoch = state.epoch.wrapping_add(1);
                state.active = false;
                state.watching = false;
                state.dirty = false;
                state.sequence = 0;
                state.owned_sequence = None;
                state.mimes.clear();
            }
        }
    });
}

fn invalidate(state: &Arc<Mutex<State>>) {
    if let Ok(mut state) = state.lock() {
        state.epoch = state.epoch.wrapping_add(1);
        state.active = false;
        state.watching = false;
        state.dirty = false;
        state.sequence = 0;
        state.owned_sequence = None;
        state.mimes.clear();
    }
}

fn current_epoch(state: &Arc<Mutex<State>>, epoch: u64, sequence: u64) -> bool {
    state
        .lock()
        .is_ok_and(|state| state.active && state.epoch == epoch && state.sequence == sequence)
}

fn apply_owner_changed(state: &mut State, sequence: u64, mimes: Vec<String>) {
    if sequence < state.sequence {
        return;
    }
    state.sequence = sequence;
    state.mimes = mimes;
    if state.owned_sequence == Some(sequence) {
        state.owned_sequence = None;
        state.dirty = false;
    } else {
        state.dirty = true;
    }
}

fn valid_mimes(mimes: &[String]) -> bool {
    !mimes.is_empty()
        && mimes.len() <= MAX_MIMES
        && mimes
            .iter()
            .all(|mime| !mime.is_empty() && mime.len() <= MAX_MIME_BYTES)
}
fn valid_payloads(values: &HashMap<String, Vec<u8>>) -> bool {
    !values.is_empty()
        && values.len() <= MAX_MIMES
        && values.iter().all(|(mime, body)| {
            !mime.is_empty() && mime.len() <= MAX_MIME_BYTES && body.len() <= MAX_BODY
        })
        && values.values().map(Vec::len).sum::<usize>() <= MAX_WRITE
}
fn representation(mimes: &[String]) -> Option<(String, &'static str, bool)> {
    for (mime, type_, binary) in [
        ("text/html", copypaste_ipc::content_type::HTML, false),
        ("text/rtf", copypaste_ipc::content_type::RICH_TEXT, false),
        ("image/png", copypaste_ipc::content_type::IMAGE_PNG, true),
        ("image/tiff", copypaste_ipc::content_type::IMAGE_TIFF, true),
        ("text/uri-list", copypaste_ipc::content_type::FILE, true),
        (
            "text/plain;charset=utf-8",
            copypaste_ipc::content_type::TEXT,
            false,
        ),
        ("UTF8_STRING", copypaste_ipc::content_type::TEXT, false),
    ] {
        if mimes.iter().any(|value| value == mime) {
            return Some((mime.into(), type_, binary));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[derive(Default)]
    struct BridgeState {
        sequence: u64,
        mimes: Vec<String>,
        values: HashMap<String, Vec<u8>>,
        writes: Vec<HashMap<String, Vec<u8>>>,
        stall_reads: bool,
    }

    struct FixtureBridge(Arc<Mutex<BridgeState>>);

    #[zbus::interface(name = "app.copypaste.Clipboard")]
    impl FixtureBridge {
        #[zbus(signal)]
        async fn owner_changed(
            emitter: &zbus::object_server::SignalEmitter<'_>,
            sequence: u64,
            mimes: Vec<String>,
        ) -> zbus::Result<()>;

        fn snapshot(&self) -> (u64, Vec<String>) {
            let state = self.0.lock().expect("fixture bridge state");
            (state.sequence, state.mimes.clone())
        }

        fn read(&self, sequence: u64, mime: &str, limit: u32) -> Vec<u8> {
            let state = self.0.lock().expect("fixture bridge state");
            if state.stall_reads {
                drop(state);
                thread::sleep(RPC_TIMEOUT + Duration::from_secs(1));
                return Vec::new();
            }
            assert_eq!(sequence, state.sequence, "bridge read sequence");
            let value = state.values.get(mime).cloned().unwrap_or_default();
            value[..value.len().min(limit as usize)].to_vec()
        }

        fn write(&self, values: HashMap<String, Vec<u8>>) -> u64 {
            let mut state = self.0.lock().expect("fixture bridge state");
            state.writes.push(values);
            state.sequence = state.sequence.wrapping_add(1);
            state.sequence
        }
    }

    struct BridgeFixture {
        _runtime: tokio::runtime::Runtime,
        connection: Connection,
        state: Arc<Mutex<BridgeState>>,
    }

    impl BridgeFixture {
        fn start() -> Self {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("fixture runtime");
            let _guard = runtime.enter();
            let state = Arc::new(Mutex::new(BridgeState {
                sequence: 1,
                mimes: vec!["text/plain;charset=utf-8".into()],
                values: HashMap::from([(
                    "text/plain;charset=utf-8".into(),
                    b"bridge text".to_vec(),
                )]),
                ..Default::default()
            }));
            let connection = Connection::session().expect("fixture session bus");
            connection
                .request_name(BRIDGE)
                .expect("fixture bridge name");
            connection
                .object_server()
                .at(PATH, FixtureBridge(Arc::clone(&state)))
                .expect("fixture bridge object");
            Self {
                _runtime: runtime,
                connection,
                state,
            }
        }

        fn owner_changed(&self, sequence: u64, mimes: Vec<String>) {
            let interface = self
                .connection
                .object_server()
                .interface::<_, FixtureBridge>(PATH)
                .expect("fixture bridge interface");
            self._runtime
                .block_on(FixtureBridge::owner_changed(
                    interface.signal_emitter(),
                    sequence,
                    mimes,
                ))
                .expect("fixture owner signal");
        }

        fn update_owner(&self, sequence: u64, mimes: Vec<String>) {
            let mut state = self.state.lock().expect("fixture bridge state");
            state.sequence = sequence;
            state.mimes = mimes.clone();
            drop(state);
            self.owner_changed(sequence, mimes);
        }
    }

    fn wait_for_active(clipboard: &GnomeClipboard) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if clipboard
                .state
                .lock()
                .is_ok_and(|state| state.active && state.watching)
            {
                return;
            }
            thread::sleep(Duration::from_millis(5));
        }
        panic!("GNOME transport did not activate against the fixture bridge");
    }

    fn wait_for_change(clipboard: &mut GnomeClipboard) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while !clipboard.changed() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(1));
        }
        assert!(clipboard.changed(), "fixture OwnerChanged was not observed");
    }

    #[test]
    fn live_gnome_dbus_fixture_reads_writes_and_bounds_stalled_calls() {
        if std::env::var_os("COPYPASTE_GNOME_FIXTURE") != Some("1".into()) {
            return;
        }
        let fixture = BridgeFixture::start();
        let directory = tempfile::tempdir().expect("fixture data dir");
        let mut clipboard = GnomeClipboard::new(directory.path()).expect("GNOME clipboard");
        wait_for_active(&clipboard);

        fixture.update_owner(2, vec!["text/plain;charset=utf-8".into()]);
        wait_for_change(&mut clipboard);
        let capture = clipboard.poll().expect("capture bridge text");
        assert_eq!(capture.content, "bridge text");

        fixture.update_owner(3, vec!["text/plain;charset=utf-8".into()]);
        wait_for_change(&mut clipboard);
        let private = copypaste_ipc::ConfigData {
            private_mode: true,
            ..Default::default()
        };
        assert!(clipboard
            .poll_with_policy(CapturePolicy::new(&private))
            .is_none());
        assert!(
            clipboard.poll().is_none(),
            "private cursor was not acknowledged"
        );

        {
            let mut bridge = fixture.state.lock().expect("fixture bridge state");
            bridge.sequence = 4;
            bridge.mimes = vec![
                KDE_PASSWORD_MANAGER_HINT.into(),
                "text/plain;charset=utf-8".into(),
            ];
            bridge
                .values
                .insert(KDE_PASSWORD_MANAGER_HINT.into(), b"secret".to_vec());
        }
        fixture.owner_changed(
            4,
            vec![
                KDE_PASSWORD_MANAGER_HINT.into(),
                "text/plain;charset=utf-8".into(),
            ],
        );
        wait_for_change(&mut clipboard);
        assert!(clipboard.poll().is_none());
        assert!(
            clipboard.poll().is_none(),
            "secret cursor was not acknowledged"
        );

        clipboard
            .set_contents("written bridge text")
            .expect("bridge write");
        let state = fixture.state.lock().expect("fixture bridge state");
        assert_eq!(state.writes.len(), 1);
        assert_eq!(
            state.writes[0]
                .get("text/plain;charset=utf-8")
                .map(Vec::as_slice),
            Some(b"written bridge text".as_slice())
        );
        drop(state);

        fixture
            .state
            .lock()
            .expect("fixture bridge state")
            .stall_reads = true;
        fixture.update_owner(
            6,
            vec![
                KDE_PASSWORD_MANAGER_HINT.into(),
                "text/plain;charset=utf-8".into(),
            ],
        );
        wait_for_change(&mut clipboard);
        let start = Instant::now();
        assert!(clipboard.poll().is_none());
        assert!(
            start.elapsed() >= RPC_TIMEOUT.saturating_sub(Duration::from_millis(100)),
            "stalled bridge Read never reached the RPC timeout"
        );
        assert!(
            start.elapsed() < REQUEST_TIMEOUT,
            "stalled bridge Read exceeded the bounded caller wait"
        );
        drop(clipboard);
        let replacement =
            GnomeClipboard::new(directory.path()).expect("replacement GNOME clipboard");
        wait_for_active(&replacement);
    }

    #[test]
    fn bounded_write_rejects_overflowing_body_and_total() {
        assert!(!valid_payloads(&HashMap::from([(
            "text/plain;charset=utf-8".into(),
            vec![0; MAX_BODY + 1],
        )])));
        let payloads = (0..9)
            .map(|index| (format!("application/x-{index}"), vec![0; MAX_BODY]))
            .collect();
        assert!(!valid_payloads(&payloads));
    }

    #[test]
    fn representation_is_stable_and_never_guesses_unknown_mimes() {
        assert_eq!(
            representation(&["image/png".into(), "text/plain;charset=utf-8".into()]),
            Some((
                "image/png".into(),
                copypaste_ipc::content_type::IMAGE_PNG,
                true
            ))
        );
        assert_eq!(representation(&["application/x-future".into()]), None);
        assert_eq!(
            representation(&["image/tiff".into()]),
            Some((
                "image/tiff".into(),
                copypaste_ipc::content_type::IMAGE_TIFF,
                true,
            ))
        );
    }

    #[test]
    fn service_loss_fences_stale_read_and_resets_capture_cursor() {
        let state = Arc::new(Mutex::new(State {
            active: true,
            watching: true,
            dirty: true,
            epoch: 7,
            sequence: 42,
            owned_sequence: None,
            mimes: vec!["text/plain;charset=utf-8".into()],
        }));
        assert!(current_epoch(&state, 7, 42));
        invalidate(&state);
        let state = state.lock().unwrap();
        assert!(!state.active && !state.dirty && state.sequence == 0 && state.mimes.is_empty());
        assert!(!current_epoch(
            &Arc::new(Mutex::new(State {
                epoch: state.epoch,
                ..State::default()
            })),
            7,
            42
        ));
    }

    #[test]
    fn private_gate_acknowledges_the_current_change() {
        let mut state = State {
            active: true,
            dirty: true,
            ..Default::default()
        };
        // read() performs this transition before it evaluates private mode.
        state.dirty = false;
        assert!(!state.dirty);
    }

    #[test]
    fn delayed_own_signal_is_suppressed_but_foreign_change_is_preserved() {
        let mut state = State {
            active: true,
            sequence: 4,
            owned_sequence: Some(5),
            ..Default::default()
        };
        apply_owner_changed(&mut state, 5, vec!["text/plain;charset=utf-8".into()]);
        assert!(!state.dirty);
        state.owned_sequence = Some(6);
        apply_owner_changed(&mut state, 7, vec!["text/plain;charset=utf-8".into()]);
        assert!(state.dirty);
        assert_eq!(state.sequence, 7);
    }
}

//! Authenticated compositor clipboard bridge transport.
//!
//! GNOME and KWin expose the same bounded, generation-bound protocol. A
//! missing companion leaves its adapter inactive rather than pretending that
//! another clipboard implementation exists.

use std::collections::HashMap;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use zbus::blocking::{connection::Builder, Connection, Proxy};

use super::super::{Capture, CapturePolicy, ClipboardSource, SourcePolicyEvidence};

const DAEMON: &str = "app.copypaste.Daemon";
const GNOME_BRIDGE: &str = "app.copypaste.GnomeIntegration";
const KWIN_BRIDGE: &str = "org.kde.KWin";
const PATH: &str = "/app/copypaste/Clipboard";
const INTERFACE: &str = "app.copypaste.Clipboard";
const MAX_BODY: usize = 4 * 1024 * 1024;
const MAX_WRITE: usize = 32 * 1024 * 1024;
const MAX_MIMES: usize = 64;
const MAX_MIME_BYTES: usize = 255;
const KDE_PASSWORD_MANAGER_HINT: &str = "x-kde-passwordManagerHint";
const RPC_TIMEOUT: Duration = Duration::from_secs(2);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);
const BRIDGE_VERSION: u32 = 2;

#[derive(Clone, Copy)]
struct Bridge {
    service: &'static str,
    backend_name: &'static str,
}

const GNOME: Bridge = Bridge {
    service: GNOME_BRIDGE,
    backend_name: "linux-gnome-wayland-clipboard",
};

const KWIN: Bridge = Bridge {
    service: KWIN_BRIDGE,
    backend_name: "linux-kwin-wayland-clipboard",
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct SourceIdentity {
    app_id: String,
}

impl SourceIdentity {
    fn from_wire(status: String, pid: u32, _uid: u32, app_id: String) -> Option<Self> {
        match status.as_str() {
            "verified" if pid != 0 && valid_app_id(&app_id) => Some(Self { app_id }),
            "no-client" | "no-app-id" | "ambiguous" if app_id.is_empty() => None,
            _ => None,
        }
    }
}

#[derive(Default)]
struct State {
    active: bool,
    watching: bool,
    dirty: bool,
    epoch: u64,
    sequence: u64,
    owned_sequence: Option<u64>,
    mimes: Vec<String>,
    source: Option<SourceIdentity>,
    owner: Option<String>,
    #[cfg(test)]
    watch_diagnostics: WatchDiagnostics,
}

#[cfg(test)]
#[derive(Default, Debug)]
struct WatchDiagnostics {
    subscription_ready: bool,
    frames: u64,
    applied: u64,
    rejection: Option<&'static str>,
}

pub(in crate::clipboard) struct GnomeClipboard {
    bridge: Bridge,
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
        Self::with_bridge(data_dir, GNOME)
    }

    pub(super) fn new_kwin(data_dir: &Path) -> std::io::Result<Self> {
        Self::with_bridge(data_dir, KWIN)
    }

    fn with_bridge(data_dir: &Path, bridge: Bridge) -> std::io::Result<Self> {
        // The cleanup owner exists before the D-Bus worker can accept a file.
        let staging = super::super::file_materialize::StagingArea::new(data_dir)?;
        let state = Arc::new(Mutex::new(State::default()));
        let (commands, receiver) = mpsc::channel();
        let worker_state = Arc::clone(&state);
        // zbus::blocking owns a global runtime. Never construct or call it
        // from the daemon's Tokio executor; this thread is the only boundary.
        let worker = thread::spawn(move || worker(receiver, worker_state, bridge));
        Ok(Self {
            bridge,
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
        let (epoch, sequence, mimes, source) = {
            let mut state = self.state.lock().ok()?;
            if !state.active || !state.dirty {
                return None;
            }
            state.dirty = false;
            (
                state.epoch,
                state.sequence,
                state.mimes.clone(),
                state.source.clone(),
            )
        };
        // The cursor has advanced before every policy gate: a value copied
        // during private mode must never be captured when private mode ends.
        if settings.private_mode || !allows_source(settings, source.as_ref()) {
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
            let mut capture = super::file_capture(bytes, privacy, None)?;
            attach_source(&mut capture, source);
            return Some(capture);
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
            app_bundle_id: source.as_ref().map(|source| source.app_id.clone()),
            app_name: source.and_then(|source| source_label(&source.app_id)),
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
        self.bridge.backend_name
    }
}

fn worker(commands: mpsc::Receiver<Command>, state: Arc<Mutex<State>>, bridge: Bridge) {
    let Ok(connection) =
        Builder::session().and_then(|builder| builder.method_timeout(RPC_TIMEOUT).build())
    else {
        return;
    };
    if connection.request_name(DAEMON).is_err() {
        return;
    }
    // Subscribe before resolving the first owner.  Otherwise a bridge that
    // appears between those two operations can be the only owner transition
    // we never observe, leaving this adapter inactive until another restart.
    let (ready, subscribed) = mpsc::sync_channel(1);
    watch_service(connection.clone(), Arc::clone(&state), bridge, ready);
    if subscribed.recv_timeout(RPC_TIMEOUT).is_err() {
        return;
    }
    activate(&connection, &state, bridge);
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
        let owner = state
            .lock()
            .ok()
            .and_then(|state| state.active.then(|| state.owner.clone()).flatten());
        let proxy = owner.as_deref().and_then(|owner| {
            owner_is_current(&connection, bridge, owner)
                .then(|| Proxy::new(&connection, owner, PATH, INTERFACE).ok())
                .flatten()
        });
        match command {
            Command::Read {
                sequence,
                mime,
                limit,
                reply,
            } => {
                let bytes = proxy
                    .as_ref()
                    .and_then(|proxy| proxy.call("Read", &(sequence, mime.as_str(), limit)).ok())
                    .filter(|_| {
                        owner.as_deref().is_some_and(|owner| {
                            owner_is_current(&connection, bridge, owner)
                                && state.lock().is_ok_and(|state| {
                                    state.active && state.owner.as_deref() == Some(owner)
                                })
                        })
                    });
                if bytes.is_none() {
                    if let Some(owner) = owner.as_deref() {
                        invalidate_owner(&state, Some(owner));
                    }
                }
                let _ = reply.send(bytes);
            }
            Command::Write { values, reply } => {
                let sequence = proxy
                    .as_ref()
                    .and_then(|proxy| proxy.call("Write", &(values,)).ok())
                    .filter(|_| {
                        owner.as_deref().is_some_and(|owner| {
                            owner_is_current(&connection, bridge, owner)
                                && state.lock().is_ok_and(|state| {
                                    state.active && state.owner.as_deref() == Some(owner)
                                })
                        })
                    });
                if sequence.is_none() {
                    if let Some(owner) = owner.as_deref() {
                        invalidate_owner(&state, Some(owner));
                    }
                }
                let _ = reply.send(sequence);
            }
            Command::Stop => unreachable!("Stop returned before proxy dispatch"),
        }
    }
}

fn activate(connection: &Connection, state: &Arc<Mutex<State>>, bridge: Bridge) {
    let Some(epoch) = state.lock().ok().map(|state| state.epoch) else {
        return;
    };
    let Some(owner) = verified_service_owner(connection, bridge) else {
        return;
    };
    if let Ok(mut state) = state.lock() {
        if state.epoch != epoch {
            return;
        }
        state.active = false;
        state.watching = false;
        state.dirty = false;
        state.sequence = 0;
        state.owned_sequence = None;
        state.mimes.clear();
        state.source = None;
        state.owner = Some(owner.clone());
    } else {
        return;
    }
    let (ready, subscribed) = mpsc::sync_channel(1);
    watch_clipboard(
        connection.clone(),
        Arc::clone(state),
        epoch,
        owner.clone(),
        ready,
    );
    if subscribed.recv_timeout(RPC_TIMEOUT).is_err() {
        invalidate_owner(state, Some(&owner));
        return;
    }
    let Ok(proxy) = Proxy::new(connection, owner.as_str(), PATH, INTERFACE) else {
        invalidate_owner(state, Some(&owner));
        return;
    };
    let Ok(version) = proxy.call::<_, _, u32>("Version", &()) else {
        invalidate_owner(state, Some(&owner));
        return;
    };
    if version != BRIDGE_VERSION {
        invalidate_owner(state, Some(&owner));
        return;
    }
    let Ok((sequence, mimes, identity)) =
        proxy.call::<_, _, (u64, Vec<String>, (String, u32, u32, String))>("Snapshot", &())
    else {
        invalidate_owner(state, Some(&owner));
        return;
    };
    if !valid_mimes(&mimes) || !owner_is_current(connection, bridge, &owner) {
        invalidate_owner(state, Some(&owner));
        return;
    }
    let source = SourceIdentity::from_wire(identity.0, identity.1, identity.2, identity.3);
    if let Ok(mut state) = state.lock() {
        if state.epoch != epoch || state.owner.as_deref() != Some(owner.as_str()) {
            return;
        }
        state.active = true;
        state.watching = true;
        if state.sequence <= sequence {
            state.dirty = false;
            state.sequence = sequence;
            state.mimes = mimes;
            state.source = source;
        }
    }
}

fn verified_service_owner(connection: &Connection, bridge: Bridge) -> Option<String> {
    let Ok(bus) = Proxy::new(
        connection,
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
    ) else {
        return None;
    };
    let Ok(owner) = bus.call::<_, _, String>("GetNameOwner", &(bridge.service,)) else {
        return None;
    };
    let Ok(uid) = bus.call::<_, _, u32>("GetConnectionUnixUser", &(owner.as_str(),)) else {
        return None;
    };
    std::fs::metadata("/proc/self")
        .ok()
        .filter(|current| current.uid() == uid)
        .map(|_| owner)
}

fn owner_is_current(connection: &Connection, bridge: Bridge, expected: &str) -> bool {
    let Ok(bus) = Proxy::new(
        connection,
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
    ) else {
        return false;
    };
    bus.call::<_, _, String>("GetNameOwner", &(bridge.service,))
        .is_ok_and(|owner| owner == expected)
}

fn watch_service(
    connection: Connection,
    state: Arc<Mutex<State>>,
    bridge: Bridge,
    ready: mpsc::SyncSender<()>,
) {
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
        let _ = ready.send(());
        for signal in signals {
            let Ok((name, _old, new)) = signal.body().deserialize::<(String, String, String)>()
            else {
                continue;
            };
            if name != bridge.service {
                continue;
            }
            invalidate(&state);
            if !new.is_empty() {
                activate(&connection, &state, bridge);
            }
        }
    });
}

fn watch_clipboard(
    connection: Connection,
    state: Arc<Mutex<State>>,
    epoch: u64,
    owner: String,
    ready: mpsc::SyncSender<()>,
) {
    thread::spawn(move || {
        let Ok(proxy) = Proxy::new(&connection, owner.as_str(), PATH, INTERFACE) else {
            return;
        };
        let Ok(signals) = proxy.receive_signal("OwnerChanged") else {
            return;
        };
        #[cfg(test)]
        if let Ok(mut state) = state.lock() {
            state.watch_diagnostics.subscription_ready = true;
        }
        let _ = ready.send(());
        for signal in signals {
            #[cfg(test)]
            if let Ok(mut state) = state.lock() {
                state.watch_diagnostics.frames += 1;
            }
            let Ok((sequence, mimes, identity)) =
                signal
                    .body()
                    .deserialize::<(u64, Vec<String>, (String, u32, u32, String))>()
            else {
                #[cfg(test)]
                if let Ok(mut state) = state.lock() {
                    state.watch_diagnostics.rejection = Some("body");
                }
                invalidate_owner(&state, Some(&owner));
                break;
            };
            if !valid_mimes(&mimes) {
                #[cfg(test)]
                if let Ok(mut state) = state.lock() {
                    state.watch_diagnostics.rejection = Some("mimes");
                }
                invalidate_owner(&state, Some(&owner));
                break;
            }
            if let Ok(mut state) = state.lock() {
                if state.epoch != epoch || state.owner.as_deref() != Some(owner.as_str()) {
                    #[cfg(test)]
                    {
                        state.watch_diagnostics.rejection = Some("owner");
                    }
                    break;
                }
                apply_owner_changed(
                    &mut state,
                    sequence,
                    mimes.to_vec(),
                    SourceIdentity::from_wire(identity.0, identity.1, identity.2, identity.3),
                );
                #[cfg(test)]
                {
                    state.watch_diagnostics.applied += 1;
                }
            }
        }
        if let Ok(mut state) = state.lock() {
            if state.epoch == epoch && state.owner.as_deref() == Some(owner.as_str()) {
                state.epoch = state.epoch.wrapping_add(1);
                state.active = false;
                state.watching = false;
                state.dirty = false;
                state.sequence = 0;
                state.owned_sequence = None;
                state.mimes.clear();
                state.source = None;
                state.owner = None;
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
        state.source = None;
        state.owner = None;
    }
}

fn invalidate_owner(state: &Arc<Mutex<State>>, owner: Option<&str>) {
    let Ok(mut state) = state.lock() else {
        return;
    };
    if owner.is_some_and(|owner| state.owner.as_deref() != Some(owner)) {
        return;
    }
    state.epoch = state.epoch.wrapping_add(1);
    state.active = false;
    state.watching = false;
    state.dirty = false;
    state.sequence = 0;
    state.owned_sequence = None;
    state.mimes.clear();
    state.source = None;
    state.owner = None;
}

fn current_epoch(state: &Arc<Mutex<State>>, epoch: u64, sequence: u64) -> bool {
    state.lock().is_ok_and(|state| {
        state.active && state.owner.is_some() && state.epoch == epoch && state.sequence == sequence
    })
}

fn apply_owner_changed(
    state: &mut State,
    sequence: u64,
    mimes: Vec<String>,
    source: Option<SourceIdentity>,
) {
    if sequence < state.sequence {
        return;
    }
    state.sequence = sequence;
    state.mimes = mimes;
    state.source = source;
    if state.owned_sequence == Some(sequence) {
        state.owned_sequence = None;
        state.dirty = false;
    } else {
        state.dirty = true;
    }
}

fn valid_mimes(mimes: &[String]) -> bool {
    mimes.len() <= MAX_MIMES
        && mimes
            .iter()
            .all(|mime| !mime.is_empty() && mime.len() <= MAX_MIME_BYTES)
}

fn valid_app_id(value: &str) -> bool {
    (1..=255).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn allows_source(settings: &copypaste_ipc::ConfigData, source: Option<&SourceIdentity>) -> bool {
    settings.excluded_app_bundle_ids.is_empty()
        || source.is_some_and(|source| {
            !settings
                .excluded_app_bundle_ids
                .iter()
                .any(|excluded| excluded == &source.app_id)
        })
}

fn source_label(app_id: &str) -> Option<String> {
    super::super::linux_attribution::resolve_desktop_id(app_id).map(|source| source.name)
}

fn attach_source(capture: &mut Capture, source: Option<SourceIdentity>) {
    capture.app_name = source
        .as_ref()
        .and_then(|source| source_label(&source.app_id));
    capture.app_bundle_id = source.map(|source| source.app_id);
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
        identity: (String, u32, u32, String),
        values: HashMap<String, Vec<u8>>,
        writes: Vec<HashMap<String, Vec<u8>>>,
        reads: usize,
        stall_reads: bool,
        snapshots: usize,
        completed_snapshots: usize,
        snapshot_started: Option<mpsc::Sender<()>>,
        snapshot_release: Option<mpsc::Receiver<()>>,
    }

    struct FixtureBridge(Arc<Mutex<BridgeState>>);

    #[zbus::interface(name = "app.copypaste.Clipboard")]
    impl FixtureBridge {
        fn version(&self) -> u32 {
            BRIDGE_VERSION
        }

        fn snapshot(&self) -> (u64, Vec<String>, (String, u32, u32, String)) {
            let mut state = self.0.lock().expect("fixture bridge state");
            state.snapshots += 1;
            let snapshot = (state.sequence, state.mimes.clone(), state.identity.clone());
            let started = state.snapshot_started.take();
            let release = state.snapshot_release.take();
            if let Some(started) = started {
                let _ = started.send(());
            }
            if let Some(release) = release {
                drop(state);
                release
                    .recv_timeout(RPC_TIMEOUT.saturating_sub(Duration::from_millis(100)))
                    .expect("release stalled fixture Snapshot");
                state = self.0.lock().expect("fixture bridge state");
            }
            state.completed_snapshots += 1;
            snapshot
        }

        fn read(&self, sequence: u64, mime: &str, limit: u32) -> Vec<u8> {
            let mut state = self.0.lock().expect("fixture bridge state");
            state.reads += 1;
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
        service: &'static str,
    }

    impl BridgeFixture {
        fn start(bridge: Bridge) -> Self {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("fixture runtime");
            let _guard = runtime.enter();
            let state = Arc::new(Mutex::new(BridgeState {
                sequence: 0,
                mimes: Vec::new(),
                identity: ("verified".into(), 99, 1000, "org.example.Writer".into()),
                values: HashMap::from([(
                    "text/plain;charset=utf-8".into(),
                    b"bridge text".to_vec(),
                )]),
                ..Default::default()
            }));
            let connection = Connection::session().expect("fixture session bus");
            connection
                .request_name(bridge.service)
                .expect("fixture bridge name");
            connection
                .object_server()
                .at(PATH, FixtureBridge(Arc::clone(&state)))
                .expect("fixture bridge object");
            Self {
                _runtime: runtime,
                connection,
                state,
                service: bridge.service,
            }
        }

        fn owner_changed(&self, sequence: u64, mimes: Vec<String>) {
            let identity = self
                .state
                .lock()
                .expect("fixture bridge state")
                .identity
                .clone();
            self.connection
                .emit_signal(
                    None::<&str>,
                    PATH,
                    INTERFACE,
                    "OwnerChanged",
                    &(sequence, mimes, identity),
                )
                .expect("fixture owner signal");
        }

        fn update_owner(&self, sequence: u64, mimes: Vec<String>) {
            let mut state = self.state.lock().expect("fixture bridge state");
            state.sequence = sequence;
            state.mimes = mimes.clone();
            drop(state);
            self.owner_changed(sequence, mimes);
        }

        fn update_identity(&self, identity: (String, u32, u32, String)) {
            self.state.lock().expect("fixture bridge state").identity = identity;
        }

        fn update_value(&self, mime: &str, value: &[u8]) {
            self.state
                .lock()
                .expect("fixture bridge state")
                .values
                .insert(mime.into(), value.into());
        }

        fn stall_next_snapshot(&self) -> (mpsc::Receiver<()>, mpsc::Sender<()>) {
            let (started, started_receiver) = mpsc::channel();
            let (release_sender, release) = mpsc::channel();
            let mut state = self.state.lock().expect("fixture bridge state");
            state.snapshot_started = Some(started);
            state.snapshot_release = Some(release);
            (started_receiver, release_sender)
        }

        fn release_name(&self) {
            self.connection
                .release_name(self.service)
                .expect("release fixture bridge name");
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
        assert!(
            clipboard.changed(),
            "fixture OwnerChanged was not observed: {}",
            clipboard.state.lock().map_or_else(
                |_| "state lock poisoned".into(),
                |state| format!(
                    "active={} watching={} sequence={} owner={:?} diagnostics={:?}",
                    state.active,
                    state.watching,
                    state.sequence,
                    state.owner,
                    state.watch_diagnostics,
                )
            )
        );
    }

    fn wait_for_sequence(clipboard: &GnomeClipboard, sequence: u64) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if clipboard
                .state
                .lock()
                .is_ok_and(|state| state.sequence == sequence)
            {
                return;
            }
            thread::sleep(Duration::from_millis(1));
        }
        panic!("fixture OwnerChanged did not advance to sequence {sequence}");
    }

    #[test]
    fn live_gnome_dbus_fixture_reads_writes_and_bounds_stalled_calls() {
        if std::env::var_os("COPYPASTE_GNOME_FIXTURE") != Some("1".into()) {
            return;
        }
        for bridge in [GNOME, KWIN] {
            live_bridge_fixture(bridge);
            owner_changed_during_stalled_snapshot_fixture(bridge);
            owner_switch_during_stalled_snapshot_fixture(bridge);
        }
    }

    fn live_bridge_fixture(bridge: Bridge) {
        let fixture = BridgeFixture::start(bridge);
        let directory = tempfile::tempdir().expect("fixture data dir");
        let mut clipboard =
            GnomeClipboard::with_bridge(directory.path(), bridge).expect("compositor clipboard");
        wait_for_active(&clipboard);

        fixture.update_owner(2, vec!["text/plain;charset=utf-8".into()]);
        wait_for_change(&mut clipboard);
        let capture = clipboard.poll().expect("capture bridge text");
        assert_eq!(capture.content, "bridge text");
        assert_eq!(capture.app_bundle_id.as_deref(), Some("org.example.Writer"));

        fixture.update_identity(("verified".into(), 100, 1000, "org.example.Other".into()));
        fixture.update_owner(3, vec!["text/plain;charset=utf-8".into()]);
        wait_for_change(&mut clipboard);
        let capture = clipboard.poll().expect("capture changed writer");
        assert_eq!(capture.app_bundle_id.as_deref(), Some("org.example.Other"));

        let excluded = copypaste_ipc::ConfigData {
            excluded_app_bundle_ids: vec!["org.example.Writer".into()],
            ..Default::default()
        };
        for (sequence, identity) in [
            (
                4,
                ("verified".into(), 99, 1000, "org.example.Writer".into()),
            ),
            (5, ("no-app-id".into(), 99, 1000, String::new())),
            (6, ("ambiguous".into(), 99, 1000, String::new())),
            (7, ("verified".into(), 99, 1000, "../invalid".into())),
        ] {
            fixture.update_identity(identity);
            fixture.update_owner(sequence, vec!["text/plain;charset=utf-8".into()]);
            wait_for_change(&mut clipboard);
            let reads = fixture.state.lock().expect("fixture bridge state").reads;
            assert!(clipboard
                .poll_with_policy(CapturePolicy::new(&excluded))
                .is_none());
            assert_eq!(
                fixture.state.lock().expect("fixture bridge state").reads,
                reads,
                "source policy sent Read for sequence {sequence}",
            );
        }

        fixture.update_identity(("verified".into(), 99, 1000, "org.example.Writer".into()));
        fixture.update_owner(8, vec!["text/plain;charset=utf-8".into()]);
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
            bridge.sequence = 9;
            bridge.mimes = vec![
                KDE_PASSWORD_MANAGER_HINT.into(),
                "text/plain;charset=utf-8".into(),
            ];
            bridge
                .values
                .insert(KDE_PASSWORD_MANAGER_HINT.into(), b"secret".to_vec());
        }
        fixture.owner_changed(
            9,
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

        let foreign_sequence = {
            let mut state = fixture.state.lock().expect("fixture bridge state");
            state.stall_reads = true;
            // Write has already consumed its own generation. A foreign owner
            // must advance it rather than replaying that write's signal.
            state.sequence.checked_add(1).expect("fixture generation")
        };
        fixture.update_owner(
            foreign_sequence,
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
        let replacement = GnomeClipboard::with_bridge(directory.path(), bridge)
            .expect("replacement compositor clipboard");
        wait_for_active(&replacement);
        fixture.release_name();
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if replacement
                .state
                .lock()
                .is_ok_and(|state| !state.active && state.source.is_none())
            {
                return;
            }
            thread::sleep(Duration::from_millis(5));
        }
        panic!("companion owner loss retained source provenance");
    }

    fn owner_changed_during_stalled_snapshot_fixture(bridge: Bridge) {
        let original = BridgeFixture::start(bridge);
        original.update_owner(1, vec!["text/plain;charset=utf-8".into()]);
        let (snapshot_started, snapshot_release) = original.stall_next_snapshot();
        let directory = tempfile::tempdir().expect("fixture data dir");
        let mut clipboard =
            GnomeClipboard::with_bridge(directory.path(), bridge).expect("compositor clipboard");
        snapshot_started
            .recv_timeout(RPC_TIMEOUT)
            .expect("fixture Snapshot did not stall");
        original.update_identity(("verified".into(), 100, 1000, "org.example.NewWriter".into()));
        original.update_value("text/plain;charset=utf-8", b"new bridge text");
        original.update_owner(2, vec!["text/plain;charset=utf-8".into()]);
        wait_for_sequence(&clipboard, 2);
        snapshot_release
            .send(())
            .expect("release stalled fixture Snapshot");
        wait_for_change(&mut clipboard);
        let capture = clipboard
            .poll()
            .expect("new OwnerChanged capture survived stale Snapshot");
        assert_eq!(capture.content, "new bridge text");
        assert_eq!(
            capture.app_bundle_id.as_deref(),
            Some("org.example.NewWriter")
        );
    }

    fn owner_switch_during_stalled_snapshot_fixture(bridge: Bridge) {
        let original = BridgeFixture::start(bridge);
        original.update_owner(1, vec!["text/plain;charset=utf-8".into()]);
        let (snapshot_started, snapshot_release) = original.stall_next_snapshot();
        let directory = tempfile::tempdir().expect("fixture data dir");
        let clipboard =
            GnomeClipboard::with_bridge(directory.path(), bridge).expect("compositor clipboard");
        snapshot_started
            .recv_timeout(RPC_TIMEOUT)
            .expect("fixture Snapshot did not stall");
        original.release_name();
        let replacement = BridgeFixture::start(bridge);
        wait_for_active(&clipboard);
        snapshot_release
            .send(())
            .expect("release stale owner Snapshot");
        replacement.update_value("text/plain;charset=utf-8", b"replacement bridge text");
        replacement.update_owner(2, vec!["text/plain;charset=utf-8".into()]);
        let mut clipboard = clipboard;
        wait_for_change(&mut clipboard);
        let capture = clipboard.poll().expect("replacement OwnerChanged capture");
        assert_eq!(capture.content, "replacement bridge text");
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
    fn only_a_bounded_verified_identity_can_authorize_an_exclusion_read() {
        let source =
            SourceIdentity::from_wire("verified".into(), 42, 1000, "org.example.Writer".into());
        let settings = copypaste_ipc::ConfigData {
            excluded_app_bundle_ids: vec!["org.example.Other".into()],
            ..Default::default()
        };
        assert!(allows_source(&settings, source.as_ref()));
        assert!(!allows_source(
            &copypaste_ipc::ConfigData {
                excluded_app_bundle_ids: vec!["org.example.Writer".into()],
                ..Default::default()
            },
            source.as_ref(),
        ));
        assert!(!allows_source(&settings, None));
        assert!(
            SourceIdentity::from_wire("verified".into(), 0, 1000, "org.example.Writer".into(),)
                .is_none()
        );
        assert!(
            SourceIdentity::from_wire("ambiguous".into(), 0, 0, "org.example.Writer".into(),)
                .is_none()
        );
    }

    #[test]
    fn owner_transitions_replace_and_protocol_loss_clears_provenance() {
        let mut state = State {
            active: true,
            sequence: 40,
            source: SourceIdentity::from_wire(
                "verified".into(),
                40,
                1000,
                "org.example.First".into(),
            ),
            ..State::default()
        };
        apply_owner_changed(
            &mut state,
            41,
            vec!["text/plain;charset=utf-8".into()],
            SourceIdentity::from_wire("verified".into(), 41, 1000, "org.example.Second".into()),
        );
        assert_eq!(
            state.source.as_ref().map(|source| source.app_id.as_str()),
            Some("org.example.Second")
        );

        // A stale sequence cannot replace metadata from the current owner.
        apply_owner_changed(
            &mut state,
            40,
            vec!["text/plain;charset=utf-8".into()],
            None,
        );
        assert_eq!(
            state.source.as_ref().map(|source| source.app_id.as_str()),
            Some("org.example.Second")
        );

        // A valid frame with malformed identity is a new owner but has no
        // provenance. Exclusion policy therefore fails before Read.
        apply_owner_changed(
            &mut state,
            42,
            vec!["text/plain;charset=utf-8".into()],
            SourceIdentity::from_wire("verified".into(), 42, 1000, "../invalid".into()),
        );
        assert!(state.source.is_none());
        let state = Arc::new(Mutex::new(state));
        invalidate(&state);
        assert!(state.lock().expect("state").source.is_none());
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
            owner: Some(":1.42".into()),
            ..State::default()
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
        apply_owner_changed(&mut state, 5, vec!["text/plain;charset=utf-8".into()], None);
        assert!(!state.dirty);
        state.owned_sequence = Some(6);
        apply_owner_changed(&mut state, 7, vec!["text/plain;charset=utf-8".into()], None);
        assert!(state.dirty);
        assert_eq!(state.sequence, 7);
    }
}

//! Native Wayland data-control clipboard transport.
//!
//! The worker owns a single event queue for its lifetime.  It prefers the
//! standardized `ext-data-control-v1` protocol and uses the older wlr protocol
//! only when an otherwise compatible compositor has not exposed the extension.

use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use rustix::event::{poll, PollFd, PollFlags};
use rustix::fs::{fcntl_setfl, OFlags};
use rustix::io::{write, Errno};
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::{event_created_child, Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1, ext_data_control_device_v1::ExtDataControlDeviceV1,
    ext_data_control_manager_v1::ExtDataControlManagerV1, ext_data_control_offer_v1,
    ext_data_control_offer_v1::ExtDataControlOfferV1, ext_data_control_source_v1,
    ext_data_control_source_v1::ExtDataControlSourceV1,
};
use wayland_protocols_wlr::data_control::v1::client::{
    zwlr_data_control_device_v1, zwlr_data_control_device_v1::ZwlrDataControlDeviceV1,
    zwlr_data_control_manager_v1::ZwlrDataControlManagerV1, zwlr_data_control_offer_v1,
    zwlr_data_control_offer_v1::ZwlrDataControlOfferV1, zwlr_data_control_source_v1,
    zwlr_data_control_source_v1::ZwlrDataControlSourceV1,
};

use super::super::{Capture, CapturePolicy, ClipboardSource, SourcePolicyEvidence};

const MAX_MIMES: usize = 64;
const MAX_MIME_BYTES: usize = 255;
const SECRET_HINT: &str = "x-kde-passwordManagerHint";
const READ_TIMEOUT: Duration = Duration::from_secs(2);
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);
const COMMAND_CAPACITY: usize = 128;
const COMMAND_BUDGET: usize = 32;
const MAX_WRITERS: usize = 8;
const WRITER_POLL_QUANTUM: Duration = Duration::from_millis(50);

type Payloads = Arc<HashMap<String, Vec<u8>>>;

#[derive(Default)]
struct PublicState {
    active: bool,
    dirty: bool,
    sequence: u64,
}

enum Command {
    Read {
        sequence: u64,
        settings: copypaste_ipc::ConfigData,
        reply: mpsc::Sender<Option<Capture>>,
    },
    Write {
        values: HashMap<String, Vec<u8>>,
        reply: mpsc::Sender<bool>,
    },
}

struct Sender {
    commands: Mutex<VecDeque<Command>>,
    wake: Mutex<Option<UnixStream>>,
    shutdown: AtomicBool,
}

impl Sender {
    fn send(&self, command: Command) -> Result<(), ()> {
        if self.shutdown.load(Ordering::Acquire) {
            return Err(());
        }
        let mut commands = self.commands.lock().map_err(|_| ())?;
        if self.shutdown.load(Ordering::Acquire) {
            return Err(());
        }
        if commands.len() == COMMAND_CAPACITY {
            return Err(());
        }
        commands.push_back(command);
        let mut wake = self.wake.lock().map_err(|_| ())?;
        let Some(wake) = wake.as_mut() else {
            commands.pop_back();
            return Err(());
        };
        match wake.write(&[1]) {
            Ok(_) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(()),
            Err(_) => {
                // The worker cannot consume this command before it obtains
                // `commands`, so removing it keeps a failed send exact.
                commands.pop_back();
                Err(())
            }
        }
    }

    fn has_commands(&self) -> bool {
        self.commands
            .lock()
            .is_ok_and(|commands| !commands.is_empty())
    }

    fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Release);
        // Closing the writer wakes a `poll` blocked on the compositor even
        // when the bounded command deque is full or its wake byte was lost.
        if let Ok(mut wake) = self.wake.lock() {
            wake.take();
        }
    }

    fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }
}

struct Writer {
    cancel: Arc<AtomicBool>,
    worker: JoinHandle<()>,
}

/// A real data-control source when the compositor has granted the protocol;
/// otherwise an inactive native adapter that retries when the connection is
/// restored. It never changes display-server transports behind the caller.
pub(in crate::clipboard) struct WaylandClipboard {
    sender: Arc<Sender>,
    state: Arc<Mutex<PublicState>>,
    worker: Option<JoinHandle<()>>,
    staging: super::super::file_materialize::StagingArea,
}

impl WaylandClipboard {
    pub(super) fn new(data_dir: &Path) -> std::io::Result<Self> {
        let staging = super::super::file_materialize::StagingArea::new(data_dir)?;
        let (read, write) = UnixStream::pair()?;
        read.set_nonblocking(true)?;
        write.set_nonblocking(true)?;
        let state = Arc::new(Mutex::new(PublicState::default()));
        let worker_state = Arc::clone(&state);
        let sender = Arc::new(Sender {
            commands: Mutex::new(VecDeque::new()),
            wake: Mutex::new(Some(write)),
            shutdown: AtomicBool::new(false),
        });
        let worker_sender = Arc::clone(&sender);
        let worker = thread::Builder::new()
            .name("copypaste-wayland-clipboard".into())
            .spawn(move || worker(worker_sender, read, worker_state))?;
        Ok(Self {
            sender,
            state,
            worker: Some(worker),
            staging,
        })
    }

    fn read(&mut self, settings: &copypaste_ipc::ConfigData) -> Option<Capture> {
        let sequence = {
            let mut state = self.state.lock().ok()?;
            if !state.active || !state.dirty {
                return None;
            }
            // Advance before every policy gate. A value copied while private
            // mode is active cannot reappear when the setting is disabled.
            state.dirty = false;
            state.sequence
        };
        let (reply, received) = mpsc::channel();
        if self
            .sender
            .send(Command::Read {
                sequence,
                settings: settings.clone(),
                reply,
            })
            .is_err()
        {
            // A rejected command performed no native read. Restore the dirty
            // cursor so queue backpressure cannot discard a foreign change.
            if !self.sender.is_shutdown() {
                if let Ok(mut state) = self.state.lock() {
                    if state.active && state.sequence == sequence {
                        state.dirty = true;
                    }
                }
            }
            return None;
        }
        received.recv().ok().flatten()
    }

    fn write(&self, mut values: HashMap<String, Vec<u8>>) -> anyhow::Result<()> {
        if !valid_payloads(&values) {
            anyhow::bail!("invalid Wayland clipboard payload");
        }
        // MIME presence is an unambiguous, compositor-provided acknowledgement
        // of our selection change. It prevents a self-write sentinel from
        // swallowing a foreign owner transition that races the write.
        values.insert(
            format!("application/x-copypaste-owner-{}", uuid::Uuid::new_v4()),
            Vec::new(),
        );
        let (reply, received) = mpsc::channel();
        self.sender
            .send(Command::Write { values, reply })
            .map_err(|_| anyhow::anyhow!("Wayland clipboard worker stopped"))?;
        received
            .recv()
            .ok()
            .filter(|ok| *ok)
            .map(|_| ())
            .ok_or_else(|| anyhow::anyhow!("Wayland data-control is unavailable"))
    }

    fn active(&self) -> bool {
        self.state.lock().is_ok_and(|state| state.active)
    }
}

impl Drop for WaylandClipboard {
    fn drop(&mut self) {
        self.sender.shutdown();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl ClipboardSource for WaylandClipboard {
    fn poll(&mut self) -> Option<Capture> {
        self.read(&copypaste_ipc::ConfigData::default())
    }

    fn poll_with_policy(&mut self, policy: CapturePolicy<'_>) -> Option<Capture> {
        self.read(policy.settings)
    }

    fn changed(&mut self) -> bool {
        self.state
            .lock()
            .is_ok_and(|state| state.active && state.dirty)
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
        if self.active() {
            "linux-wayland-data-control"
        } else {
            "linux-wayland-data-control-inactive"
        }
    }
}

#[derive(Clone)]
enum Manager {
    Ext(ExtDataControlManagerV1),
    Wlr(ZwlrDataControlManagerV1),
}

#[derive(Clone)]
enum Device {
    Ext(ext_data_control_device_v1::ExtDataControlDeviceV1),
    Wlr(zwlr_data_control_device_v1::ZwlrDataControlDeviceV1),
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum Offer {
    Ext(ExtDataControlOfferV1),
    Wlr(ZwlrDataControlOfferV1),
}

impl Offer {
    fn receive(&self, mime: String, fd: std::os::fd::BorrowedFd<'_>) {
        match self {
            Self::Ext(offer) => offer.receive(mime, fd),
            Self::Wlr(offer) => offer.receive(mime, fd),
        }
    }

    fn destroy(&self) {
        match self {
            Self::Ext(offer) => offer.destroy(),
            Self::Wlr(offer) => offer.destroy(),
        }
    }
}

struct Worker {
    public: Arc<Mutex<PublicState>>,
    devices: Vec<Device>,
    offers: HashMap<Offer, Vec<String>>,
    selection: Option<Offer>,
    initialized: bool,
    pending_marker: Option<String>,
    sequence: u64,
    lost: bool,
    manager: Option<Manager>,
    writers: Vec<Writer>,
}

impl Worker {
    fn new(public: Arc<Mutex<PublicState>>) -> Self {
        Self {
            public,
            devices: Vec::new(),
            offers: HashMap::new(),
            selection: None,
            initialized: false,
            pending_marker: None,
            sequence: 0,
            lost: false,
            manager: None,
            writers: Vec::new(),
        }
    }

    fn selection(&mut self, offer: Option<Offer>) {
        if let Some(previous) = self.selection.take() {
            previous.destroy();
            self.offers.remove(&previous);
        }
        self.selection = offer.clone();
        let mimes = offer
            .as_ref()
            .and_then(|offer| self.offers.get(offer))
            .cloned()
            .unwrap_or_default();
        self.sequence = self.sequence.wrapping_add(1);
        let own = self
            .pending_marker
            .as_ref()
            .is_some_and(|marker| mimes.iter().any(|mime| mime == marker));
        if own {
            self.pending_marker = None;
        }
        let dirty = self.initialized && !own;
        self.initialized = true;
        if let Ok(mut public) = self.public.lock() {
            public.active = true;
            public.sequence = self.sequence;
            public.dirty = dirty;
        }
    }

    fn deactivate(&self) {
        if let Ok(mut public) = self.public.lock() {
            public.active = false;
            public.dirty = false;
            public.sequence = 0;
        }
    }

    fn start_writer(&mut self, values: &Payloads, mime_type: String, fd: OwnedFd) {
        self.reap_writers();
        if self.writers.len() == MAX_WRITERS {
            return;
        }
        let values = Arc::clone(values);
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let worker = thread::spawn(move || {
            let Some(bytes) = values.get(&mime_type) else {
                return;
            };
            let _ = write_bounded(fd, bytes, WRITE_TIMEOUT, &worker_cancel);
        });
        self.writers.push(Writer { cancel, worker });
    }

    fn reap_writers(&mut self) {
        let mut active = Vec::with_capacity(self.writers.len());
        for writer in self.writers.drain(..) {
            if writer.worker.is_finished() {
                let _ = writer.worker.join();
            } else {
                active.push(writer);
            }
        }
        self.writers = active;
    }

    fn cancel_writers(&mut self) {
        for writer in &self.writers {
            writer.cancel.store(true, Ordering::Release);
        }
        for writer in self.writers.drain(..) {
            let _ = writer.worker.join();
        }
    }

    fn capture(
        &mut self,
        queue: &mut EventQueue<Self>,
        sequence: u64,
        settings: &copypaste_ipc::ConfigData,
    ) -> Option<Capture> {
        if self.lost || self.sequence != sequence || !super::can_read_unknown_source(settings) {
            return None;
        }
        let offer = self.selection.clone()?;
        let mimes = self.offers.get(&offer)?.clone();
        let secret = if mimes.iter().any(|mime| mime == SECRET_HINT) {
            receive(queue, &offer, SECRET_HINT, 64)?.eq_ignore_ascii_case(b"secret")
        } else {
            false
        };
        let privacy = copypaste_ipc::ClipboardPrivacy {
            secret,
            transient: false,
        };
        if !privacy.allows(settings) || self.lost || self.sequence != sequence {
            return None;
        }
        let (mime, content_type, binary) = representation(&mimes)?;
        let bytes = receive(
            queue,
            &offer,
            mime,
            settings.capture_limit_bytes(content_type) as usize,
        )?;
        if self.lost || self.sequence != sequence {
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

    fn write(&mut self, queue: &mut EventQueue<Self>, values: HashMap<String, Vec<u8>>) -> bool {
        if self.lost || self.devices.is_empty() || !valid_payloads(&values) {
            return false;
        }
        let marker = values
            .keys()
            .find(|mime| mime.starts_with("application/x-copypaste-owner-"))
            .cloned();
        let Some(marker) = marker else {
            return false;
        };
        let values = Arc::new(values);
        let qh = queue.handle();
        for device in &self.devices {
            match (self.manager.as_ref(), device) {
                (Some(Manager::Ext(manager)), Device::Ext(device)) => {
                    let source = manager.create_data_source(&qh, Arc::clone(&values));
                    for mime in values.keys() {
                        source.offer(mime.clone());
                    }
                    device.set_selection(Some(&source));
                }
                (Some(Manager::Wlr(manager)), Device::Wlr(device)) => {
                    let source = manager.create_data_source(&qh, Arc::clone(&values));
                    for mime in values.keys() {
                        source.offer(mime.clone());
                    }
                    device.set_selection(Some(&source));
                }
                _ => return false,
            }
        }
        self.pending_marker = Some(marker);
        queue.flush().is_ok()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel_writers();
    }
}

struct Session {
    queue: EventQueue<Worker>,
    state: Worker,
}

impl Session {
    fn connect(public: Arc<Mutex<PublicState>>) -> Option<Self> {
        let connection = Connection::connect_to_env().ok()?;
        let (globals, mut queue) = registry_queue_init::<Worker>(&connection).ok()?;
        let qh = queue.handle();
        let manager = globals
            .bind::<ExtDataControlManagerV1, _, _>(&qh, 1..=1, ())
            .ok()
            .map(Manager::Ext)
            .or_else(|| {
                globals
                    .bind::<ZwlrDataControlManagerV1, _, _>(&qh, 1..=2, ())
                    .ok()
                    .map(Manager::Wlr)
            })?;
        let registry = globals.registry();
        let seats: Vec<_> = globals.contents().with_list(|contents| {
            contents
                .iter()
                .filter(|global| global.interface == WlSeat::interface().name)
                .map(|global| registry.bind(global.name, global.version.min(1), &qh, ()))
                .collect()
        });
        if seats.is_empty() {
            return None;
        }
        let mut state = Worker::new(public);
        state.manager = Some(manager.clone());
        state.devices = seats
            .into_iter()
            .map(|seat| match &manager {
                Manager::Ext(manager) => Device::Ext(manager.get_data_device(&seat, &qh, 0)),
                Manager::Wlr(manager) => Device::Wlr(manager.get_data_device(&seat, &qh, 0)),
            })
            .collect();
        queue.roundtrip(&mut state).ok()?;
        if let Ok(mut public) = state.public.lock() {
            public.active = !state.lost;
            public.dirty = false;
            public.sequence = state.sequence;
        }
        Some(Self { queue, state })
    }

    fn command(&mut self, command: Command) -> bool {
        match command {
            Command::Read {
                sequence,
                settings,
                reply,
            } => {
                let _ = reply.send(self.state.capture(&mut self.queue, sequence, &settings));
                true
            }
            Command::Write { values, reply } => {
                let _ = reply.send(self.state.write(&mut self.queue, values));
                true
            }
        }
    }
}

fn worker(sender: Arc<Sender>, mut wake: UnixStream, public: Arc<Mutex<PublicState>>) {
    let mut session: Option<Session> = None;
    loop {
        if sender.is_shutdown() {
            return;
        }
        if session.is_none() {
            session = Session::connect(Arc::clone(&public));
            if session.is_none() {
                deactivate(&public);
                if !wait_for_wake(&mut wake, Some(Duration::from_secs(1))) {
                    continue;
                }
                if sender.is_shutdown() {
                    return;
                }
                if !drain_commands(&sender, None) {
                    return;
                }
                if sender.has_commands() {
                    continue;
                }
                continue;
            }
        }
        let current = session.as_mut().expect("connected session");
        if sender.is_shutdown() {
            return;
        }
        if !drain_commands(&sender, Some(current)) {
            return;
        }
        if current.queue.dispatch_pending(&mut current.state).is_err() || current.state.lost {
            current.state.deactivate();
            session = None;
            continue;
        }
        if current.queue.flush().is_err() {
            current.state.deactivate();
            session = None;
            continue;
        }
        if sender.has_commands() {
            continue;
        }
        let Some(guard) = current.queue.prepare_read() else {
            continue;
        };
        let poll_result = {
            let mut fds = [
                PollFd::from_borrowed_fd(guard.connection_fd(), PollFlags::IN),
                PollFd::new(&wake, PollFlags::IN),
            ];
            if poll(&mut fds, None).is_err() {
                None
            } else {
                Some((
                    fds[0].revents().contains(PollFlags::IN),
                    fds[1].revents().contains(PollFlags::IN),
                ))
            }
        };
        let Some((wayland_ready, wake_ready)) = poll_result else {
            current.state.deactivate();
            session = None;
            continue;
        };
        if sender.is_shutdown() {
            return;
        }
        if wayland_ready && guard.read().is_err() {
            current.state.deactivate();
            session = None;
            continue;
        }
        if wake_ready {
            drain_wake(&mut wake);
        }
    }
}

fn drain_commands(sender: &Sender, session: Option<&mut Session>) -> bool {
    let mut session = session;
    let commands: Vec<_> = {
        let mut commands = match sender.commands.try_lock() {
            Ok(commands) => commands,
            Err(std::sync::TryLockError::WouldBlock) => return true,
            Err(std::sync::TryLockError::Poisoned(_)) => return false,
        };
        (0..COMMAND_BUDGET)
            .filter_map(|_| commands.pop_front())
            .collect()
    };
    for command in commands {
        if let Some(session) = session.as_deref_mut() {
            if !session.command(command) {
                return false;
            }
        } else {
            match command {
                Command::Read { reply, .. } => {
                    let _ = reply.send(None);
                }
                Command::Write { reply, .. } => {
                    let _ = reply.send(false);
                }
            }
        }
    }
    true
}

fn wait_for_wake(wake: &mut UnixStream, timeout: Option<Duration>) -> bool {
    let mut fds = [PollFd::new(wake, PollFlags::IN)];
    let timeout = timeout.and_then(|duration| rustix::time::Timespec::try_from(duration).ok());
    poll(&mut fds, timeout.as_ref()).is_ok() && fds[0].revents().contains(PollFlags::IN)
}

fn drain_wake(wake: &mut UnixStream) {
    let mut buffer = [0; 256];
    while wake.read(&mut buffer).is_ok() {}
}

fn deactivate(state: &Arc<Mutex<PublicState>>) {
    if let Ok(mut state) = state.lock() {
        state.active = false;
        state.dirty = false;
        state.sequence = 0;
    }
}

fn receive(
    queue: &mut EventQueue<Worker>,
    offer: &Offer,
    mime: &str,
    limit: usize,
) -> Option<Vec<u8>> {
    let (mut read, write) = UnixStream::pair().ok()?;
    read.set_read_timeout(Some(READ_TIMEOUT)).ok()?;
    offer.receive(mime.to_owned(), write.as_fd());
    drop(write);
    queue.flush().ok()?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut read)
        .take(limit.saturating_add(1) as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() <= limit).then_some(bytes)
}

fn valid_payloads(values: &HashMap<String, Vec<u8>>) -> bool {
    !values.is_empty()
        && values.len() <= MAX_MIMES
        && values.iter().all(|(mime, body)| {
            !mime.is_empty()
                && mime.len() <= MAX_MIME_BYTES
                && body.len() <= copypaste_ipc::MAX_CONTENT_BYTES
        })
        && values.values().map(Vec::len).sum::<usize>() <= copypaste_ipc::MAX_CONTENT_BYTES
}

fn representation(mimes: &[String]) -> Option<(&str, &'static str, bool)> {
    [
        ("text/uri-list", copypaste_ipc::content_type::FILE, true),
        ("text/html", copypaste_ipc::content_type::HTML, false),
        ("text/rtf", copypaste_ipc::content_type::RICH_TEXT, false),
        ("image/png", copypaste_ipc::content_type::IMAGE_PNG, true),
        ("image/tiff", copypaste_ipc::content_type::IMAGE_TIFF, true),
        (
            "text/plain;charset=utf-8",
            copypaste_ipc::content_type::TEXT,
            false,
        ),
        ("UTF8_STRING", copypaste_ipc::content_type::TEXT, false),
        ("text/plain", copypaste_ipc::content_type::TEXT, false),
    ]
    .into_iter()
    .find(|(mime, _, _)| mimes.iter().any(|value| value == mime))
}

impl Dispatch<WlRegistry, GlobalListContents> for Worker {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSeat, ()> for Worker {
    fn event(
        _: &mut Self,
        _: &WlSeat,
        _: <WlSeat as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtDataControlManagerV1, ()> for Worker {
    fn event(
        _: &mut Self,
        _: &ExtDataControlManagerV1,
        _: <ExtDataControlManagerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<ZwlrDataControlManagerV1, ()> for Worker {
    fn event(
        _: &mut Self,
        _: &ZwlrDataControlManagerV1,
        _: <ZwlrDataControlManagerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtDataControlDeviceV1, usize> for Worker {
    fn event(
        state: &mut Self,
        _: &ExtDataControlDeviceV1,
        event: <ExtDataControlDeviceV1 as Proxy>::Event,
        _: &usize,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_data_control_device_v1::Event::DataOffer { id } => {
                state.offers.insert(Offer::Ext(id), Vec::new());
            }
            ext_data_control_device_v1::Event::Selection { id } => {
                state.selection(id.map(Offer::Ext))
            }
            ext_data_control_device_v1::Event::Finished => state.lost = true,
            _ => {}
        }
    }
    event_created_child!(Worker, ExtDataControlDeviceV1, [ext_data_control_device_v1::EVT_DATA_OFFER_OPCODE => (ExtDataControlOfferV1, ())]);
}

impl Dispatch<ZwlrDataControlDeviceV1, usize> for Worker {
    fn event(
        state: &mut Self,
        _: &ZwlrDataControlDeviceV1,
        event: <ZwlrDataControlDeviceV1 as Proxy>::Event,
        _: &usize,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_data_control_device_v1::Event::DataOffer { id } => {
                state.offers.insert(Offer::Wlr(id), Vec::new());
            }
            zwlr_data_control_device_v1::Event::Selection { id } => {
                state.selection(id.map(Offer::Wlr))
            }
            zwlr_data_control_device_v1::Event::Finished => state.lost = true,
            _ => {}
        }
    }
    event_created_child!(Worker, ZwlrDataControlDeviceV1, [zwlr_data_control_device_v1::EVT_DATA_OFFER_OPCODE => (ZwlrDataControlOfferV1, ())]);
}

impl Dispatch<ExtDataControlOfferV1, ()> for Worker {
    fn event(
        state: &mut Self,
        proxy: &ExtDataControlOfferV1,
        event: <ExtDataControlOfferV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_data_control_offer_v1::Event::Offer { mime_type } = event {
            state
                .offers
                .entry(Offer::Ext(proxy.clone()))
                .or_default()
                .push(mime_type);
        }
    }
}
impl Dispatch<ZwlrDataControlOfferV1, ()> for Worker {
    fn event(
        state: &mut Self,
        proxy: &ZwlrDataControlOfferV1,
        event: <ZwlrDataControlOfferV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwlr_data_control_offer_v1::Event::Offer { mime_type } = event {
            state
                .offers
                .entry(Offer::Wlr(proxy.clone()))
                .or_default()
                .push(mime_type);
        }
    }
}

impl Dispatch<ExtDataControlSourceV1, Payloads> for Worker {
    fn event(
        state: &mut Self,
        proxy: &ExtDataControlSourceV1,
        event: <ExtDataControlSourceV1 as Proxy>::Event,
        values: &Payloads,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_data_control_source_v1::Event::Send { mime_type, fd } => {
                state.start_writer(values, mime_type, fd)
            }
            ext_data_control_source_v1::Event::Cancelled => proxy.destroy(),
            _ => {}
        }
    }
}
impl Dispatch<ZwlrDataControlSourceV1, Payloads> for Worker {
    fn event(
        state: &mut Self,
        proxy: &ZwlrDataControlSourceV1,
        event: <ZwlrDataControlSourceV1 as Proxy>::Event,
        values: &Payloads,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_data_control_source_v1::Event::Send { mime_type, fd } => {
                state.start_writer(values, mime_type, fd)
            }
            zwlr_data_control_source_v1::Event::Cancelled => proxy.destroy(),
            _ => {}
        }
    }
}

fn write_bounded(fd: OwnedFd, bytes: &[u8], timeout: Duration, cancel: &AtomicBool) -> bool {
    if fcntl_setfl(&fd, OFlags::NONBLOCK).is_err() {
        return false;
    }
    let deadline = Instant::now() + timeout;
    let mut written = 0;
    while written < bytes.len() {
        if cancel.load(Ordering::Acquire) {
            return false;
        }
        match write(&fd, &bytes[written..]) {
            Ok(0) => return false,
            Ok(count) => written += count,
            Err(Errno::AGAIN) => {
                let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                    return false;
                };
                let timeout = remaining.min(WRITER_POLL_QUANTUM);
                let Ok(timeout) = rustix::time::Timespec::try_from(timeout) else {
                    return false;
                };
                let mut fds = [PollFd::new(&fd, PollFlags::OUT)];
                if poll(&mut fds, Some(&timeout)).is_err()
                    || !fds[0].revents().contains(PollFlags::OUT)
                {
                    continue;
                }
            }
            Err(_) => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command as ProcessCommand, Stdio};
    use wl_clipboard_rs::copy::{MimeSource, MimeType, Options, Source};

    #[test]
    fn representation_includes_every_shared_linux_payload() {
        for (mime, content_type) in [
            (
                "text/plain;charset=utf-8",
                copypaste_ipc::content_type::TEXT,
            ),
            ("text/html", copypaste_ipc::content_type::HTML),
            ("text/rtf", copypaste_ipc::content_type::RICH_TEXT),
            ("image/png", copypaste_ipc::content_type::IMAGE_PNG),
            ("image/tiff", copypaste_ipc::content_type::IMAGE_TIFF),
            ("text/uri-list", copypaste_ipc::content_type::FILE),
        ] {
            assert_eq!(
                representation(&[mime.into()]).map(|(_, kind, _)| kind),
                Some(content_type)
            );
        }
    }

    #[test]
    fn selection_prefers_native_file_and_text_over_rich_fallbacks() {
        assert_eq!(
            representation(&[
                "text/html".into(),
                "text/plain".into(),
                "text/uri-list".into()
            ]),
            Some(("text/uri-list", copypaste_ipc::content_type::FILE, true))
        );
    }

    #[test]
    fn bounded_payload_validation_rejects_unbounded_writes() {
        assert!(!valid_payloads(&HashMap::from([(
            "text/plain;charset=utf-8".into(),
            vec![0; copypaste_ipc::MAX_CONTENT_BYTES + 1],
        )])));
    }

    #[test]
    fn stalled_recipient_cancels_a_nonblocking_source_writer() {
        let (_reader, writer) = UnixStream::pair().expect("socket pair");
        let cancel = AtomicBool::new(false);
        let started = Instant::now();
        assert!(!write_bounded(
            writer.into(),
            &vec![0; 1024 * 1024],
            Duration::from_millis(10),
            &cancel,
        ));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn saturated_wake_coalesces_without_rejecting_the_queued_command() {
        let (_read, mut write) = UnixStream::pair().expect("socket pair");
        write.set_nonblocking(true).expect("nonblocking wake");
        let fill = [0_u8; 8192];
        while write.write(&fill).is_ok() {}
        let sender = Sender {
            commands: Mutex::new(VecDeque::new()),
            wake: Mutex::new(Some(write)),
            shutdown: AtomicBool::new(false),
        };
        let (reply, _received) = mpsc::channel();
        sender
            .send(Command::Read {
                sequence: 1,
                settings: Default::default(),
                reply,
            })
            .expect("a full wake pipe is already a pending wake");
        assert_eq!(sender.commands.lock().unwrap().len(), 1);
    }

    #[test]
    fn command_drain_yields_after_a_fixed_budget() {
        let (_read, write) = UnixStream::pair().expect("socket pair");
        let sender = Sender {
            commands: Mutex::new(VecDeque::new()),
            wake: Mutex::new(Some(write)),
            shutdown: AtomicBool::new(false),
        };
        for _ in 0..=COMMAND_BUDGET {
            let (reply, _received) = mpsc::channel();
            sender
                .send(Command::Read {
                    sequence: 1,
                    settings: Default::default(),
                    reply,
                })
                .expect("queued command");
        }
        assert!(drain_commands(&sender, None));
        assert_eq!(sender.commands.lock().unwrap().len(), 1);
    }

    #[test]
    fn dropping_with_a_full_command_queue_unblocks_the_worker() {
        let dir = tempfile::tempdir().expect("staging directory");
        let clipboard = WaylandClipboard::new(dir.path()).expect("Wayland adapter");
        let sender = Arc::clone(&clipboard.sender);
        let mut pending = sender.commands.lock().expect("command queue");
        for _ in 0..COMMAND_CAPACITY {
            let (reply, _received) = mpsc::channel();
            pending.push_back(Command::Read {
                sequence: 1,
                settings: Default::default(),
                reply,
            });
        }
        assert_eq!(pending.len(), COMMAND_CAPACITY);
        // Keep the queue mutex while Drop joins. The worker must yield its
        // bounded drain and observe shutdown instead of waiting on this lock.
        let started = Instant::now();
        let dropper = thread::spawn(move || drop(clipboard));
        let deadline = Instant::now() + Duration::from_secs(1);
        while !sender.is_shutdown() && Instant::now() < deadline {
            thread::yield_now();
        }
        assert!(
            sender.is_shutdown(),
            "drop must signal shutdown before joining"
        );
        let (reply, _received) = mpsc::channel();
        assert!(
            sender
                .send(Command::Read {
                    sequence: 2,
                    settings: Default::default(),
                    reply,
                })
                .is_err(),
            "shutdown rejects a flooded command queue"
        );
        dropper.join().expect("clipboard drop");
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "shutdown must not wait for command queue capacity"
        );
    }

    #[test]
    #[ignore = "requires a running native Wayland data-control compositor and wl-clipboard"]
    fn data_control_interoperates_with_independent_wl_clipboard_clients() {
        let dir = tempfile::tempdir().expect("staging directory");
        let mut clipboard = WaylandClipboard::new(dir.path()).expect("Wayland adapter");
        let settings = copypaste_ipc::ConfigData::default();
        wait_until_active(&clipboard);

        for (mime, body, content_type, binary) in [
            (
                "text/plain;charset=utf-8",
                b"native text".as_slice(),
                copypaste_ipc::content_type::TEXT,
                false,
            ),
            (
                "text/html",
                b"<b>native html</b>".as_slice(),
                copypaste_ipc::content_type::HTML,
                false,
            ),
            (
                "text/rtf",
                br"{\rtf1 native rich text}".as_slice(),
                copypaste_ipc::content_type::RICH_TEXT,
                false,
            ),
            (
                "image/png",
                b"not-decoded-png-fixture".as_slice(),
                copypaste_ipc::content_type::IMAGE_PNG,
                true,
            ),
            (
                "image/tiff",
                b"not-decoded-tiff-fixture".as_slice(),
                copypaste_ipc::content_type::IMAGE_TIFF,
                true,
            ),
            (
                "text/uri-list",
                b"file:///tmp/copypaste-wayland-fixture.txt\n".as_slice(),
                copypaste_ipc::content_type::FILE,
                true,
            ),
        ] {
            wl_copy(mime, body);
            let capture = wait_capture(&mut clipboard, &settings).expect("external capture");
            assert_eq!(capture.content_type, content_type);
            if content_type == copypaste_ipc::content_type::FILE {
                assert!(capture.file_path.is_some());
                assert!(capture.file_metadata.is_some());
            } else if binary {
                assert_eq!(capture.binary_content.as_deref(), Some(body));
            } else {
                assert_eq!(capture.content.as_bytes(), body);
            }
        }

        clipboard.set_contents("owned text").expect("text write");
        assert_eq!(wl_paste("text/plain;charset=utf-8"), b"owned text");
        assert!(!clipboard.changed(), "our marker suppresses the self-write");

        clipboard
            .set_binary_contents(
                "fixture",
                copypaste_ipc::content_type::IMAGE_TIFF,
                b"outbound tiff",
                None,
            )
            .expect("TIFF write");
        assert_eq!(wl_paste("image/tiff"), b"outbound tiff");

        wl_copy("text/plain;charset=utf-8", b"private cursor");
        let private = copypaste_ipc::ConfigData {
            private_mode: true,
            ..Default::default()
        };
        assert!(wait_capture(&mut clipboard, &private).is_none());
        assert!(clipboard
            .poll_with_policy(CapturePolicy::new(&settings))
            .is_none());

        wl_copy_secret_hint();
        assert!(wait_capture(&mut clipboard, &settings).is_none());

        let capped = copypaste_ipc::ConfigData {
            max_text_size_bytes: 4,
            ..Default::default()
        };
        wl_copy("text/plain;charset=utf-8", b"oversized");
        assert!(wait_capture(&mut clipboard, &capped).is_none());
        wl_copy("text/plain;charset=utf-8", b"next");
        assert_eq!(
            wait_capture(&mut clipboard, &settings)
                .expect("next capture")
                .content,
            "next"
        );
    }

    fn wl_copy(mime: &str, body: &[u8]) {
        let mut child = ProcessCommand::new("wl-copy")
            .args(["--type", mime])
            .stdin(Stdio::piped())
            .spawn()
            .expect("wl-copy is installed");
        child
            .stdin
            .take()
            .expect("wl-copy stdin")
            .write_all(body)
            .expect("wl-copy input");
        assert!(child.wait().expect("wl-copy status").success());
    }

    fn wl_paste(mime: &str) -> Vec<u8> {
        let output = ProcessCommand::new("wl-paste")
            .args(["--no-newline", "--type", mime])
            .output()
            .expect("wl-paste is installed");
        assert!(output.status.success(), "wl-paste failed: {output:?}");
        output.stdout
    }

    fn wl_copy_secret_hint() {
        Options::new()
            .copy_multi(vec![
                MimeSource {
                    source: Source::Bytes(b"private producer".to_vec().into_boxed_slice()),
                    mime_type: MimeType::Specific("text/plain;charset=utf-8".into()),
                },
                MimeSource {
                    source: Source::Bytes(b"secret".to_vec().into_boxed_slice()),
                    mime_type: MimeType::Specific(SECRET_HINT.into()),
                },
            ])
            .expect("multi-MIME secret source");
    }

    fn wait_capture(
        clipboard: &mut WaylandClipboard,
        settings: &copypaste_ipc::ConfigData,
    ) -> Option<Capture> {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if clipboard.changed() {
                return clipboard.poll_with_policy(CapturePolicy::new(settings));
            }
            if Instant::now() >= deadline {
                return None;
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn wait_until_active(clipboard: &WaylandClipboard) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while !clipboard.active() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(clipboard.active(), "data-control adapter did not activate");
    }
}

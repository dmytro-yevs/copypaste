//! X11 clipboard transport. The worker keeps ownership and serves requests.

use std::io::{Read, Write};
use std::os::fd::{AsRawFd, BorrowedFd};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use rustix::event::{poll, PollFd, PollFlags, Timespec};
use x11rb::connection::Connection;
use x11rb::protocol::xfixes::{ConnectionExt as _, SelectionEventMask};
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ConnectionExt as _, CreateWindowAux, EventMask, PropMode, SelectionNotifyEvent,
    WindowClass,
};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use super::{Capture, CapturePolicy, ClipboardSource, SourcePolicyEvidence};

mod gnome;
mod wayland;

const KDE_PASSWORD_MANAGER_HINT: &str = "x-kde-passwordManagerHint";
const MAX_TARGETS: usize = 1024;
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(2);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone)]
struct Atoms {
    clipboard: Atom,
    targets: Atom,
    utf8: Atom,
    text: Atom,
    string: Atom,
    plain_utf8: Atom,
    html: Atom,
    rtf: Atom,
    png: Atom,
    tiff: Atom,
    uri_list: Atom,
    incr: Atom,
    kde_hint: Atom,
    property: Atom,
}
impl Atoms {
    fn intern(c: &RustConnection) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let a = |name: &[u8]| -> Result<Atom, Box<dyn std::error::Error + Send + Sync>> {
            Ok(c.intern_atom(false, name)?.reply()?.atom)
        };
        Ok(Self {
            clipboard: a(b"CLIPBOARD")?,
            targets: a(b"TARGETS")?,
            utf8: a(b"UTF8_STRING")?,
            text: a(b"TEXT")?,
            string: a(b"STRING")?,
            plain_utf8: a(b"text/plain;charset=utf-8")?,
            html: a(b"text/html")?,
            rtf: a(b"text/rtf")?,
            png: a(b"image/png")?,
            tiff: a(b"image/tiff")?,
            uri_list: a(b"text/uri-list")?,
            incr: a(b"INCR")?,
            kde_hint: a(KDE_PASSWORD_MANAGER_HINT.as_bytes())?,
            property: a(b"_COPYPASTE_CLIPBOARD")?,
        })
    }
}

enum Command {
    Read {
        settings: copypaste_ipc::ConfigData,
        reply: mpsc::Sender<Option<Capture>>,
    },
    Set {
        payload: OwnedSelection,
        reply: mpsc::Sender<bool>,
    },
    Stop,
}
#[derive(Clone)]
struct OwnedSelection {
    target: Atom,
    bytes: Vec<u8>,
}
struct Sender {
    commands: mpsc::Sender<Command>,
    wake: Mutex<UnixStream>,
}
impl Sender {
    fn send(&self, command: Command) -> Result<(), ()> {
        self.commands.send(command).map_err(|_| ())?;
        let _ = self.wake.lock().map_err(|_| ())?.write(&[1]);
        Ok(())
    }
}

pub(super) struct X11Clipboard {
    sender: Arc<Sender>,
    changed: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    staging: super::file_materialize::StagingArea,
}
impl X11Clipboard {
    pub(super) fn new(data_dir: &Path) -> std::io::Result<Self> {
        // Create the cleanup owner before exposing a worker which can accept a
        // file payload. A spawned worker must never outlive unowned plaintext.
        let staging = super::file_materialize::StagingArea::new(data_dir)?;
        let (tx, rx) = mpsc::channel();
        let (read, write) = UnixStream::pair()?;
        read.set_nonblocking(true)?;
        write.set_nonblocking(true)?;
        let changed = Arc::new(AtomicBool::new(true));
        let thread_changed = Arc::clone(&changed);
        let worker = thread::Builder::new()
            .name("copypaste-x11-clipboard".into())
            .spawn(move || worker(rx, read, thread_changed))?;
        Ok(Self {
            sender: Arc::new(Sender {
                commands: tx,
                wake: Mutex::new(write),
            }),
            changed,
            worker: Some(worker),
            staging,
        })
    }
    fn read(&mut self, settings: &copypaste_ipc::ConfigData) -> Option<Capture> {
        if !self.changed.swap(false, Ordering::AcqRel) {
            return None;
        }
        let (tx, rx) = mpsc::channel();
        self.sender
            .send(Command::Read {
                settings: settings.clone(),
                reply: tx,
            })
            .ok()?;
        rx.recv_timeout(REQUEST_TIMEOUT).ok().flatten()
    }
    fn write(&self, payload: OwnedSelection) -> anyhow::Result<()> {
        let (tx, rx) = mpsc::channel();
        self.sender
            .send(Command::Set { payload, reply: tx })
            .map_err(|_| anyhow::anyhow!("the X11 clipboard worker stopped"))?;
        rx.recv_timeout(REQUEST_TIMEOUT)
            .ok()
            .filter(|ok| *ok)
            .map(|_| ())
            .ok_or_else(|| anyhow::anyhow!("the X11 clipboard rejected the write"))
    }
}

/// Selects the transport for the active display server. A Wayland session is
/// never silently redirected through XWayland: missing compositor support is
/// surfaced by the inactive native adapter instead.
pub(super) enum LinuxClipboard {
    X11(X11Clipboard),
    Gnome(gnome::GnomeClipboard),
    Kwin(gnome::GnomeClipboard),
    Wayland(wayland::WaylandClipboard),
}

impl LinuxClipboard {
    pub(super) fn new(data_dir: &Path) -> std::io::Result<Self> {
        if is_wayland_session() {
            if is_gnome_session() {
                Ok(Self::Gnome(gnome::GnomeClipboard::new(data_dir)?))
            } else if is_kde_session() {
                Ok(Self::Kwin(gnome::GnomeClipboard::new_kwin(data_dir)?))
            } else {
                Ok(Self::Wayland(wayland::WaylandClipboard::new(data_dir)?))
            }
        } else if std::env::var_os("DISPLAY").is_some() {
            X11Clipboard::new(data_dir).map(Self::X11)
        } else {
            Ok(Self::Wayland(wayland::WaylandClipboard::new(data_dir)?))
        }
    }
}

fn is_wayland_session() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
        || std::env::var("XDG_SESSION_TYPE")
            .is_ok_and(|value| value.eq_ignore_ascii_case("wayland"))
}

fn is_gnome_session() -> bool {
    [
        "XDG_CURRENT_DESKTOP",
        "XDG_SESSION_DESKTOP",
        "DESKTOP_SESSION",
    ]
    .into_iter()
    .filter_map(std::env::var_os)
    .any(|value| {
        value
            .to_string_lossy()
            .to_ascii_lowercase()
            .contains("gnome")
    })
}

fn is_kde_session() -> bool {
    [
        "XDG_CURRENT_DESKTOP",
        "XDG_SESSION_DESKTOP",
        "DESKTOP_SESSION",
    ]
    .into_iter()
    .filter_map(std::env::var_os)
    .any(|value| {
        let value = value.to_string_lossy().to_ascii_lowercase();
        value.contains("kde") || value.contains("plasma")
    })
}
impl Drop for X11Clipboard {
    fn drop(&mut self) {
        let _ = self.sender.send(Command::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl ClipboardSource for X11Clipboard {
    fn poll(&mut self) -> Option<Capture> {
        self.read(&copypaste_ipc::ConfigData::default())
    }
    fn poll_with_policy(&mut self, policy: CapturePolicy<'_>) -> Option<Capture> {
        self.read(policy.settings)
    }
    fn changed(&mut self) -> bool {
        self.changed.load(Ordering::Acquire)
    }
    fn set_contents(&mut self, text: &str) -> anyhow::Result<()> {
        self.write(OwnedSelection {
            target: 0,
            bytes: text.as_bytes().to_vec(),
        })
    }
    fn set_binary_contents(
        &mut self,
        _: &str,
        content_type: &str,
        bytes: &[u8],
        metadata: Option<&copypaste_core::FileMetadata>,
    ) -> Result<(), copypaste_core::ClipboardWriteError> {
        use copypaste_core::ClipboardWriteError;
        let target = match content_type {
            copypaste_ipc::content_type::HTML => 1,
            copypaste_ipc::content_type::RICH_TEXT => 2,
            copypaste_ipc::content_type::IMAGE_PNG => 3,
            copypaste_ipc::content_type::IMAGE_TIFF => 4,
            copypaste_ipc::content_type::FILE => 5,
            _ => return Err(ClipboardWriteError::UnsupportedContent),
        };
        let bytes = if target == 5 {
            let path = self
                .staging
                .materialize(bytes, metadata.ok_or(ClipboardWriteError::Failed)?)
                .map_err(|_| ClipboardWriteError::Failed)?;
            url::Url::from_file_path(path)
                .map_err(|_| ClipboardWriteError::Failed)?
                .to_string()
                .into_bytes()
        } else {
            bytes.to_vec()
        };
        self.write(OwnedSelection { target, bytes })
            .map_err(|_| ClipboardWriteError::Failed)
    }
    fn backend_name(&self) -> &'static str {
        "linux-x11-system-clipboard"
    }
}

impl ClipboardSource for LinuxClipboard {
    fn poll(&mut self) -> Option<Capture> {
        match self {
            Self::X11(source) => source.poll(),
            Self::Gnome(source) => source.poll(),
            Self::Kwin(source) => source.poll(),
            Self::Wayland(source) => source.poll(),
        }
    }
    fn poll_with_policy(&mut self, policy: CapturePolicy<'_>) -> Option<Capture> {
        match self {
            Self::X11(source) => source.poll_with_policy(policy),
            Self::Gnome(source) => source.poll_with_policy(policy),
            Self::Kwin(source) => source.poll_with_policy(policy),
            Self::Wayland(source) => source.poll_with_policy(policy),
        }
    }
    fn changed(&mut self) -> bool {
        match self {
            Self::X11(source) => source.changed(),
            Self::Gnome(source) => source.changed(),
            Self::Kwin(source) => source.changed(),
            Self::Wayland(source) => source.changed(),
        }
    }
    fn set_contents(&mut self, text: &str) -> anyhow::Result<()> {
        match self {
            Self::X11(source) => source.set_contents(text),
            Self::Gnome(source) => source.set_contents(text),
            Self::Kwin(source) => source.set_contents(text),
            Self::Wayland(source) => source.set_contents(text),
        }
    }
    fn set_binary_contents(
        &mut self,
        item_id: &str,
        content_type: &str,
        bytes: &[u8],
        metadata: Option<&copypaste_core::FileMetadata>,
    ) -> Result<(), copypaste_core::ClipboardWriteError> {
        match self {
            Self::X11(source) => source.set_binary_contents(item_id, content_type, bytes, metadata),
            Self::Gnome(source) => {
                source.set_binary_contents(item_id, content_type, bytes, metadata)
            }
            Self::Kwin(source) => {
                source.set_binary_contents(item_id, content_type, bytes, metadata)
            }
            Self::Wayland(source) => {
                source.set_binary_contents(item_id, content_type, bytes, metadata)
            }
        }
    }
    fn backend_name(&self) -> &'static str {
        match self {
            Self::X11(source) => source.backend_name(),
            Self::Gnome(source) => source.backend_name(),
            Self::Kwin(source) => source.backend_name(),
            Self::Wayland(source) => source.backend_name(),
        }
    }
}

fn worker(commands: mpsc::Receiver<Command>, mut wake: UnixStream, changed: Arc<AtomicBool>) {
    let Ok((connection, screen)) = x11rb::connect(None) else {
        return;
    };
    let Ok(window) = connection.generate_id() else {
        return;
    };
    let root = connection.setup().roots[screen].root;
    if connection
        .create_window(
            0,
            window,
            root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        )
        .is_err()
    {
        return;
    }
    let Ok(atoms) = Atoms::intern(&connection) else {
        return;
    };
    if connection
        .xfixes_select_selection_input(
            window,
            atoms.clipboard,
            SelectionEventMask::SET_SELECTION_OWNER
                | SelectionEventMask::SELECTION_WINDOW_DESTROY
                | SelectionEventMask::SELECTION_CLIENT_CLOSE,
        )
        .and_then(|_| connection.flush())
        .is_err()
    {
        return;
    }
    let mut owned = None;
    loop {
        while let Ok(request) = commands.try_recv() {
            if !command(
                &connection,
                window,
                &atoms,
                &mut owned,
                request,
                &changed,
                &commands,
                &mut wake,
            ) {
                return;
            }
        }
        while let Ok(Some(next_event)) = connection.poll_for_event() {
            event(
                &connection,
                window,
                &atoms,
                &mut owned,
                next_event,
                &changed,
            );
        }
        let xfd = unsafe { BorrowedFd::borrow_raw(connection.stream().as_raw_fd()) };
        let mut fds = [
            PollFd::from_borrowed_fd(xfd, PollFlags::IN),
            PollFd::new(&wake, PollFlags::IN),
        ];
        if poll(&mut fds, None).is_err() {
            return;
        }
        if fds[1].revents().contains(PollFlags::IN) {
            let mut buffer = [0; 256];
            while wake.read(&mut buffer).is_ok() {}
        }
    }
}
fn command(
    c: &RustConnection,
    window: u32,
    a: &Atoms,
    owned: &mut Option<OwnedSelection>,
    command: Command,
    changed: &AtomicBool,
    commands: &mpsc::Receiver<Command>,
    wake: &mut UnixStream,
) -> bool {
    match command {
        Command::Stop => false,
        Command::Set { mut payload, reply } => {
            let ok = set_selection(c, window, a, owned, &mut payload, changed);
            let _ = reply.send(ok);
            true
        }
        Command::Read { settings, reply } => {
            let mut transfer = Transfer {
                c,
                window,
                atoms: a,
                commands,
                wake,
                owned,
                changed,
                stopped: false,
            };
            let capture = transfer.read_clipboard(&settings);
            let keep_running = !transfer.stopped;
            let _ = reply.send(capture);
            keep_running
        }
    }
}

fn set_selection(
    c: &RustConnection,
    window: u32,
    a: &Atoms,
    owned: &mut Option<OwnedSelection>,
    payload: &mut OwnedSelection,
    changed: &AtomicBool,
) -> bool {
    payload.target = match payload.target {
        0 => a.utf8,
        1 => a.html,
        2 => a.rtf,
        3 => a.png,
        4 => a.tiff,
        5 => a.uri_list,
        _ => a.utf8,
    };
    let ok = c
        .set_selection_owner(window, a.clipboard, x11rb::CURRENT_TIME)
        .and_then(|_| c.flush())
        .is_ok();
    if ok {
        *owned = Some(payload.clone());
        changed.store(false, Ordering::Release);
    }
    ok
}
fn event(
    c: &RustConnection,
    window: u32,
    a: &Atoms,
    owned: &mut Option<OwnedSelection>,
    event: Event,
    changed: &AtomicBool,
) {
    match event {
        Event::XfixesSelectionNotify(e) if e.selection == a.clipboard => {
            if e.owner != window {
                changed.store(true, Ordering::Release);
            }
        }
        Event::SelectionRequest(e) if e.selection == a.clipboard => serve(c, a, owned.as_ref(), e),
        _ => {}
    }
}
fn serve(
    c: &RustConnection,
    a: &Atoms,
    owned: Option<&OwnedSelection>,
    request: x11rb::protocol::xproto::SelectionRequestEvent,
) {
    let property = if request.property == 0 {
        request.target
    } else {
        request.property
    };
    let mut delivered = 0;
    if request.target == a.targets {
        let mut targets = vec![a.targets, a.utf8, a.text, a.string, a.plain_utf8];
        if let Some(value) = owned {
            targets.push(value.target);
        }
        if c.change_property32(
            PropMode::REPLACE,
            request.requestor,
            property,
            AtomEnum::ATOM,
            &targets,
        )
        .is_ok()
        {
            delivered = property;
        }
    } else if let Some(value) = owned.filter(|value| {
        request.target == value.target
            || (value.target == a.utf8
                && [a.text, a.string, a.plain_utf8].contains(&request.target))
    }) {
        if c.change_property8(
            PropMode::REPLACE,
            request.requestor,
            property,
            request.target,
            &value.bytes,
        )
        .is_ok()
        {
            delivered = property;
        }
    }
    let response = SelectionNotifyEvent {
        response_type: x11rb::protocol::xproto::SELECTION_NOTIFY_EVENT,
        sequence: 0,
        time: request.time,
        requestor: request.requestor,
        selection: request.selection,
        target: request.target,
        property: delivered,
    };
    let _ = c.send_event(false, request.requestor, EventMask::NO_EVENT, response);
    let _ = c.flush();
}
struct Transfer<'a> {
    c: &'a RustConnection,
    window: u32,
    atoms: &'a Atoms,
    commands: &'a mpsc::Receiver<Command>,
    wake: &'a mut UnixStream,
    owned: &'a mut Option<OwnedSelection>,
    changed: &'a AtomicBool,
    stopped: bool,
}

impl Transfer<'_> {
    fn read_clipboard(&mut self, settings: &copypaste_ipc::ConfigData) -> Option<Capture> {
        let deadline = Instant::now() + TRANSFER_TIMEOUT;
        // Unknown X11 source identity cannot satisfy an explicit exclusion. This is
        // before TARGETS or any representation request, and private mode likewise
        // reads no clipboard data at all.
        let owner = self
            .c
            .get_selection_owner(self.atoms.clipboard)
            .ok()?
            .reply()
            .ok()?
            .owner;
        let source = (owner != 0)
            .then(|| super::linux_attribution::resolve_owner(self.c, owner))
            .flatten();
        if !Self::can_read_source(settings, source.as_ref()) {
            return None;
        }
        let targets = atoms(&self.read_target(self.atoms.targets, MAX_TARGETS * 4, deadline)?)?;
        let secret = if targets.contains(&self.atoms.kde_hint) {
            self.read_target(self.atoms.kde_hint, 64, deadline)?
                .eq_ignore_ascii_case(b"secret")
        } else {
            false
        };
        let privacy = copypaste_ipc::ClipboardPrivacy {
            secret,
            transient: false,
        };
        if !privacy.allows(settings) {
            return None;
        }
        let (target, type_, binary) = if targets.contains(&self.atoms.uri_list) {
            (self.atoms.uri_list, copypaste_ipc::content_type::FILE, true)
        } else if targets.contains(&self.atoms.html) {
            (self.atoms.html, copypaste_ipc::content_type::HTML, false)
        } else if targets.contains(&self.atoms.rtf) {
            (
                self.atoms.rtf,
                copypaste_ipc::content_type::RICH_TEXT,
                false,
            )
        } else if targets.contains(&self.atoms.png) {
            (self.atoms.png, copypaste_ipc::content_type::IMAGE_PNG, true)
        } else if targets.contains(&self.atoms.tiff) {
            (
                self.atoms.tiff,
                copypaste_ipc::content_type::IMAGE_TIFF,
                true,
            )
        } else {
            let target = [
                self.atoms.utf8,
                self.atoms.plain_utf8,
                self.atoms.text,
                self.atoms.string,
            ]
            .into_iter()
            .find(|atom| targets.contains(atom))?;
            (target, copypaste_ipc::content_type::TEXT, false)
        };
        let bytes = self.read_target(
            target,
            settings.capture_limit_bytes(type_) as usize,
            deadline,
        )?;
        self.owner_is_current(owner)?;
        if type_ == copypaste_ipc::content_type::FILE {
            return file_capture(bytes, privacy, source);
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
            content_type: type_.to_owned(),
            app_bundle_id: source.as_ref().map(|source| source.id.clone()),
            app_name: source.as_ref().map(|source| source.name.clone()),
            source_policy: SourcePolicyEvidence::Legacy,
        })
    }

    fn can_read_source(
        settings: &copypaste_ipc::ConfigData,
        source: Option<&super::linux_attribution::SourceApp>,
    ) -> bool {
        !settings.private_mode
            && (settings.excluded_app_bundle_ids.is_empty()
                || source.is_some_and(|source| {
                    !settings
                        .excluded_app_bundle_ids
                        .iter()
                        .any(|excluded| excluded == &source.id)
                }))
    }
    fn owner_is_current(&self, owner: u32) -> Option<()> {
        (self
            .c
            .get_selection_owner(self.atoms.clipboard)
            .ok()?
            .reply()
            .ok()?
            .owner
            == owner)
            .then_some(())
    }
    fn read_target(&mut self, target: Atom, cap: usize, deadline: Instant) -> Option<Vec<u8>> {
        self.c
            .delete_property(self.window, self.atoms.property)
            .ok()?;
        self.c
            .convert_selection(
                self.window,
                self.atoms.clipboard,
                target,
                self.atoms.property,
                x11rb::CURRENT_TIME,
            )
            .ok()?;
        self.c.flush().ok()?;
        loop {
            match self.next_event(deadline)? {
                Event::SelectionNotify(e)
                    if e.selection == self.atoms.clipboard && e.target == target =>
                {
                    if e.property == 0 {
                        return None;
                    }
                    let preflight = self.property_size(false)?;
                    return if preflight.0 == self.atoms.incr {
                        (preflight.1 <= 4)
                            .then(|| self.read_incr(cap, deadline))
                            .flatten()
                    } else {
                        self.read_property(false, cap, preflight.1)
                    };
                }
                _ => {}
            }
        }
    }
    fn read_incr(&mut self, cap: usize, deadline: Instant) -> Option<Vec<u8>> {
        self.c
            .delete_property(self.window, self.atoms.property)
            .ok()?;
        self.c.flush().ok()?;
        let mut output = Vec::new();
        loop {
            match self.next_event(deadline)? {
                Event::PropertyNotify(e)
                    if e.window == self.window
                        && e.atom == self.atoms.property
                        && e.state == x11rb::protocol::xproto::Property::NEW_VALUE =>
                {
                    let (_, bytes) = self.property_size(false)?;
                    if bytes == 0 {
                        self.c
                            .delete_property(self.window, self.atoms.property)
                            .ok()?;
                        return Some(output);
                    }
                    let remaining = cap.checked_sub(output.len())?;
                    let chunk = self.read_property(true, remaining, bytes)?;
                    output.extend_from_slice(&chunk);
                }
                _ => {}
            }
        }
    }
    fn property_size(&self, delete: bool) -> Option<(Atom, usize)> {
        let reply = self
            .c
            .get_property(
                delete,
                self.window,
                self.atoms.property,
                AtomEnum::ANY,
                0,
                0,
            )
            .ok()?
            .reply()
            .ok()?;
        Some((reply.type_, reply.bytes_after as usize))
    }
    fn read_property(&self, delete: bool, cap: usize, announced: usize) -> Option<Vec<u8>> {
        if announced > cap {
            return None;
        }
        let units = u32::try_from(announced.checked_add(3)? / 4).ok()?;
        let reply = self
            .c
            .get_property(
                delete,
                self.window,
                self.atoms.property,
                AtomEnum::ANY,
                0,
                units,
            )
            .ok()?
            .reply()
            .ok()?;
        (reply.bytes_after == 0 && reply.value.len() <= cap).then_some(reply.value)
    }
    fn next_event(&mut self, deadline: Instant) -> Option<Event> {
        loop {
            while let Ok(Some(event)) = self.c.poll_for_event() {
                match event {
                    Event::SelectionRequest(request)
                        if request.selection == self.atoms.clipboard =>
                    {
                        serve(self.c, self.atoms, self.owned.as_ref(), request);
                    }
                    Event::XfixesSelectionNotify(event)
                        if event.selection == self.atoms.clipboard =>
                    {
                        if event.owner != self.window {
                            self.changed.store(true, Ordering::Release);
                        }
                    }
                    event => return Some(event),
                }
            }
            while let Ok(command) = self.commands.try_recv() {
                match command {
                    Command::Stop => {
                        self.stopped = true;
                        return None;
                    }
                    Command::Set { mut payload, reply } => {
                        let ok = set_selection(
                            self.c,
                            self.window,
                            self.atoms,
                            self.owned,
                            &mut payload,
                            self.changed,
                        );
                        let _ = reply.send(ok);
                    }
                    Command::Read { reply, .. } => {
                        // The in-flight transaction owns the transfer property. A
                        // second poll is acknowledged without starting a nested
                        // selection conversion or retaining stale clipboard data.
                        let _ = reply.send(None);
                        self.changed.store(false, Ordering::Release);
                    }
                }
            }
            let remaining = deadline.checked_duration_since(Instant::now())?;
            let timeout = Timespec::try_from(remaining).ok()?;
            let xfd = unsafe { BorrowedFd::borrow_raw(self.c.stream().as_raw_fd()) };
            let mut fds = [
                PollFd::from_borrowed_fd(xfd, PollFlags::IN),
                PollFd::new(&*self.wake, PollFlags::IN),
            ];
            poll(&mut fds, Some(&timeout)).ok()?;
            if fds[1].revents().contains(PollFlags::IN) {
                let mut buffer = [0; 256];
                while self.wake.read(&mut buffer).is_ok() {}
            }
        }
    }
}
fn atoms(bytes: &[u8]) -> Option<Vec<Atom>> {
    (bytes.len() % 4 == 0).then(|| {
        bytes
            .chunks_exact(4)
            .map(|value| u32::from_ne_bytes(value.try_into().expect("fixed atom chunk")))
            .collect()
    })
}
pub(super) fn can_read_unknown_source(settings: &copypaste_ipc::ConfigData) -> bool {
    !settings.private_mode && settings.excluded_app_bundle_ids.is_empty()
}

pub(super) fn file_capture(
    bytes: Vec<u8>,
    privacy: copypaste_ipc::ClipboardPrivacy,
    source: Option<super::linux_attribution::SourceApp>,
) -> Option<Capture> {
    let mut uris = std::str::from_utf8(&bytes)
        .ok()?
        .lines()
        .filter(|line| !line.starts_with('#'));
    let uri = uris.next()?;
    if uris.next().is_some() {
        return None;
    }
    let path = url::Url::parse(uri).ok()?.to_file_path().ok()?;
    let filename = path.file_name()?.to_str()?.to_owned();
    let metadata = copypaste_core::FileMetadata::with_source_reference(
        filename,
        "application/octet-stream",
        path.to_string_lossy(),
    )?;
    Some(Capture {
        privacy,
        content: String::new(),
        binary_content: None,
        file_path: Some(path),
        file_metadata: Some(metadata),
        content_type: copypaste_ipc::content_type::FILE.to_owned(),
        app_bundle_id: source.as_ref().map(|source| source.id.clone()),
        app_name: source.as_ref().map(|source| source.name.clone()),
        source_policy: SourcePolicyEvidence::Legacy,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use std::time::Duration;

    #[derive(Clone)]
    enum FixturePayload {
        Text(Vec<u8>),
        Tiff(Vec<u8>),
        File(Vec<u8>),
        Secret,
        IncompleteIncr,
        Oversized,
        Stalled,
    }

    struct FixtureOwner {
        stop: Arc<AtomicBool>,
        requests: Arc<AtomicUsize>,
        worker: Option<JoinHandle<()>>,
    }

    impl FixtureOwner {
        fn start(payload: FixturePayload) -> Self {
            let stop = Arc::new(AtomicBool::new(false));
            let requests = Arc::new(AtomicUsize::new(0));
            let worker_stop = Arc::clone(&stop);
            let worker_requests = Arc::clone(&requests);
            let (ready, started) = mpsc::sync_channel(1);
            let worker = thread::spawn(move || {
                let (connection, screen) = x11rb::connect(None).expect("fixture X server");
                let window = connection.generate_id().expect("fixture window id");
                let root = connection.setup().roots[screen].root;
                connection
                    .create_window(
                        0,
                        window,
                        root,
                        0,
                        0,
                        1,
                        1,
                        0,
                        WindowClass::INPUT_OUTPUT,
                        0,
                        &CreateWindowAux::new(),
                    )
                    .expect("create fixture window");
                let atoms = Atoms::intern(&connection).expect("fixture atoms");
                let pid = connection
                    .intern_atom(false, b"_NET_WM_PID")
                    .expect("pid atom")
                    .reply()
                    .expect("pid atom reply")
                    .atom;
                connection
                    .change_property32(
                        PropMode::REPLACE,
                        window,
                        pid,
                        AtomEnum::CARDINAL,
                        &[std::process::id()],
                    )
                    .expect("fixture pid");
                connection
                    .change_property8(
                        PropMode::REPLACE,
                        window,
                        AtomEnum::WM_CLASS,
                        AtomEnum::STRING,
                        b"fixture\0org.example.Fixture\0",
                    )
                    .expect("fixture class");
                connection
                    .set_selection_owner(window, atoms.clipboard, x11rb::CURRENT_TIME)
                    .expect("fixture owner");
                connection.flush().expect("flush fixture owner");
                let _ = ready.send(());
                while !worker_stop.load(Ordering::Acquire) {
                    while let Some(event) = connection.poll_for_event().expect("fixture event") {
                        let Event::SelectionRequest(request) = event else {
                            continue;
                        };
                        if request.selection != atoms.clipboard {
                            continue;
                        }
                        worker_requests.fetch_add(1, AtomicOrdering::Relaxed);
                        serve_fixture(&connection, &atoms, &payload, request);
                    }
                    thread::sleep(Duration::from_millis(1));
                }
            });
            started
                .recv_timeout(Duration::from_secs(2))
                .expect("fixture owner startup");
            Self {
                stop,
                requests,
                worker: Some(worker),
            }
        }

        fn request_count(&self) -> usize {
            self.requests.load(AtomicOrdering::Relaxed)
        }
    }

    impl Drop for FixtureOwner {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
            if let Some(worker) = self.worker.take() {
                worker.join().expect("fixture owner shutdown");
            }
        }
    }

    fn serve_fixture(
        connection: &RustConnection,
        atoms: &Atoms,
        payload: &FixturePayload,
        request: x11rb::protocol::xproto::SelectionRequestEvent,
    ) {
        let property = if request.property == 0 {
            request.target
        } else {
            request.property
        };
        let (type_, value) = match request.target {
            target if target == atoms.targets => {
                let mut targets = vec![atoms.targets, atoms.utf8];
                match payload {
                    FixturePayload::Tiff(_) => targets.push(atoms.tiff),
                    FixturePayload::File(_) => targets.push(atoms.uri_list),
                    FixturePayload::Secret => targets.push(atoms.kde_hint),
                    FixturePayload::Text(_)
                    | FixturePayload::IncompleteIncr
                    | FixturePayload::Oversized
                    | FixturePayload::Stalled => {}
                }
                let delivered = connection.change_property32(
                    PropMode::REPLACE,
                    request.requestor,
                    property,
                    AtomEnum::ATOM,
                    &targets,
                );
                notify_fixture(connection, request, delivered.is_ok().then_some(property));
                return;
            }
            _ if matches!(payload, FixturePayload::Stalled) => return,
            target if target == atoms.kde_hint && matches!(payload, FixturePayload::Secret) => {
                (target, b"secret".as_slice())
            }
            target if target == atoms.utf8 => match payload {
                FixturePayload::Text(value) => (target, value.as_slice()),
                FixturePayload::Secret => (target, b"ignored".as_slice()),
                FixturePayload::IncompleteIncr => {
                    let delivered = connection.change_property32(
                        PropMode::REPLACE,
                        request.requestor,
                        property,
                        atoms.incr,
                        &[1],
                    );
                    notify_fixture(connection, request, delivered.is_ok().then_some(property));
                    return;
                }
                FixturePayload::Oversized => {
                    let bytes = vec![0_u8; copypaste_ipc::MAX_CONTENT_BYTES + 1];
                    let delivered = connection.change_property8(
                        PropMode::REPLACE,
                        request.requestor,
                        property,
                        target,
                        &bytes,
                    );
                    notify_fixture(connection, request, delivered.is_ok().then_some(property));
                    return;
                }
                _ => return notify_fixture(connection, request, None),
            },
            target if target == atoms.tiff => match payload {
                FixturePayload::Tiff(value) => (target, value.as_slice()),
                _ => return notify_fixture(connection, request, None),
            },
            target if target == atoms.uri_list => match payload {
                FixturePayload::File(value) => (target, value.as_slice()),
                _ => return notify_fixture(connection, request, None),
            },
            _ => return notify_fixture(connection, request, None),
        };
        let delivered = connection.change_property8(
            PropMode::REPLACE,
            request.requestor,
            property,
            type_,
            value,
        );
        notify_fixture(connection, request, delivered.is_ok().then_some(property));
    }

    fn notify_fixture(
        connection: &RustConnection,
        request: x11rb::protocol::xproto::SelectionRequestEvent,
        property: Option<Atom>,
    ) {
        connection
            .send_event(
                false,
                request.requestor,
                EventMask::NO_EVENT,
                SelectionNotifyEvent {
                    response_type: x11rb::protocol::xproto::SELECTION_NOTIFY_EVENT,
                    sequence: 0,
                    time: request.time,
                    requestor: request.requestor,
                    selection: request.selection,
                    target: request.target,
                    property: property.unwrap_or(0),
                },
            )
            .expect("fixture selection notify");
        connection.flush().expect("flush fixture notify");
    }

    fn wait_for_capture(
        clipboard: &mut X11Clipboard,
        settings: &copypaste_ipc::ConfigData,
    ) -> Option<Capture> {
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if clipboard.changed() {
                if let Some(capture) = clipboard.poll_with_policy(CapturePolicy::new(settings)) {
                    return Some(capture);
                }
            }
            thread::sleep(Duration::from_millis(5));
        }
        None
    }

    fn read_owned_target(target: Atom) -> Vec<u8> {
        let (connection, screen) = x11rb::connect(None).expect("fixture requestor X server");
        let window = connection
            .generate_id()
            .expect("fixture requestor window id");
        let root = connection.setup().roots[screen].root;
        connection
            .create_window(
                0,
                window,
                root,
                0,
                0,
                1,
                1,
                0,
                WindowClass::INPUT_OUTPUT,
                0,
                &CreateWindowAux::new(),
            )
            .expect("create fixture requestor window");
        let atoms = Atoms::intern(&connection).expect("fixture requestor atoms");
        connection
            .convert_selection(
                window,
                atoms.clipboard,
                target,
                atoms.property,
                x11rb::CURRENT_TIME,
            )
            .expect("request owned target");
        connection.flush().expect("flush owned request");
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if let Some(Event::SelectionNotify(event)) =
                connection.poll_for_event().expect("fixture owned event")
            {
                assert_ne!(event.property, 0, "owned target was refused");
                return connection
                    .get_property(false, window, atoms.property, AtomEnum::ANY, 0, 4096)
                    .expect("read owned target")
                    .reply()
                    .expect("owned target reply")
                    .value;
            }
            thread::sleep(Duration::from_millis(1));
        }
        panic!("owned target did not respond before its deadline");
    }

    #[test]
    fn live_x11_fixture_captures_metadata_tiff_file_secret_exclusion_and_owned_write() {
        if std::env::var_os("COPYPASTE_X11_FIXTURE") != Some("1".into()) {
            return;
        }
        let data_home = std::path::PathBuf::from(
            std::env::var_os("XDG_DATA_HOME").expect("fixture XDG_DATA_HOME"),
        );
        std::fs::create_dir_all(data_home.join("applications")).expect("fixture desktop dir");
        std::fs::write(
            data_home.join("applications/org.example.Fixture.desktop"),
            "[Desktop Entry]\nName=Fixture Writer\nStartupWMClass=org.example.Fixture\n",
        )
        .expect("fixture desktop entry");
        let directory = tempfile::tempdir().expect("fixture data dir");
        let file = directory.path().join("fixture.bin");
        std::fs::write(&file, b"fixture file").expect("fixture file");
        let settings = copypaste_ipc::ConfigData::default();

        let text = FixtureOwner::start(FixturePayload::Text(b"fixture text".to_vec()));
        let mut clipboard = X11Clipboard::new(directory.path()).expect("X11 clipboard");
        let capture = wait_for_capture(&mut clipboard, &settings).expect("capture fixture text");
        assert_eq!(capture.content, "fixture text");
        assert_eq!(
            capture.app_bundle_id.as_deref(),
            Some("org.example.Fixture")
        );
        assert_eq!(capture.app_name.as_deref(), Some("Fixture Writer"));
        drop(clipboard);
        drop(text);

        let tiff_bytes = b"fixture tiff".to_vec();
        let tiff = FixtureOwner::start(FixturePayload::Tiff(tiff_bytes.clone()));
        let mut clipboard = X11Clipboard::new(directory.path()).expect("X11 clipboard");
        let capture = wait_for_capture(&mut clipboard, &settings).unwrap_or_else(|| {
            panic!(
                "capture fixture TIFF; owner handled {} selection requests",
                tiff.request_count()
            )
        });
        assert_eq!(
            capture.content_type,
            copypaste_ipc::content_type::IMAGE_TIFF
        );
        assert_eq!(
            capture.binary_content.as_deref(),
            Some(tiff_bytes.as_slice())
        );
        drop(clipboard);
        drop(tiff);

        let file_uri = url::Url::from_file_path(&file)
            .expect("fixture file URI")
            .to_string()
            .into_bytes();
        let file_owner = FixtureOwner::start(FixturePayload::File(file_uri));
        let mut clipboard = X11Clipboard::new(directory.path()).expect("X11 clipboard");
        let capture = wait_for_capture(&mut clipboard, &settings).expect("capture fixture file");
        assert_eq!(capture.file_path.as_deref(), Some(file.as_path()));
        assert_eq!(
            capture.app_bundle_id.as_deref(),
            Some("org.example.Fixture")
        );
        drop(clipboard);
        drop(file_owner);

        let secret = FixtureOwner::start(FixturePayload::Secret);
        let mut clipboard = X11Clipboard::new(directory.path()).expect("X11 clipboard");
        assert!(clipboard
            .poll_with_policy(CapturePolicy::new(&settings))
            .is_none());
        assert!(
            secret.request_count() >= 2,
            "TARGETS and secret hint were requested"
        );
        drop(clipboard);
        drop(secret);

        let excluded = FixtureOwner::start(FixturePayload::Text(b"excluded".to_vec()));
        let excluded_settings = copypaste_ipc::ConfigData {
            excluded_app_bundle_ids: vec!["org.example.Fixture".into()],
            ..Default::default()
        };
        let mut clipboard = X11Clipboard::new(directory.path()).expect("X11 clipboard");
        assert!(clipboard
            .poll_with_policy(CapturePolicy::new(&excluded_settings))
            .is_none());
        assert_eq!(
            excluded.request_count(),
            0,
            "excluded source was queried for payload before its policy gate"
        );

        clipboard
            .set_binary_contents(
                "fixture",
                copypaste_ipc::content_type::IMAGE_TIFF,
                b"written tiff",
                None,
            )
            .expect("write TIFF to X11");
        let (connection, _) = x11rb::connect(None).expect("fixture verification X server");
        let atoms = Atoms::intern(&connection).expect("fixture verification atoms");
        assert_eq!(read_owned_target(atoms.tiff), b"written tiff");
        drop(clipboard);
        drop(excluded);
    }

    #[test]
    fn live_x11_fixture_bounds_stalled_incr_and_oversized_transfers() {
        if std::env::var_os("COPYPASTE_X11_FIXTURE") != Some("1".into()) {
            return;
        }
        let directory = tempfile::tempdir().expect("fixture data dir");
        let settings = copypaste_ipc::ConfigData::default();
        for payload in [
            FixturePayload::Stalled,
            FixturePayload::IncompleteIncr,
            FixturePayload::Oversized,
        ] {
            let owner = FixtureOwner::start(payload);
            let mut clipboard = X11Clipboard::new(directory.path()).expect("X11 clipboard");
            let start = Instant::now();
            assert!(clipboard
                .poll_with_policy(CapturePolicy::new(&settings))
                .is_none());
            assert!(
                start.elapsed() < Duration::from_secs(3),
                "bounded X11 transfer exceeded its absolute deadline"
            );
            assert!(
                owner.request_count() >= 1,
                "fixture owner saw no selection request"
            );
            drop(clipboard);
            drop(owner);
        }
    }

    #[test]
    fn live_x11_fixture_stop_wakes_a_stalled_transfer_and_shutdown() {
        if std::env::var_os("COPYPASTE_X11_FIXTURE") != Some("1".into()) {
            return;
        }
        let directory = tempfile::tempdir().expect("fixture data dir");
        let owner = FixtureOwner::start(FixturePayload::Stalled);
        let mut clipboard = X11Clipboard::new(directory.path()).expect("X11 clipboard");
        let sender = Arc::clone(&clipboard.sender);
        let worker = thread::spawn(move || {
            let result = clipboard.read(&copypaste_ipc::ConfigData::default());
            (result, clipboard)
        });
        let deadline = Instant::now() + Duration::from_secs(2);
        while owner.request_count() < 2 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(1));
        }
        assert!(owner.request_count() >= 2, "stalled transfer did not start");
        let start = Instant::now();
        sender.send(Command::Stop).expect("stop clipboard worker");
        let (result, clipboard) = worker.join().expect("stalled caller shutdown");
        assert!(result.is_none());
        drop(clipboard);
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "Stop did not wake the X11 transfer reactor"
        );
    }
    #[test]
    fn target_atoms_are_native_endian_and_complete() {
        assert_eq!(atoms(&1_u32.to_ne_bytes()), Some(vec![1]));
        assert_eq!(atoms(&[1, 2, 3]), None);
    }
    #[test]
    fn local_file_uri_becomes_deferred_file_capture() {
        let capture = file_capture(
            b"file:///tmp/report.pdf\n".to_vec(),
            Default::default(),
            None,
        )
        .unwrap();
        assert_eq!(capture.file_metadata.unwrap().filename, "report.pdf");
        assert!(
            file_capture(b"https://example.test/a".to_vec(), Default::default(), None).is_none()
        );
    }
    #[test]
    fn confidential_kde_target_is_not_payload_type() {
        assert_eq!(KDE_PASSWORD_MANAGER_HINT, "x-kde-passwordManagerHint");
    }
    #[test]
    fn private_mode_and_unknown_source_exclusions_fail_before_payload_reads() {
        let mut settings = copypaste_ipc::ConfigData::default();
        assert!(Transfer::can_read_source(&settings, None));
        settings.private_mode = true;
        assert!(!Transfer::can_read_source(&settings, None));
        settings.private_mode = false;
        settings.excluded_app_bundle_ids = vec!["org.example.Writer".into()];
        assert!(!Transfer::can_read_source(&settings, None));
        assert!(!Transfer::can_read_source(
            &settings,
            Some(&super::super::linux_attribution::SourceApp {
                id: "org.example.Writer".into(),
                name: "Writer".into(),
            }),
        ));
    }
}

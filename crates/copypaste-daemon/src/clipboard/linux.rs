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
use x11rb::protocol::xfixes::{ConnectionExt as _, SelectionEvent, SelectionEventMask};
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
const XFIXES_SELECTION_VERSION: (u32, u32) = (2, 0);

#[derive(Clone)]
struct Atoms {
    clipboard: Atom,
    clipboard_manager: Atom,
    wm_check: Atom,
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
            clipboard_manager: a(b"CLIPBOARD_MANAGER")?,
            wm_check: a(b"_NET_SUPPORTING_WM_CHECK")?,
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

/// Ownership metadata is sufficient to keep a policy-dropped value private
/// when a clipboard manager restores it after the original writer exits.
#[derive(Default)]
struct SelectionPrivacy {
    owner: Option<u32>,
    revision: u64,
    departed_owner: Option<u32>,
    suppressed_owner: Option<u32>,
    tainted_cache: bool,
    manager_restore: bool,
}

impl SelectionPrivacy {
    fn observe(&mut self, owner: u32, departed: bool, manager: Option<bool>) -> bool {
        self.revision = self.revision.wrapping_add(1);
        let previous = self.owner.replace(owner).unwrap_or(0);
        if owner == 0 {
            self.manager_restore = false;
            if departed {
                if previous != 0 {
                    self.departed_owner = Some(previous);
                }
            } else {
                self.departed_owner = None;
                self.tainted_cache = false;
            }
            self.suppressed_owner = None;
            return false;
        }
        // An uncertain manager identity cannot authorize reading a deferred
        // replacement. An intervening writer does not prove the manager has
        // finished replacing its old cache, even if we captured that writer.
        self.manager_restore = self.departed_owner.is_some() && manager != Some(false);
        let suppressed = self.tainted_cache && self.manager_restore;
        if !suppressed && manager == Some(true) {
            // A manager making a new copy while an owner is still alive (or
            // while it already owns the selection) is not an exit handoff.
            self.tainted_cache = false;
        }
        self.departed_owner = None;
        self.suppressed_owner = suppressed.then_some(owner);
        !suppressed
    }

    fn suppress_current(&mut self) {
        let owner = self.owner.unwrap_or(0);
        self.suppressed_owner = (owner != 0).then_some(owner);
        self.tainted_cache = owner != 0 || self.departed_owner.is_some();
    }

    fn suppress_snapshot(&mut self, revision: u64) {
        // The old value may remain in a manager's cache, but a delayed policy
        // decision must not suppress a newer independently owned selection.
        self.tainted_cache = true;
        if self.revision == revision || self.manager_restore {
            self.suppressed_owner = self.owner.filter(|owner| *owner != 0);
        }
    }

    fn permits_read(&self, owner: u32) -> bool {
        owner != 0 && self.owner == Some(owner) && self.suppressed_owner != Some(owner)
    }
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
    available: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    staging: super::file_materialize::StagingArea,
}

struct WorkerAvailability(Arc<AtomicBool>);

impl WorkerAvailability {
    fn new(available: Arc<AtomicBool>) -> Self {
        available.store(false, Ordering::Release);
        Self(available)
    }

    fn mark_ready(&self) {
        self.0.store(true, Ordering::Release);
    }
}

impl Drop for WorkerAvailability {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
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
        let available = Arc::new(AtomicBool::new(false));
        let thread_available = Arc::clone(&available);
        let worker = thread::Builder::new()
            .name("copypaste-x11-clipboard".into())
            .spawn(move || worker(rx, read, thread_changed, thread_available))?;
        Ok(Self {
            sender: Arc::new(Sender {
                commands: tx,
                wake: Mutex::new(write),
            }),
            changed,
            available,
            worker: Some(worker),
            staging,
        })
    }
    fn read(&mut self, settings: &copypaste_ipc::ConfigData) -> Option<Capture> {
        if !self.changed.swap(false, Ordering::AcqRel) && !settings.private_mode {
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
    fn is_available(&self) -> bool {
        self.available.load(Ordering::Acquire)
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
    fn is_available(&self) -> bool {
        match self {
            Self::X11(source) => source.is_available(),
            Self::Gnome(source) => source.is_available(),
            Self::Kwin(source) => source.is_available(),
            Self::Wayland(source) => source.is_available(),
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

fn worker(
    commands: mpsc::Receiver<Command>,
    mut wake: UnixStream,
    changed: Arc<AtomicBool>,
    available: Arc<AtomicBool>,
) {
    let availability = WorkerAvailability::new(available);
    let Ok((connection, screen)) = x11rb::connect(None) else {
        return;
    };
    let Ok(window) = connection.generate_id() else {
        return;
    };
    let root = connection.setup().roots[screen].root;
    let Ok(create) = connection.create_window(
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
    ) else {
        return;
    };
    if create.check().is_err() {
        return;
    }
    let Ok(atoms) = Atoms::intern(&connection) else {
        return;
    };
    let Ok(version) =
        connection.xfixes_query_version(XFIXES_SELECTION_VERSION.0, XFIXES_SELECTION_VERSION.1)
    else {
        return;
    };
    let Ok(version) = version.reply() else {
        return;
    };
    if (version.major_version, version.minor_version) < XFIXES_SELECTION_VERSION {
        return;
    }
    let Ok(subscription) = connection.xfixes_select_selection_input(
        window,
        atoms.clipboard,
        SelectionEventMask::SET_SELECTION_OWNER
            | SelectionEventMask::SELECTION_WINDOW_DESTROY
            | SelectionEventMask::SELECTION_CLIENT_CLOSE,
    ) else {
        return;
    };
    if subscription.check().is_err() {
        return;
    }
    availability.mark_ready();
    let mut owned = None;
    let mut privacy = SelectionPrivacy::default();
    loop {
        while let Ok(request) = commands.try_recv() {
            if !command(
                &connection,
                window,
                &atoms,
                &mut owned,
                &mut privacy,
                request,
                &changed,
                &commands,
                &mut wake,
            ) {
                return;
            }
        }
        loop {
            match connection.poll_for_event() {
                Ok(Some(next_event)) => event(
                    &connection,
                    window,
                    &atoms,
                    &mut owned,
                    &mut privacy,
                    next_event,
                    &changed,
                ),
                Ok(None) => break,
                Err(_) => return,
            }
        }
        let xfd = unsafe { BorrowedFd::borrow_raw(connection.stream().as_raw_fd()) };
        let mut fds = [
            PollFd::from_borrowed_fd(xfd, PollFlags::IN),
            PollFd::new(&wake, PollFlags::IN),
        ];
        match poll(&mut fds, None) {
            Err(rustix::io::Errno::INTR) => continue,
            Err(_) => return,
            Ok(_) => {}
        }
        if fds[0]
            .revents()
            .intersects(PollFlags::ERR | PollFlags::HUP | PollFlags::NVAL)
        {
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
    privacy: &mut SelectionPrivacy,
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
                privacy,
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
    privacy: &mut SelectionPrivacy,
    event: Event,
    changed: &AtomicBool,
) {
    match event {
        Event::XfixesSelectionNotify(e) if e.selection == a.clipboard => {
            note_selection(c, window, a, privacy, e, changed);
        }
        Event::SelectionRequest(e) if e.selection == a.clipboard => serve(c, a, owned.as_ref(), e),
        _ => {}
    }
}

fn note_selection(
    c: &RustConnection,
    window: u32,
    atoms: &Atoms,
    privacy: &mut SelectionPrivacy,
    event: x11rb::protocol::xfixes::SelectionNotifyEvent,
    changed: &AtomicBool,
) {
    let manager = if (privacy.tainted_cache || privacy.departed_owner.is_some())
        && event.owner != 0
        && event.owner != window
    {
        manager_owns_selection(c, window, atoms, event.owner)
    } else {
        Some(false)
    };
    let readable = privacy.observe(
        event.owner,
        event.subtype != SelectionEvent::SET_SELECTION_OWNER,
        manager,
    );
    changed.store(readable && event.owner != window, Ordering::Release);
}

fn manager_owns_selection(
    c: &RustConnection,
    window: u32,
    atoms: &Atoms,
    owner: u32,
) -> Option<bool> {
    let manager = c
        .get_selection_owner(atoms.clipboard_manager)
        .ok()?
        .reply()
        .ok()?
        .owner;
    let mut uncertain = false;
    if manager != 0 {
        match super::linux_attribution::same_client(c, owner, manager) {
            Some(true) => return Some(true),
            Some(false) => {}
            None => uncertain = true,
        }
    }
    let root = c.query_tree(window).ok()?.reply().ok()?.root;
    let reply = c
        .get_property(false, root, atoms.wm_check, AtomEnum::WINDOW, 0, 1)
        .ok()?
        .reply()
        .ok()?;
    if reply.type_ == u32::from(AtomEnum::NONE) && reply.value.is_empty() {
        return (!uncertain).then_some(false);
    }
    if reply.bytes_after != 0 || reply.value_len != 1 {
        return None;
    }
    let manager = reply.value32()?.next().filter(|value| *value != 0)?;
    match super::linux_attribution::same_client(c, owner, manager) {
        Some(true) => Some(true),
        Some(false) if !uncertain => Some(false),
        _ => None,
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
    privacy: &'a mut SelectionPrivacy,
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
        // The owner reply is a server barrier. Drain earlier XFixes events
        // before acknowledging a policy drop, even if the idle flag was clear.
        while let Some(next) = self.c.poll_for_event().ok()? {
            event(
                self.c,
                self.window,
                self.atoms,
                self.owned,
                self.privacy,
                next,
                self.changed,
            );
        }
        self.privacy.owner.get_or_insert(owner);
        let revision = self.privacy.revision;
        if settings.private_mode {
            self.privacy.suppress_current();
            self.changed.store(false, Ordering::Release);
            return None;
        }
        if owner == self.window || !self.privacy.permits_read(owner) {
            return None;
        }
        let source = (owner != 0)
            .then(|| super::linux_attribution::resolve_owner(self.c, owner))
            .flatten();
        if !Self::can_read_source(settings, source.as_ref()) {
            self.privacy.suppress_snapshot(revision);
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
            self.privacy.suppress_snapshot(revision);
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
            loop {
                match self.c.poll_for_event() {
                    Ok(Some(event)) => match event {
                        Event::SelectionRequest(request)
                            if request.selection == self.atoms.clipboard =>
                        {
                            serve(self.c, self.atoms, self.owned.as_ref(), request);
                        }
                        Event::XfixesSelectionNotify(event)
                            if event.selection == self.atoms.clipboard =>
                        {
                            note_selection(
                                self.c,
                                self.window,
                                self.atoms,
                                self.privacy,
                                event,
                                self.changed,
                            );
                        }
                        event => return Some(event),
                    },
                    Ok(None) => break,
                    Err(_) => {
                        self.stopped = true;
                        return None;
                    }
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
            match poll(&mut fds, Some(&timeout)) {
                // Signals do not invalidate the connection. The next iteration
                // recomputes the remaining time from the original deadline.
                Err(rustix::io::Errno::INTR) => continue,
                Err(_) => {
                    self.stopped = true;
                    return None;
                }
                Ok(_) => {}
            }
            if fds[0]
                .revents()
                .intersects(PollFlags::ERR | PollFlags::HUP | PollFlags::NVAL)
            {
                self.stopped = true;
                return None;
            }
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
    use std::process::{Command as ProcessCommand, Stdio};
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use std::time::Duration;

    #[test]
    fn policy_drop_survives_a_deferred_manager_handoff_and_keeps_the_next_copy() {
        let mut privacy = SelectionPrivacy::default();
        assert!(privacy.observe(1, false, Some(false)));
        privacy.suppress_current();
        assert!(!privacy.permits_read(1));
        assert!(!privacy.observe(0, true, Some(false)));
        privacy.suppress_current();
        assert!(!privacy.observe(2, false, Some(true)));
        assert!(!privacy.permits_read(2));
        assert!(privacy.observe(3, false, Some(false)));
        assert!(privacy.permits_read(3));
    }

    #[test]
    fn resume_fences_an_owner_that_closed_before_the_last_private_poll() {
        let mut privacy = SelectionPrivacy::default();
        privacy.observe(1, false, Some(false));
        privacy.observe(0, true, Some(false));
        privacy.suppress_current();
        assert!(!privacy.observe(2, false, Some(true)));
        assert!(!privacy.permits_read(2));
    }

    #[test]
    fn a_new_copy_from_the_same_owner_is_not_the_suppressed_snapshot() {
        let mut privacy = SelectionPrivacy::default();
        privacy.observe(1, false, Some(false));
        privacy.suppress_current();
        assert!(privacy.observe(1, false, Some(false)));
        assert!(privacy.permits_read(1));
    }

    #[test]
    fn intervening_copies_do_not_authorize_a_managers_old_private_cache() {
        let mut privacy = SelectionPrivacy::default();
        privacy.observe(1, false, Some(false));
        privacy.suppress_current();
        privacy.observe(0, true, Some(false));
        for writer in [2, 3] {
            assert!(privacy.observe(writer, false, Some(false)));
            assert!(privacy.permits_read(writer));
            privacy.observe(0, true, Some(false));
            assert!(!privacy.observe(9, false, Some(true)));
            assert!(!privacy.permits_read(9));
        }
        // A new manager-owned copy, unlike an exit handoff, establishes a
        // fresh origin and remains usable after retained values were rejected.
        assert!(privacy.observe(9, false, Some(true)));
        assert!(privacy.permits_read(9));
        privacy.observe(4, false, Some(false));
        privacy.observe(0, true, Some(false));
        assert!(privacy.observe(9, false, Some(true)));
    }

    #[test]
    fn a_manager_copy_from_a_live_writer_is_a_fresh_origin() {
        let mut privacy = SelectionPrivacy::default();
        privacy.observe(1, false, Some(false));
        privacy.suppress_current();
        assert!(privacy.observe(9, false, Some(true)));
        assert!(privacy.permits_read(9));
    }

    #[test]
    fn a_delayed_policy_decision_does_not_suppress_the_next_owner() {
        let mut privacy = SelectionPrivacy::default();
        privacy.observe(1, false, Some(false));
        let inspected = privacy.revision;
        privacy.observe(2, false, Some(false));
        privacy.suppress_snapshot(inspected);
        assert!(privacy.permits_read(2));
        privacy.observe(0, true, Some(false));
        assert!(!privacy.observe(9, false, Some(true)));
    }

    #[test]
    fn a_delayed_policy_decision_does_not_suppress_a_new_copy_from_the_same_owner() {
        let mut privacy = SelectionPrivacy::default();
        privacy.observe(1, false, Some(false));
        let inspected = privacy.revision;
        privacy.observe(1, false, Some(false));
        privacy.suppress_snapshot(inspected);
        assert!(privacy.permits_read(1));
    }

    #[test]
    fn a_delayed_policy_decision_still_fences_a_manager_replacement() {
        let mut privacy = SelectionPrivacy::default();
        privacy.observe(1, false, Some(false));
        let inspected = privacy.revision;
        privacy.observe(0, true, Some(false));
        privacy.observe(9, false, Some(true));
        privacy.suppress_snapshot(inspected);
        assert!(!privacy.permits_read(9));
    }

    #[test]
    fn an_explicit_clear_cancels_the_old_handoff_lineage() {
        let mut privacy = SelectionPrivacy::default();
        privacy.observe(1, false, Some(false));
        privacy.suppress_current();
        privacy.observe(0, false, Some(false));
        privacy.suppress_current();
        assert!(privacy.observe(2, false, Some(true)));
    }

    #[test]
    fn an_uncertain_deferred_handoff_fails_closed_without_losing_a_later_copy() {
        let mut privacy = SelectionPrivacy::default();
        privacy.observe(1, false, Some(false));
        privacy.suppress_current();
        privacy.observe(0, true, Some(false));
        assert!(!privacy.observe(2, false, None));
        assert!(!privacy.permits_read(2));
        assert!(privacy.observe(3, false, Some(false)));
    }

    #[test]
    fn x11_worker_availability_tracks_setup_and_unwind() {
        let available = Arc::new(AtomicBool::new(false));
        let failed_setup = Arc::clone(&available);
        let failed_worker = thread::spawn(move || {
            let _availability = WorkerAvailability::new(failed_setup);
        });
        failed_worker.join().expect("setup worker exits normally");
        assert!(
            !available.load(Ordering::Acquire),
            "a worker that returns before setup must remain unavailable"
        );

        let worker_available = Arc::clone(&available);
        let (ready, started) = mpsc::sync_channel(1);
        let (release, continue_worker) = mpsc::sync_channel(1);
        let worker = thread::spawn(move || {
            let availability = WorkerAvailability::new(worker_available);
            assert!(!availability.0.load(Ordering::Acquire));
            availability.mark_ready();
            ready.send(()).expect("report worker readiness");
            continue_worker.recv().expect("wait for release");
            panic!("fixture worker exit");
        });

        started.recv().expect("worker reached ready state");
        assert!(available.load(Ordering::Acquire));
        release.send(()).expect("release worker");
        assert!(worker.join().is_err());
        assert!(!available.load(Ordering::Acquire));
    }

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
        owner: u32,
        stop: Arc<AtomicBool>,
        requests: Arc<AtomicUsize>,
        worker: Option<JoinHandle<()>>,
    }

    impl FixtureOwner {
        fn start(payload: FixturePayload) -> Self {
            Self::start_with_manager(payload, false)
        }

        fn start_with_manager(payload: FixturePayload, manager: bool) -> Self {
            let stop = Arc::new(AtomicBool::new(false));
            let requests = Arc::new(AtomicUsize::new(0));
            let worker_stop = Arc::clone(&stop);
            let worker_requests = Arc::clone(&requests);
            let (ready, started) = mpsc::sync_channel(1);
            let worker = thread::spawn(move || {
                let (connection, screen) = x11rb::connect(None).expect("fixture X server");
                let owner = connection.generate_id().expect("fixture owner window id");
                let frame = connection.generate_id().expect("fixture frame window id");
                let metadata = connection
                    .generate_id()
                    .expect("fixture metadata window id");
                let root = connection.setup().roots[screen].root;
                connection
                    .create_window(
                        0,
                        owner,
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
                    .expect("create fixture owner window");
                connection
                    .create_window(
                        0,
                        frame,
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
                    .expect("create fixture frame window");
                connection
                    .create_window(
                        0,
                        metadata,
                        frame,
                        0,
                        0,
                        1,
                        1,
                        0,
                        WindowClass::INPUT_OUTPUT,
                        0,
                        &CreateWindowAux::new(),
                    )
                    .expect("create reparented fixture metadata window");
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
                        metadata,
                        pid,
                        AtomEnum::CARDINAL,
                        &[std::process::id()],
                    )
                    .expect("fixture pid");
                connection
                    .change_property8(
                        PropMode::REPLACE,
                        metadata,
                        AtomEnum::WM_CLASS,
                        AtomEnum::STRING,
                        b"fixture\0org.example.Fixture\0",
                    )
                    .expect("fixture class");
                if manager {
                    let manager_atom = connection
                        .intern_atom(false, b"CLIPBOARD_MANAGER")
                        .expect("manager atom")
                        .reply()
                        .expect("manager atom reply")
                        .atom;
                    connection
                        .set_selection_owner(owner, manager_atom, x11rb::CURRENT_TIME)
                        .expect("fixture clipboard manager");
                }
                connection
                    .set_selection_owner(owner, atoms.clipboard, x11rb::CURRENT_TIME)
                    .expect("fixture owner");
                connection.flush().expect("flush fixture owner");
                let _ = ready.send(owner);
                while !worker_stop.load(Ordering::Acquire) {
                    loop {
                        match connection.poll_for_event() {
                            Ok(Some(Event::SelectionRequest(request)))
                                if request.selection == atoms.clipboard =>
                            {
                                worker_requests.fetch_add(1, AtomicOrdering::Relaxed);
                                serve_fixture(&connection, &atoms, &payload, request);
                            }
                            Ok(Some(_)) => {}
                            Ok(None) => break,
                            Err(_) => return,
                        }
                    }
                    thread::sleep(Duration::from_millis(1));
                }
            });
            let owner = started
                .recv_timeout(Duration::from_secs(2))
                .expect("fixture owner startup");
            Self {
                owner,
                stop,
                requests,
                worker: Some(worker),
            }
        }

        fn request_count(&self) -> usize {
            self.requests.load(AtomicOrdering::Relaxed)
        }

        fn owner(&self) -> u32 {
            self.owner
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

    struct GtkProvider {
        child: std::process::Child,
        _directory: tempfile::TempDir,
    }

    fn gtk_provider_diagnostics(path: &std::path::Path) -> String {
        use std::io::Read;
        let mut diagnostics = String::new();
        let result = std::fs::File::open(path)
            .and_then(|file| file.take(8192).read_to_string(&mut diagnostics));
        match result {
            Ok(_) => diagnostics,
            Err(error) => format!("could not read GTK provider stderr: {error}"),
        }
    }

    impl GtkProvider {
        fn start() -> Self {
            let directory = tempfile::tempdir().expect("provider directory");
            let payload = directory.path().join("payload");
            let ready = directory.path().join("ready");
            let activity = directory.path().join("activity");
            let stderr = directory.path().join("stderr");
            std::fs::write(&payload, b"fixture text").expect("provider payload");
            let helper =
                std::env::var("COPYPASTE_GTK_CLIPBOARD_PROVIDER").expect("GTK provider helper");
            eprintln!(
                "GTK_PROVIDER_SPAWN {}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("fixture wall clock")
                    .as_micros()
            );
            let child = ProcessCommand::new(helper)
                .env("COPYPASTE_GTK_PROVIDER_DIAGNOSTICS", "1")
                .args([
                    "--application-id",
                    "org.copypaste.QualificationSource",
                    "--ready-file",
                ])
                .arg(&ready)
                .args(["--activity-log"])
                .arg(&activity)
                .args(["--hold-seconds", "5", "--offer", "text/plain;charset=utf-8"])
                .arg(&payload)
                .stdout(Stdio::null())
                .stderr(Stdio::from(
                    std::fs::File::create(&stderr).expect("provider stderr"),
                ))
                .spawn()
                .expect("start GTK provider");
            let mut provider = Self {
                child,
                _directory: directory,
            };
            let deadline = Instant::now() + Duration::from_secs(2);
            while !ready.is_file() && Instant::now() < deadline {
                if let Some(status) = provider.child.try_wait().expect("provider exit status") {
                    panic!(
                        "GTK provider exited before readiness ({status}): {}",
                        gtk_provider_diagnostics(&stderr)
                    );
                }
                thread::sleep(Duration::from_millis(5));
            }
            assert!(
                ready.is_file(),
                "GTK provider did not become ready; exit status {:?}; stderr: {}",
                provider.child.try_wait().expect("provider exit status"),
                gtk_provider_diagnostics(&stderr)
            );
            provider
        }
    }

    impl Drop for GtkProvider {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            eprintln!(
                "GTK provider startup diagnostics: {}",
                gtk_provider_diagnostics(&self._directory.path().join("stderr"))
            );
        }
    }

    fn wait_for_available(clipboard: &X11Clipboard) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while !clipboard.is_available() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(
            clipboard.is_available(),
            "X11 adapter did not become available"
        );
    }

    #[test]
    fn live_x11_fixture_captures_gtk_owner_after_initial_empty_poll() {
        if std::env::var_os("COPYPASTE_X11_FIXTURE") != Some("1".into()) {
            return;
        }
        let data_home = std::path::PathBuf::from(
            std::env::var_os("XDG_DATA_HOME").expect("fixture XDG_DATA_HOME"),
        );
        std::fs::create_dir_all(data_home.join("applications")).expect("fixture desktop dir");
        std::fs::write(
            data_home.join("applications/org.copypaste.QualificationSource.desktop"),
            "[Desktop Entry]\nType=Application\nName=CopyPaste Qualification Source\nStartupWMClass=Org.copypaste.QualificationSource\nExec=true\n",
        )
        .expect("fixture desktop entry");

        let directory = tempfile::tempdir().expect("fixture data dir");
        let settings = copypaste_ipc::ConfigData::default();
        let mut clipboard = X11Clipboard::new(directory.path()).expect("X11 clipboard");
        wait_for_available(&clipboard);
        assert!(clipboard
            .poll_with_policy(CapturePolicy::new(&settings))
            .is_none());
        assert!(!clipboard.changed(), "initial empty poll was not drained");

        let _provider = GtkProvider::start();
        let capture = wait_for_capture(&mut clipboard, &settings).expect("capture later GTK owner");
        assert_eq!(capture.content, "fixture text");
        assert_eq!(
            capture.app_bundle_id.as_deref(),
            Some("org.copypaste.QualificationSource")
        );
        assert_eq!(
            capture.app_name.as_deref(),
            Some("CopyPaste Qualification Source")
        );
    }

    #[test]
    fn x11_worker_clears_availability_after_x_server_disconnect() {
        if std::env::var_os("COPYPASTE_X11_FIXTURE") != Some("1".into()) {
            return;
        }
        let directory = tempfile::tempdir().expect("fixture data dir");
        let owner = FixtureOwner::start(FixturePayload::Stalled);
        let mut clipboard = X11Clipboard::new(directory.path()).expect("X11 clipboard");
        wait_for_available(&clipboard);
        let availability = Arc::clone(&clipboard.available);
        let transfer = thread::spawn(move || {
            let result = clipboard.read(&copypaste_ipc::ConfigData::default());
            (result, clipboard)
        });
        let request_deadline = Instant::now() + Duration::from_secs(2);
        while owner.request_count() == 0 && Instant::now() < request_deadline {
            thread::sleep(Duration::from_millis(1));
        }
        assert!(
            owner.request_count() > 0,
            "stalled transfer did not request the selection owner"
        );
        let server =
            std::env::var("COPYPASTE_X11_FIXTURE_XVFB_PID").expect("fixture Xvfb process id");
        let start = Instant::now();
        assert!(
            ProcessCommand::new("kill")
                .args(["-TERM", server.as_str()])
                .status()
                .expect("signal fixture Xvfb")
                .success(),
            "fixture Xvfb did not accept SIGTERM"
        );
        let (result, clipboard) = transfer.join().expect("transfer must not panic");
        assert!(result.is_none());
        drop(clipboard);
        assert!(
            !availability.load(Ordering::Acquire),
            "X11 adapter remained available after server disconnect during a transfer"
        );
        drop(owner);
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "server disconnect waited for the selection transfer deadline"
        );
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
    fn live_x11_fixture_stale_client_list_preserves_capture_and_privacy() {
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

        let (connection, screen) = x11rb::connect(None).expect("fixture attribution verifier");
        let root = connection.setup().roots[screen].root;
        let client_list = connection
            .intern_atom(false, b"_NET_CLIENT_LIST")
            .expect("fixture client list atom")
            .reply()
            .expect("fixture client list atom reply")
            .atom;
        let previous = connection
            .get_property(false, root, client_list, AtomEnum::WINDOW, 0, 256)
            .expect("read prior fixture client list")
            .reply()
            .expect("prior fixture client list reply")
            .value32()
            .map(|values| values.collect::<Vec<_>>());
        let unrelated = connection
            .generate_id()
            .expect("unrelated fixture window id");
        connection
            .create_window(
                0,
                unrelated,
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
            .expect("create unrelated fixture window");
        connection
            .change_property32(
                PropMode::REPLACE,
                root,
                client_list,
                AtomEnum::WINDOW,
                &[unrelated],
            )
            .expect("publish stale fixture client list");
        connection.flush().expect("flush stale fixture client list");
        struct RestoreClientList<'a> {
            connection: &'a RustConnection,
            root: u32,
            atom: u32,
            previous: Option<Vec<u32>>,
            unrelated: u32,
        }
        impl Drop for RestoreClientList<'_> {
            fn drop(&mut self) {
                if let Some(previous) = &self.previous {
                    let _ = self.connection.change_property32(
                        PropMode::REPLACE,
                        self.root,
                        self.atom,
                        AtomEnum::WINDOW,
                        previous,
                    );
                } else {
                    let _ = self.connection.delete_property(self.root, self.atom);
                }
                let _ = self.connection.destroy_window(self.unrelated);
                let _ = self.connection.flush();
            }
        }
        let _restore = RestoreClientList {
            connection: &connection,
            root,
            atom: client_list,
            previous,
            unrelated,
        };
        // The compositor list names another client throughout all capture and
        // privacy cases; only the bounded same-writer tree can supply identity.
        let text = FixtureOwner::start(FixturePayload::Text(b"fixture text".to_vec()));
        let pid = connection
            .intern_atom(false, b"_NET_WM_PID")
            .expect("fixture pid atom")
            .reply()
            .expect("fixture pid atom reply")
            .atom;
        assert!(
            connection
                .get_property(false, text.owner(), pid, AtomEnum::CARDINAL, 0, 1)
                .expect("read hidden owner pid")
                .reply()
                .expect("read hidden owner pid reply")
                .value32()
                .is_none_or(|mut values| values.next().is_none()),
            "fixture selection owner unexpectedly has direct metadata"
        );
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
    fn live_x11_fixture_rejects_metadata_from_a_different_x_client() {
        if std::env::var_os("COPYPASTE_X11_FIXTURE") != Some("1".into()) {
            return;
        }
        use x11rb::protocol::res::{ClientIdMask, ClientIdSpec, ConnectionExt as _};

        let data_home = std::path::PathBuf::from(
            std::env::var_os("XDG_DATA_HOME").expect("fixture XDG_DATA_HOME"),
        );
        std::fs::create_dir_all(data_home.join("applications")).expect("fixture desktop dir");
        std::fs::write(
            data_home.join("applications/org.example.Fixture.desktop"),
            "[Desktop Entry]\nName=Fixture Writer\nStartupWMClass=org.example.Fixture\n",
        )
        .expect("fixture desktop entry");

        let (owner_connection, screen) = x11rb::connect(None).expect("fixture owner X server");
        let owner = owner_connection
            .generate_id()
            .expect("fixture owner window id");
        let root = owner_connection.setup().roots[screen].root;
        owner_connection
            .create_window(
                0,
                owner,
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
            .expect("create fixture owner window");
        owner_connection.flush().expect("flush fixture owner");

        let (spoof_connection, spoof_screen) =
            x11rb::connect(None).expect("fixture spoof X server");
        let spoof = spoof_connection
            .generate_id()
            .expect("fixture spoof window id");
        let spoof_root = spoof_connection.setup().roots[spoof_screen].root;
        assert_eq!(root, spoof_root, "fixture clients use different roots");
        spoof_connection
            .create_window(
                0,
                spoof,
                spoof_root,
                0,
                0,
                1,
                1,
                0,
                WindowClass::INPUT_OUTPUT,
                0,
                &CreateWindowAux::new(),
            )
            .expect("create fixture spoof window");
        let pid = spoof_connection
            .intern_atom(false, b"_NET_WM_PID")
            .expect("fixture pid atom")
            .reply()
            .expect("fixture pid atom reply")
            .atom;
        spoof_connection
            .change_property32(
                PropMode::REPLACE,
                spoof,
                pid,
                AtomEnum::CARDINAL,
                &[std::process::id()],
            )
            .expect("fixture spoof pid");
        spoof_connection
            .change_property8(
                PropMode::REPLACE,
                spoof,
                AtomEnum::WM_CLASS,
                AtomEnum::STRING,
                b"spoof\0org.example.Fixture\0",
            )
            .expect("fixture spoof class");
        spoof_connection.flush().expect("flush fixture spoof");

        let (resolver, _) = x11rb::connect(None).expect("fixture resolver X server");
        let xres_client = |window| {
            resolver
                .res_query_client_ids(&[ClientIdSpec {
                    client: window,
                    mask: ClientIdMask::CLIENT_XID,
                }])
                .expect("XRes client request")
                .reply()
                .expect("XRes client reply")
                .ids
                .into_iter()
                .find(|identity| identity.spec.mask == ClientIdMask::CLIENT_XID)
                .map(|identity| identity.spec.client)
                .expect("XRes client XID")
        };
        assert_ne!(
            xres_client(owner),
            xres_client(spoof),
            "fixture windows unexpectedly share an X client"
        );
        assert_eq!(
            super::super::linux_attribution::resolve_owner(&resolver, owner),
            None,
            "same-PID metadata from another X client must not be attributed"
        );
    }

    #[test]
    fn live_x11_fixture_policy_dropped_manager_handoff_is_not_replayed() {
        if std::env::var_os("COPYPASTE_X11_FIXTURE") != Some("1".into()) {
            return;
        }
        for (policy, payload, expected_requests, retained) in [
            (
                copypaste_ipc::ConfigData {
                    private_mode: true,
                    ..Default::default()
                },
                FixturePayload::Text(b"private fixture".to_vec()),
                0,
                b"private fixture".as_slice(),
            ),
            (
                copypaste_ipc::ConfigData {
                    skip_secret: true,
                    ..Default::default()
                },
                FixturePayload::Secret,
                2,
                b"ignored".as_slice(),
            ),
        ] {
            let directory = tempfile::tempdir().expect("fixture data directory");
            let mut clipboard = X11Clipboard::new(directory.path()).expect("X11 clipboard");
            wait_for_available(&clipboard);
            let sender = Arc::clone(&clipboard.sender);
            let read_worker = |settings| {
                let (reply, result) = mpsc::channel();
                sender
                    .send(Command::Read { settings, reply })
                    .expect("worker read barrier");
                result
                    .recv_timeout(REQUEST_TIMEOUT)
                    .expect("worker read reply")
            };
            let (observer, _) = x11rb::connect(None).expect("fixture observer");
            let atoms = Atoms::intern(&observer).expect("fixture atoms");
            let wait_for_owner = |expected| {
                let deadline = Instant::now() + Duration::from_secs(2);
                while observer
                    .get_selection_owner(atoms.clipboard)
                    .unwrap()
                    .reply()
                    .unwrap()
                    .owner
                    != expected
                {
                    assert!(
                        Instant::now() < deadline,
                        "fixture ownership did not settle"
                    );
                    thread::sleep(Duration::from_millis(1));
                }
            };
            let owner = FixtureOwner::start(payload);
            wait_for_owner(owner.owner());
            assert!(read_worker(policy.clone()).is_none());
            assert_eq!(owner.request_count(), expected_requests);
            drop(owner);
            wait_for_owner(0);
            if policy.private_mode {
                // Resume must acknowledge ownership even when the earlier
                // private poll already consumed the idle change flag.
                assert!(clipboard.read(&policy).is_none());
            }
            // Cache IO can lag behind a short-lived public writer that the
            // daemon already captured. Its value must not authorize old cache.
            let intermediate =
                FixtureOwner::start(FixturePayload::Text(b"intermediate public copy".to_vec()));
            wait_for_owner(intermediate.owner());
            assert_eq!(
                read_worker(Default::default()).unwrap().content,
                "intermediate public copy"
            );
            drop(intermediate);
            wait_for_owner(0);
            let manager =
                FixtureOwner::start_with_manager(FixturePayload::Text(retained.to_vec()), true);
            wait_for_owner(manager.owner());
            // This processes the actual X11 event rather than passing on an
            // idle flag before the manager's replacement has been observed.
            assert!(
                read_worker(Default::default()).is_none(),
                "a clipboard manager replayed a policy-dropped selection"
            );
            assert_eq!(
                manager.request_count(),
                0,
                "protected payload must not be requested"
            );
            let next = FixtureOwner::start(FixturePayload::Text(b"first public copy".to_vec()));
            let deadline = Instant::now() + Duration::from_secs(2);
            let captured = loop {
                if let Some(capture) = clipboard.poll() {
                    break capture;
                }
                assert!(Instant::now() < deadline, "first post-policy copy was lost");
                thread::sleep(Duration::from_millis(1));
            };
            assert_eq!(captured.content, "first public copy");
            assert!(next.request_count() >= 2);
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

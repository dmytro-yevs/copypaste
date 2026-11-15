//! The relay server: rendezvous by id, then frame forwarding between two
//! WebSockets. State lives only in memory and nothing is ever stored.

use std::collections::HashMap;
use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use rand::{rngs::OsRng, RngCore};
use subtle::ConstantTimeEq;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::{mpsc, oneshot};
use tokio::time::{sleep, timeout, Instant};
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{accept_async_with_config, WebSocketStream};
use tracing::{debug, info};

use crate::proto::{
    observe_reply, verify, Control, ErrorCode, Event, Hello, HexId, Id, Register, Slot, Ticket,
    MAX_CANDIDATES, MAX_CONTROL_BYTES, MAX_FRAME_BYTES, MAX_REGISTRATIONS, OBSERVE_REQUEST_BYTES,
};

type Ws = WebSocketStream<TcpStream>;

/// How long a new connection has to finish the WebSocket upgrade and say hello.
const HELLO_TIMEOUT: Duration = Duration::from_secs(10);
/// Devices that may be present on one id at once; the oldest is dropped.
const MAX_REGS_PER_ID: usize = 8;
/// Queued events per presence connection before a dial is refused as busy.
const EVENT_QUEUE: usize = 16;

#[derive(Debug, Clone)]
pub struct Config {
    /// Optional shared secret every client must present.
    pub token: Option<String>,
    pub max_connections: usize,
    pub max_connections_per_ip: usize,
    /// Bytes one relayed session may forward in total.
    pub session_bytes: u64,
    /// Bytes per second one direction of a session may forward; 0 = unlimited.
    pub session_bytes_per_sec: u64,
    /// A presence connection that says nothing for this long is dropped.
    pub presence_idle: Duration,
    /// A relayed session with no traffic in either direction for this long is
    /// closed.
    pub session_idle: Duration,
    /// How long a dial waits for the target device to answer.
    pub dial_timeout: Duration,
    /// Where devices reach the UDP observer, as `host:port` or `:port`; `None`
    /// offers no direct paths, so every session is spliced.
    pub observe: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            token: None,
            max_connections: 1024,
            max_connections_per_ip: 32,
            session_bytes: 256 * 1024 * 1024,
            session_bytes_per_sec: 4 * 1024 * 1024,
            presence_idle: Duration::from_secs(150),
            session_idle: Duration::from_secs(120),
            dial_timeout: Duration::from_secs(8),
            observe: None,
        }
    }
}

/// Serve relay connections on `listener` until `shutdown` completes.
pub async fn serve(listener: TcpListener, config: Config, shutdown: impl Future<Output = ()>) {
    let relay = Arc::new(Relay {
        config,
        state: Mutex::new(State::default()),
    });
    let mut tasks = tokio::task::JoinSet::new();
    tokio::pin!(shutdown);
    info!(addr = ?listener.local_addr().ok(), "relay listening");
    loop {
        tokio::select! {
            biased;
            () = &mut shutdown => break,
            Some(_) = tasks.join_next() => {}
            accepted = listener.accept() => {
                let Ok((stream, addr)) = accepted else { continue };
                let Some(guard) = Relay::admit(&relay, addr.ip()) else {
                    debug!("connection limit reached");
                    continue;
                };
                tasks.spawn(handle(Arc::clone(&relay), stream, guard));
            }
        }
    }
    tasks.shutdown().await;
    info!("relay stopped");
}

/// Answer each observer datagram with the address it came from, until
/// `shutdown` completes. Stateless: nothing is kept between datagrams.
pub async fn observe(socket: UdpSocket, shutdown: impl Future<Output = ()>) {
    tokio::pin!(shutdown);
    let mut buffer = [0u8; OBSERVE_REQUEST_BYTES + 1];
    loop {
        let (len, from) = tokio::select! {
            () = &mut shutdown => break,
            received = socket.recv_from(&mut buffer) => match received {
                Ok(received) => received,
                Err(_) => continue,
            },
        };
        if let Some(reply) = observe_reply(&buffer[..len], from) {
            let _ = socket.send_to(&reply, from).await;
        }
    }
}

struct Relay {
    config: Config,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    next_conn: u64,
    next_seq: u64,
    connections: usize,
    per_ip: HashMap<IpAddr, usize>,
    presences: HashMap<u64, Presence>,
    ids: HashMap<Id, Vec<Reg>>,
    pending: HashMap<Ticket, oneshot::Sender<Admitted>>,
    answers: HashMap<Ticket, oneshot::Sender<Vec<SocketAddr>>>,
}

struct Presence {
    events: mpsc::Sender<Event>,
    ids: Vec<Id>,
    online: Vec<Id>,
}

#[derive(Clone, Copy)]
struct Reg {
    conn: u64,
    slot: Slot,
    signed: bool,
    seq: u64,
}

struct Admitted {
    ws: Ws,
    _guard: ConnGuard,
}

/// One admitted TCP connection, counted until dropped.
struct ConnGuard {
    relay: Arc<Relay>,
    ip: IpAddr,
}

impl Drop for ConnGuard {
    fn drop(&mut self) {
        let mut state = self.relay.lock();
        state.connections = state.connections.saturating_sub(1);
        if let Some(count) = state.per_ip.get_mut(&self.ip) {
            *count -= 1;
            if *count == 0 {
                state.per_ip.remove(&self.ip);
            }
        }
    }
}

impl Relay {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }

    fn admit(relay: &Arc<Self>, ip: IpAddr) -> Option<ConnGuard> {
        let mut state = relay.lock();
        let per_ip = state.per_ip.get(&ip).copied().unwrap_or(0);
        if state.connections >= relay.config.max_connections
            || per_ip >= relay.config.max_connections_per_ip
        {
            return None;
        }
        state.connections += 1;
        *state.per_ip.entry(ip).or_default() += 1;
        Some(ConnGuard {
            relay: Arc::clone(relay),
            ip,
        })
    }

    fn authorized(&self, token: Option<&str>) -> bool {
        match (&self.config.token, token) {
            (None, _) => true,
            (Some(expected), Some(given)) => {
                bool::from(expected.as_bytes().ct_eq(given.as_bytes()))
            }
            (Some(_), None) => false,
        }
    }

    fn add_presence(&self, events: mpsc::Sender<Event>) -> u64 {
        let mut state = self.lock();
        state.next_conn += 1;
        let conn = state.next_conn;
        state.presences.insert(
            conn,
            Presence {
                events,
                ids: Vec::new(),
                online: Vec::new(),
            },
        );
        conn
    }

    fn register(&self, conn: u64, register: Register) {
        let mut state = self.lock();
        let Some(presence) = state.presences.get_mut(&conn) else {
            return;
        };
        let old = std::mem::take(&mut presence.ids);
        state.unregister(conn, &old);
        let mut ids = Vec::with_capacity(register.signed.len() + register.open.len());
        for signed in register.signed {
            // The same device on a newer connection replaces its stale entry.
            if let Some(regs) = state.ids.get_mut(&signed.id) {
                regs.retain(|reg| reg.slot != signed.slot);
            }
            state.insert(signed.id, conn, signed.slot, true);
            ids.push(signed.id);
        }
        for HexId(id) in register.open {
            let mut slot = [0u8; 16];
            slot[..8].copy_from_slice(&conn.to_be_bytes());
            state.insert(id, conn, slot, false);
            ids.push(id);
        }
        if let Some(presence) = state.presences.get_mut(&conn) {
            presence.ids = ids;
        }
        state.refresh_online();
    }

    fn remove_presence(&self, conn: u64) {
        let mut state = self.lock();
        if let Some(presence) = state.presences.remove(&conn) {
            state.unregister(conn, &presence.ids);
            state.refresh_online();
        }
    }

    /// Ask the newest other device on `id` to accept a session.
    fn request(
        &self,
        id: &Id,
        slot: &Slot,
    ) -> Result<(Ticket, oneshot::Receiver<Admitted>), ErrorCode> {
        let mut state = self.lock();
        let ticket = state.notify(id, slot, |ticket| Event::Incoming { ticket })?;
        let (deliver, admitted) = oneshot::channel();
        state.pending.insert(ticket, deliver);
        Ok((ticket, admitted))
    }

    /// Wait for the device on `id` to accept; a ticket nobody accepted in time
    /// is forgotten, so a target that never answers leaves nothing behind.
    async fn admission(&self, id: &Id, slot: &Slot) -> Result<Admitted, ErrorCode> {
        let (ticket, admitted) = self.request(id, slot)?;
        match timeout(self.config.dial_timeout, admitted).await {
            Ok(Ok(peer)) => Ok(peer),
            _ => {
                self.lock().pending.remove(&ticket);
                Err(ErrorCode::Offline)
            }
        }
    }

    /// Hand `candidates` to the newest other device on `id`.
    fn punch(
        &self,
        id: &Id,
        slot: &Slot,
        candidates: Vec<SocketAddr>,
    ) -> Result<(Ticket, oneshot::Receiver<Vec<SocketAddr>>), ErrorCode> {
        let mut state = self.lock();
        let ticket = state.notify(id, slot, |ticket| Event::Punch { ticket, candidates })?;
        let (deliver, answer) = oneshot::channel();
        state.answers.insert(ticket, deliver);
        Ok((ticket, answer))
    }

    fn answer(&self, ticket: &Ticket, candidates: Vec<SocketAddr>) {
        if let Some(deliver) = self.lock().answers.remove(ticket) {
            let _ = deliver.send(candidates);
        }
    }
}

impl State {
    /// Send the event `make` builds to the newest device on `id` other than
    /// `slot`, under a fresh ticket.
    fn notify(
        &mut self,
        id: &Id,
        slot: &Slot,
        make: impl FnOnce(Ticket) -> Event,
    ) -> Result<Ticket, ErrorCode> {
        let target = self
            .ids
            .get(id)
            .and_then(|regs| {
                regs.iter()
                    .filter(|reg| reg.slot != *slot)
                    .max_by_key(|reg| reg.seq)
            })
            .map(|reg| reg.conn)
            .ok_or(ErrorCode::Offline)?;
        let mut ticket = [0u8; 16];
        OsRng.fill_bytes(&mut ticket);
        self.presences
            .get(&target)
            .ok_or(ErrorCode::Offline)?
            .events
            .try_send(make(ticket))
            .map_err(|_| ErrorCode::Busy)?;
        Ok(ticket)
    }

    fn insert(&mut self, id: Id, conn: u64, slot: Slot, signed: bool) {
        self.next_seq += 1;
        let seq = self.next_seq;
        let regs = self.ids.entry(id).or_default();
        if regs.len() >= MAX_REGS_PER_ID {
            if let Some(oldest) = regs.iter().enumerate().min_by_key(|(_, reg)| reg.seq) {
                let index = oldest.0;
                regs.swap_remove(index);
            }
        }
        regs.push(Reg {
            conn,
            slot,
            signed,
            seq,
        });
    }

    fn unregister(&mut self, conn: u64, ids: &[Id]) {
        for id in ids {
            if let Some(regs) = self.ids.get_mut(id) {
                regs.retain(|reg| reg.conn != conn);
                if regs.is_empty() {
                    self.ids.remove(id);
                }
            }
        }
    }

    /// Tell every presence whose set of online peers changed.
    fn refresh_online(&mut self) {
        let ids = &self.ids;
        for (conn, presence) in &mut self.presences {
            let online: Vec<Id> = presence
                .ids
                .iter()
                .filter(|id| {
                    let Some(regs) = ids.get(*id) else {
                        return false;
                    };
                    let Some(own) = regs.iter().find(|reg| reg.conn == *conn && reg.signed) else {
                        return false;
                    };
                    regs.iter()
                        .any(|reg| reg.signed && reg.conn != *conn && reg.slot != own.slot)
                })
                .copied()
                .collect();
            if online != presence.online {
                presence.online.clone_from(&online);
                let ids = online.into_iter().map(HexId).collect();
                let _ = presence.events.try_send(Event::Online { ids });
            }
        }
    }
}

fn ws_config() -> WebSocketConfig {
    WebSocketConfig {
        max_message_size: Some(MAX_FRAME_BYTES),
        max_frame_size: Some(MAX_FRAME_BYTES),
        ..WebSocketConfig::default()
    }
}

async fn handle(relay: Arc<Relay>, stream: TcpStream, guard: ConnGuard) {
    let _ = stream.set_nodelay(true);
    let Ok(Ok(mut ws)) = timeout(
        HELLO_TIMEOUT,
        accept_async_with_config(stream, Some(ws_config())),
    )
    .await
    else {
        return;
    };
    let Ok(Some(hello)) = timeout(HELLO_TIMEOUT, read_control::<Hello>(&mut ws)).await else {
        return;
    };
    let token = match &hello {
        Hello::Presence { token }
        | Hello::Connect { token, .. }
        | Hello::Accept { token, .. }
        | Hello::Punch { token, .. } => token.as_deref(),
    };
    if !relay.authorized(token) {
        refuse(ws, ErrorCode::Unauthorized).await;
        return;
    }
    match hello {
        Hello::Presence { .. } => presence(&relay, ws).await,
        Hello::Connect { id, slot, .. } => connect(&relay, ws, guard, &id, &slot).await,
        Hello::Punch {
            id,
            slot,
            candidates,
            ..
        } => punch(&relay, ws, &id, &slot, candidates).await,
        Hello::Accept { ticket, .. } => {
            let deliver = relay.lock().pending.remove(&ticket);
            match deliver {
                Some(deliver) => {
                    let _ = deliver.send(Admitted { ws, _guard: guard });
                }
                None => refuse(ws, ErrorCode::Invalid).await,
            }
        }
    }
}

async fn presence(relay: &Relay, mut ws: Ws) {
    let mut nonce = [0u8; 32];
    OsRng.fill_bytes(&mut nonce);
    let challenge = Event::Challenge {
        nonce,
        observe: relay.config.observe.clone(),
    };
    if send_event(&mut ws, &challenge).await.is_err() {
        return;
    }
    let (events, mut queued) = mpsc::channel(EVENT_QUEUE);
    let conn = relay.add_presence(events);
    let mut deadline = Instant::now() + relay.config.presence_idle;
    loop {
        tokio::select! {
            message = ws.next() => {
                deadline = Instant::now() + relay.config.presence_idle;
                match message {
                    Some(Ok(Message::Text(text))) => {
                        match parse_control::<Control>(&text) {
                            Some(Control::Register(register))
                                if valid_registration(&nonce, &register) =>
                            {
                                relay.register(conn, register);
                            }
                            Some(Control::PunchAnswer { ticket, candidates })
                                if candidates.len() <= MAX_CANDIDATES =>
                            {
                                relay.answer(&ticket, candidates);
                            }
                            _ => {
                                let _ = send_event(&mut ws, &Event::Error { code: ErrorCode::Invalid }).await;
                                break;
                            }
                        }
                    }
                    Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                    _ => break,
                }
            }
            Some(event) = queued.recv() => {
                if send_event(&mut ws, &event).await.is_err() {
                    break;
                }
            }
            () = tokio::time::sleep_until(deadline) => break,
        }
    }
    relay.remove_presence(conn);
    let _ = ws.close(None).await;
}

fn valid_registration(nonce: &[u8; 32], register: &Register) -> bool {
    register.signed.len() + register.open.len() <= MAX_REGISTRATIONS
        && register.signed.iter().all(|signed| verify(nonce, signed))
}

/// Trade UDP candidates between the dialler and the device on `id`; nothing
/// else crosses this connection.
async fn punch(relay: &Relay, ws: Ws, id: &Id, slot: &Slot, candidates: Vec<SocketAddr>) {
    if candidates.is_empty() || candidates.len() > MAX_CANDIDATES {
        refuse(ws, ErrorCode::Invalid).await;
        return;
    }
    let (ticket, answer) = match relay.punch(id, slot, candidates) {
        Ok(pending) => pending,
        Err(code) => {
            refuse(ws, code).await;
            return;
        }
    };
    let event = match timeout(relay.config.dial_timeout, answer).await {
        Ok(Ok(candidates)) => Event::PunchAnswer { candidates },
        _ => {
            relay.lock().answers.remove(&ticket);
            Event::Error {
                code: ErrorCode::Offline,
            }
        }
    };
    let mut ws = ws;
    let _ = send_event(&mut ws, &event).await;
    let _ = ws.close(None).await;
}

async fn connect(relay: &Relay, mut ws: Ws, guard: ConnGuard, id: &Id, slot: &Slot) {
    let peer = match relay.admission(id, slot).await {
        Ok(peer) => peer,
        Err(code) => {
            refuse(ws, code).await;
            return;
        }
    };
    let Admitted {
        ws: mut other,
        _guard: other_guard,
    } = peer;
    if send_event(&mut ws, &Event::Ready).await.is_err()
        || send_event(&mut other, &Event::Ready).await.is_err()
    {
        return;
    }
    pump(ws, other, &relay.config).await;
    drop((guard, other_guard));
}

/// Forward binary frames both ways until either side ends, the byte budget is
/// spent, or neither side has sent anything for `session_idle`.
async fn pump(a: Ws, b: Ws, config: &Config) {
    let (a_tx, a_rx) = a.split();
    let (b_tx, b_rx) = b.split();
    let budget = AtomicU64::new(config.session_bytes);
    let start = Instant::now();
    let active = AtomicU64::new(0);
    let shared = Shared {
        budget: &budget,
        active: &active,
        start,
        config,
    };
    tokio::select! {
        () = forward(a_rx, b_tx, &shared) => {}
        () = forward(b_rx, a_tx, &shared) => {}
    }
}

struct Shared<'a> {
    budget: &'a AtomicU64,
    /// Milliseconds since `start` of the last forwarded frame, either way.
    active: &'a AtomicU64,
    start: Instant,
    config: &'a Config,
}

impl Shared<'_> {
    fn touch(&self) {
        let now = u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.active.fetch_max(now, Ordering::Relaxed);
    }

    fn idle_for(&self) -> Duration {
        self.start
            .elapsed()
            .saturating_sub(Duration::from_millis(self.active.load(Ordering::Relaxed)))
    }
}

async fn forward(mut rx: SplitStream<Ws>, mut tx: SplitSink<Ws, Message>, shared: &Shared<'_>) {
    let mut bucket = Bucket::new(shared.config.session_bytes_per_sec);
    loop {
        let message = match timeout(shared.config.session_idle, rx.next()).await {
            Ok(Some(Ok(message))) => message,
            Ok(_) => break,
            Err(_) if shared.idle_for() < shared.config.session_idle => continue,
            Err(_) => break,
        };
        match message {
            Message::Binary(data) => {
                let len = data.len() as u64;
                let spent =
                    shared
                        .budget
                        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |left| {
                            left.checked_sub(len)
                        });
                if spent.is_err() {
                    break;
                }
                shared.touch();
                bucket.take(len).await;
                if tx.send(Message::Binary(data)).await.is_err() {
                    break;
                }
            }
            Message::Ping(_) | Message::Pong(_) => {}
            _ => break,
        }
    }
    let _ = tx.close().await;
}

/// A token bucket holding at most one second of traffic.
struct Bucket {
    rate: u64,
    tokens: f64,
    refilled: Instant,
}

impl Bucket {
    fn new(rate: u64) -> Self {
        Self {
            rate,
            tokens: rate as f64,
            refilled: Instant::now(),
        }
    }

    async fn take(&mut self, len: u64) {
        if self.rate == 0 {
            return;
        }
        let rate = self.rate as f64;
        let now = Instant::now();
        let capacity = rate.max(MAX_FRAME_BYTES as f64);
        self.tokens =
            (self.tokens + now.duration_since(self.refilled).as_secs_f64() * rate).min(capacity);
        self.refilled = now;
        self.tokens -= len as f64;
        if self.tokens < 0.0 {
            sleep(Duration::from_secs_f64(-self.tokens / rate)).await;
        }
    }
}

async fn read_control<T: serde::de::DeserializeOwned>(ws: &mut Ws) -> Option<T> {
    loop {
        match ws.next().await? {
            Ok(Message::Text(text)) => return parse_control(&text),
            Ok(Message::Ping(_) | Message::Pong(_)) => {}
            _ => return None,
        }
    }
}

fn parse_control<T: serde::de::DeserializeOwned>(text: &str) -> Option<T> {
    if text.len() > MAX_CONTROL_BYTES {
        return None;
    }
    serde_json::from_str(text).ok()
}

async fn send_event(ws: &mut Ws, event: &Event) -> Result<(), ()> {
    let text = serde_json::to_string(event).map_err(|_| ())?;
    ws.send(Message::Text(text)).await.map_err(|_| ())
}

async fn refuse(mut ws: Ws, code: ErrorCode) {
    let _ = send_event(&mut ws, &Event::Error { code }).await;
    let _ = ws.close(None).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn an_unanswered_dial_forgets_its_ticket() {
        let relay = Relay {
            config: Config {
                dial_timeout: Duration::from_millis(10),
                ..Config::default()
            },
            state: Mutex::new(State::default()),
        };
        let (events, mut incoming) = mpsc::channel(EVENT_QUEUE);
        let target = relay.add_presence(events);
        let id = [7u8; 32];
        relay.lock().insert(id, target, [1u8; 16], true);

        for _ in 0..3 {
            let refused = relay.admission(&id, &[2u8; 16]).await;
            assert!(matches!(refused, Err(ErrorCode::Offline)));
            assert!(matches!(incoming.try_recv(), Ok(Event::Incoming { .. })));
        }
        assert!(relay.lock().pending.is_empty());
    }
}

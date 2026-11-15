//! The device side of the relay: one presence connection, plus a fresh
//! WebSocket per relayed session.
//!
//! The relay routes by rendezvous ids derived from each pairing's key, and the
//! existing Noise session runs end to end inside the relayed stream, so the
//! relay sees opaque frames only. A pairing invitation is routed by a
//! nameplate — one byte of the code — and authenticated by the existing PAKE.

mod frames;
mod punch;

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use backon::BackoffBuilder as _;
use copypaste_relay::proto::{
    signed_message, Control, ErrorCode, Event, Hello, HexId, Id, Register, Signed, Slot, Ticket,
    MAX_CONTROL_BYTES, MAX_FRAME_BYTES,
};
use ed25519_dalek::{Signer, SigningKey};
use futures_util::{SinkExt, StreamExt};
use hkdf::Hkdf;
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};
use tokio::sync::{mpsc, watch, Notify, Semaphore};
use tokio::time::{interval, sleep, timeout, Instant, MissedTickBehavior};
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, info, warn};
use url::Url;
use zeroize::Zeroizing;

use crate::transport::{Conn, PairingCode, TransportError, TOKEN_LEN};
use frames::{relay_addr, Socket, WsFrames};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// One ping per minute keeps NAT bindings and proxies open; the relay drops a
/// presence after 150 s of silence.
const PING_INTERVAL: Duration = Duration::from_secs(60);
const SILENCE_LIMIT: Duration = Duration::from_secs(150);
const MAX_ACCEPTS_IN_FLIGHT: usize = 4;
/// Failed invitation attempts through the relay before the invitation stops
/// being offered there; a new invitation offers it again.
const MAX_INVITATION_FAILURES: u32 = 3;
/// A presence that stayed up this long resets the reconnect backoff.
const STABLE_CONNECTION: Duration = Duration::from_secs(60);
/// After a failed punch to a peer, its sessions go straight to the splice for
/// this long: a symmetric NAT will not start cooperating within minutes.
const PUNCH_RETRY_AFTER: Duration = Duration::from_secs(600);

/// A validated relay address. A `token` query parameter is the relay's
/// optional access secret: it is sent inside the first message, never in the
/// request URL, so proxies do not log it.
#[derive(Clone, PartialEq, Eq)]
pub struct RelayConfig {
    url: String,
    token: Option<String>,
}

impl std::fmt::Debug for RelayConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RelayConfig")
            .field("url", &self.url)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RelayConfigError {
    #[error("Enter a ws:// or wss:// relay address.")]
    Invalid,
}

impl RelayConfig {
    /// Where to reach the observer the relay advertised: `:port` means the
    /// relay's own host.
    fn observer(&self, advertised: &str) -> Option<String> {
        if let Some(port) = advertised.strip_prefix(':') {
            let host = Url::parse(&self.url).ok()?.host_str()?.to_string();
            Some(format!("{host}:{port}"))
        } else {
            Some(advertised.to_string())
        }
    }

    /// # Errors
    ///
    /// [`RelayConfigError::Invalid`] unless `value` is a `ws://` or `wss://`
    /// URL with a host and no credentials.
    pub fn parse(value: &str) -> Result<Self, RelayConfigError> {
        let mut url = Url::parse(value.trim()).map_err(|_| RelayConfigError::Invalid)?;
        if !matches!(url.scheme(), "ws" | "wss")
            || url.host_str().is_none_or(str::is_empty)
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(RelayConfigError::Invalid);
        }
        let token = url
            .query_pairs()
            .find(|(key, _)| key == "token")
            .map(|(_, value)| value.into_owned())
            .filter(|token| !token.is_empty());
        url.set_query(None);
        url.set_fragment(None);
        Ok(Self {
            url: url.into(),
            token,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayStatus {
    Off,
    Connecting,
    Connected,
    Unauthorized,
    Unreachable,
}

impl From<RelayStatus> for copypaste_ipc::RelayState {
    fn from(status: RelayStatus) -> Self {
        match status {
            RelayStatus::Off => Self::Off,
            RelayStatus::Connecting => Self::Connecting,
            RelayStatus::Connected => Self::Connected,
            RelayStatus::Unauthorized => Self::Unauthorized,
            RelayStatus::Unreachable => Self::Unreachable,
        }
    }
}

/// One way to reach a peer through the relay.
#[derive(Clone)]
pub struct RelayRoute {
    config: Arc<RelayConfig>,
    id: Id,
    slot: Slot,
    /// Set for paired devices, which first try a direct punched path.
    punch: Option<Arc<Inner>>,
}

impl RelayRoute {
    /// A direct punched path when the relay offers one and this peer has not
    /// recently refused it, else a session spliced by the relay.
    pub(crate) async fn open(&self) -> Result<Conn, TransportError> {
        if let Some(inner) = &self.punch {
            if let Some(observe) = inner.punch_target(&self.id) {
                let punched = timeout(
                    punch::PUNCH_BUDGET,
                    punch::dial(
                        &self.config,
                        &observe,
                        inner.port_mapping.load(Ordering::Relaxed),
                        self.id,
                        self.slot,
                    ),
                )
                .await;
                match punched {
                    Ok(Ok(conn)) => {
                        debug!("direct path established");
                        return Ok(conn);
                    }
                    _ => {
                        // Rate-limited by the per-peer retry window below.
                        info!("direct path failed; using the relay");
                        inner.note_punch_failure(self.id);
                    }
                }
            }
        }
        let mut socket = open_socket(&self.config).await?;
        send(
            &mut socket,
            &Hello::Connect {
                token: self.config.token.clone(),
                id: self.id,
                slot: self.slot,
            },
        )
        .await?;
        await_ready(&mut socket).await?;
        let addr = relay_addr(&socket);
        Ok(Conn::relayed(Box::new(WsFrames(socket)), addr))
    }
}

/// What this device registers under: its pairings' keys and, while an
/// invitation is open, that invitation's nameplate id.
pub(crate) struct Registrations {
    pub psks: Vec<Zeroizing<[u8; TOKEN_LEN]>>,
    pub invitation: Option<Id>,
}

#[derive(Clone)]
pub struct RelayClient {
    inner: Arc<Inner>,
}

struct Inner {
    config: watch::Sender<Option<Arc<RelayConfig>>>,
    status: watch::Sender<RelayStatus>,
    online: RwLock<HashSet<Id>>,
    online_changes: watch::Sender<u64>,
    registrations: Notify,
    device_id: RwLock<Option<String>>,
    invitation_failures: AtomicU32,
    /// The observer of the connected relay, while presence is up.
    observer: RwLock<Option<String>>,
    punch_failures: Mutex<HashMap<Id, Instant>>,
    /// Whether punches may ask the router to forward their port.
    port_mapping: std::sync::atomic::AtomicBool,
    /// The last settled status logged, so a retry loop logs once.
    reported: Mutex<Option<RelayStatus>>,
}

impl Inner {
    fn punch_target(&self, id: &Id) -> Option<String> {
        let mut failures = self
            .punch_failures
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        failures.retain(|_, failed| failed.elapsed() < PUNCH_RETRY_AFTER);
        if failures.contains_key(id) {
            return None;
        }
        self.observer
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    fn note_punch_failure(&self, id: Id) {
        self.punch_failures
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(id, Instant::now());
    }

    fn set_observer(&self, observer: Option<String>) {
        *self
            .observer
            .write()
            .unwrap_or_else(|error| error.into_inner()) = observer;
    }
}

impl Default for RelayClient {
    fn default() -> Self {
        Self::new()
    }
}

impl RelayClient {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                config: watch::Sender::new(None),
                status: watch::Sender::new(RelayStatus::Off),
                online: RwLock::new(HashSet::new()),
                online_changes: watch::Sender::new(0),
                registrations: Notify::new(),
                device_id: RwLock::new(None),
                invitation_failures: AtomicU32::new(0),
                observer: RwLock::new(None),
                punch_failures: Mutex::new(HashMap::new()),
                port_mapping: std::sync::atomic::AtomicBool::new(false),
                reported: Mutex::new(None),
            }),
        }
    }

    /// Set or clear the relay address. Unchanged values do not reconnect.
    ///
    /// # Errors
    ///
    /// [`RelayConfigError::Invalid`] for an address that is not a relay URL;
    /// the previous configuration stays in place.
    pub fn configure(&self, value: Option<&str>) -> Result<(), RelayConfigError> {
        let config = value
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(RelayConfig::parse)
            .transpose()?
            .map(Arc::new);
        self.inner.config.send_if_modified(|current| {
            let changed = *current != config;
            if changed {
                *current = config;
            }
            changed
        });
        Ok(())
    }

    /// Allow or forbid asking the router (UPnP) to forward direct-path ports.
    pub fn set_port_mapping(&self, allowed: bool) {
        self.inner.port_mapping.store(allowed, Ordering::Relaxed);
    }

    #[must_use]
    pub fn status(&self) -> RelayStatus {
        *self.inner.status.borrow()
    }

    #[must_use]
    pub fn subscribe_status(&self) -> watch::Receiver<RelayStatus> {
        self.inner.status.subscribe()
    }

    /// Changes whenever a paired device appears on or leaves the relay.
    #[must_use]
    pub fn subscribe_online(&self) -> watch::Receiver<u64> {
        self.inner.online_changes.subscribe()
    }

    fn config(&self) -> Option<Arc<RelayConfig>> {
        self.inner.config.borrow().clone()
    }

    /// A route to the peer holding `psk`, when that peer is on the relay now.
    pub(crate) fn route(&self, psk: &[u8; TOKEN_LEN]) -> Option<RelayRoute> {
        let config = self.config()?;
        let id = rendezvous_id(psk);
        if !self
            .inner
            .online
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .contains(&id)
        {
            return None;
        }
        let device_id = self
            .inner
            .device_id
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let slot = device_id.map_or_else(random_slot, |device_id| slot(psk, &device_id));
        Some(RelayRoute {
            config,
            id,
            slot,
            punch: Some(Arc::clone(&self.inner)),
        })
    }

    /// A route to whichever device opened the invitation behind `code`.
    pub(crate) fn invitation_route(&self, code: &PairingCode) -> Option<RelayRoute> {
        Some(RelayRoute {
            config: self.config()?,
            id: invitation_id(code),
            slot: random_slot(),
            punch: None,
        })
    }

    pub(crate) fn invitation_id(&self, code: &PairingCode) -> Option<Id> {
        (self.inner.invitation_failures.load(Ordering::Relaxed) < MAX_INVITATION_FAILURES)
            .then(|| invitation_id(code))
    }

    pub(crate) fn registrations_changed(&self) {
        self.inner.registrations.notify_one();
    }

    pub(crate) fn note_invitation_failure(&self) {
        if self
            .inner
            .invitation_failures
            .fetch_add(1, Ordering::Relaxed)
            + 1
            >= MAX_INVITATION_FAILURES
        {
            self.registrations_changed();
        }
    }

    pub(crate) fn reset_invitation_failures(&self) {
        self.inner.invitation_failures.store(0, Ordering::Relaxed);
    }

    /// Watchers wake only on a real change, so an idle relay never notifies.
    fn set_status(&self, status: RelayStatus) {
        let changed = self.inner.status.send_if_modified(|current| {
            let changed = *current != status;
            *current = status;
            changed
        });
        if changed {
            self.report(status);
        }
    }

    /// Logs settled transitions once; a reconnect loop that keeps failing
    /// the same way stays quiet until the outcome changes.
    fn report(&self, status: RelayStatus) {
        if status == RelayStatus::Connecting {
            debug!("connecting to the relay");
            return;
        }
        let mut reported = self
            .inner
            .reported
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if *reported == Some(status) {
            return;
        }
        *reported = Some(status);
        match status {
            RelayStatus::Off => info!("relay off"),
            RelayStatus::Connected => info!("relay connected"),
            RelayStatus::Unauthorized => error!("the relay refused this device's token"),
            RelayStatus::Unreachable => warn!("relay unreachable; retrying"),
            RelayStatus::Connecting => {}
        }
    }

    fn set_online(&self, ids: HashSet<Id>) {
        let mut online = self
            .inner
            .online
            .write()
            .unwrap_or_else(|error| error.into_inner());
        if *online != ids {
            *online = ids;
            drop(online);
            self.inner.online_changes.send_modify(|count| *count += 1);
        }
    }

    /// Keep this device present on the configured relay until shutdown,
    /// handing every relayed session that reaches it to `incoming`.
    pub(crate) async fn run<R>(
        self,
        device_id: String,
        registrations: R,
        incoming: mpsc::Sender<Conn>,
        mut shutdown: watch::Receiver<bool>,
    ) where
        R: Fn() -> Registrations + Send + Sync,
    {
        *self
            .inner
            .device_id
            .write()
            .unwrap_or_else(|error| error.into_inner()) = Some(device_id.clone());
        let mut config_changes = self.inner.config.subscribe();
        let accepts = Arc::new(Semaphore::new(MAX_ACCEPTS_IN_FLIGHT));
        let mut backoff = copypaste_retry::stream_reconnect_backoff().build();
        loop {
            if *shutdown.borrow() {
                break;
            }
            let Some(config) = config_changes.borrow_and_update().clone() else {
                self.set_status(RelayStatus::Off);
                self.set_online(HashSet::new());
                tokio::select! {
                    _ = shutdown.changed() => break,
                    _ = config_changes.changed() => continue,
                }
            };
            self.set_status(RelayStatus::Connecting);
            let started = Instant::now();
            let end = self
                .presence(
                    &config,
                    &device_id,
                    &registrations,
                    &incoming,
                    &accepts,
                    &mut shutdown,
                    &mut config_changes,
                )
                .await;
            self.set_online(HashSet::new());
            self.inner.set_observer(None);
            let status = match end {
                End::Shutdown => break,
                End::Reconfigured => {
                    backoff = copypaste_retry::stream_reconnect_backoff().build();
                    continue;
                }
                End::Failed(status) => status,
            };
            self.set_status(status);
            if started.elapsed() >= STABLE_CONNECTION {
                backoff = copypaste_retry::stream_reconnect_backoff().build();
            }
            let delay = backoff
                .next()
                .unwrap_or(copypaste_retry::STREAM_RECONNECT_MAX);
            debug!(?delay, "relay presence ended; reconnecting later");
            tokio::select! {
                _ = shutdown.changed() => break,
                _ = config_changes.changed() => {}
                () = sleep(delay) => {}
            }
        }
        self.set_online(HashSet::new());
        self.set_status(RelayStatus::Off);
    }

    #[allow(clippy::too_many_arguments)]
    async fn presence<R>(
        &self,
        config: &Arc<RelayConfig>,
        device_id: &str,
        registrations: &R,
        incoming: &mpsc::Sender<Conn>,
        accepts: &Arc<Semaphore>,
        shutdown: &mut watch::Receiver<bool>,
        config_changes: &mut watch::Receiver<Option<Arc<RelayConfig>>>,
    ) -> End
    where
        R: Fn() -> Registrations,
    {
        let mut socket = match open_socket(config).await {
            Ok(socket) => socket,
            Err(_) => return End::Failed(RelayStatus::Unreachable),
        };
        let hello = Hello::Presence {
            token: config.token.clone(),
        };
        if send(&mut socket, &hello).await.is_err() {
            return End::Failed(RelayStatus::Unreachable);
        }
        let nonce = match timeout(CONNECT_TIMEOUT, next_event(&mut socket)).await {
            Ok(Some(Event::Challenge { nonce, observe })) => {
                self.inner
                    .set_observer(observe.and_then(|observe| config.observer(&observe)));
                nonce
            }
            Ok(Some(Event::Error {
                code: ErrorCode::Unauthorized,
            })) => return End::Failed(RelayStatus::Unauthorized),
            _ => return End::Failed(RelayStatus::Unreachable),
        };
        let mut sent = Vec::new();
        let mut ping = interval(PING_INTERVAL);
        ping.set_missed_tick_behavior(MissedTickBehavior::Delay);
        ping.reset();
        let mut heard = Instant::now();
        let mut first = true;
        let (answers, mut answered) = mpsc::channel::<(Ticket, Vec<std::net::SocketAddr>)>(4);
        loop {
            let refresh = first
                || {
                    tokio::select! {
                        biased;
                        _ = shutdown.changed() => {
                            let _ = socket.close(None).await;
                            return End::Shutdown;
                        }
                        _ = config_changes.changed() => {
                            let _ = socket.close(None).await;
                            return End::Reconfigured;
                        }
                        () = self.inner.registrations.notified() => true,
                        Some((ticket, candidates)) = answered.recv() => {
                            let answer = Control::PunchAnswer { ticket, candidates };
                            let Ok(text) = serde_json::to_string(&answer) else {
                                return End::Failed(RelayStatus::Unreachable);
                            };
                            if socket.send(Message::Text(text)).await.is_err() {
                                return End::Failed(RelayStatus::Unreachable);
                            }
                            false
                        }
                        _ = ping.tick() => {
                            if heard.elapsed() > SILENCE_LIMIT
                                || socket.send(Message::Ping(Vec::new())).await.is_err()
                            {
                                return End::Failed(RelayStatus::Unreachable);
                            }
                            true
                        }
                        message = socket.next() => {
                            heard = Instant::now();
                            match message {
                                Some(Ok(Message::Text(text))) => {
                                    match parse_event(&text) {
                                        Some(Event::Online { ids }) => {
                                            self.set_online(ids.into_iter().map(|HexId(id)| id).collect());
                                        }
                                        Some(Event::Incoming { ticket }) => {
                                            if let Ok(permit) = Arc::clone(accepts).try_acquire_owned() {
                                                let config = Arc::clone(config);
                                                let incoming = incoming.clone();
                                                tokio::spawn(async move {
                                                    let _permit = permit;
                                                    if let Ok(conn) = accept(&config, ticket).await {
                                                        let _ = incoming.send(conn).await;
                                                    }
                                                });
                                            }
                                        }
                                        Some(Event::Punch { ticket, candidates }) => {
                                            self.answer_punch(ticket, candidates, accepts, &answers, incoming);
                                        }
                                        Some(Event::Error { code: ErrorCode::Unauthorized }) => {
                                            return End::Failed(RelayStatus::Unauthorized);
                                        }
                                        _ => return End::Failed(RelayStatus::Unreachable),
                                    }
                                    false
                                }
                                Some(Ok(Message::Ping(_) | Message::Pong(_))) => false,
                                _ => return End::Failed(RelayStatus::Unreachable),
                            }
                        }
                    }
                };
            if refresh {
                let register = build_register(&nonce, device_id, registrations());
                let fingerprint = fingerprint(&register);
                if first || fingerprint != sent {
                    let Ok(text) = serde_json::to_string(&Control::Register(register)) else {
                        return End::Failed(RelayStatus::Unreachable);
                    };
                    if socket.send(Message::Text(text)).await.is_err() {
                        return End::Failed(RelayStatus::Unreachable);
                    }
                    sent = fingerprint;
                }
                if first {
                    first = false;
                    self.set_status(RelayStatus::Connected);
                }
            }
        }
    }
}

impl RelayClient {
    /// Answer a punch offer: learn this side's public address, hand it back
    /// through the presence socket, then accept the peer's direct connection.
    /// An empty answer lets the dialler fall back at once.
    fn answer_punch(
        &self,
        ticket: Ticket,
        offered: Vec<std::net::SocketAddr>,
        accepts: &Arc<Semaphore>,
        answers: &mpsc::Sender<(Ticket, Vec<std::net::SocketAddr>)>,
        incoming: &mpsc::Sender<Conn>,
    ) {
        let observer = self
            .inner
            .observer
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let (Some(observer), Ok(permit)) = (observer, Arc::clone(accepts).try_acquire_owned())
        else {
            let _ = answers.try_send((ticket, Vec::new()));
            return;
        };
        let answers = answers.clone();
        let incoming = incoming.clone();
        let map_port = self.inner.port_mapping.load(Ordering::Relaxed);
        tokio::spawn(async move {
            let _permit = permit;
            let Ok(prepared) = punch::prepare(&observer, map_port).await else {
                let _ = answers.send((ticket, Vec::new())).await;
                return;
            };
            if answers
                .send((ticket, prepared.candidates.clone()))
                .await
                .is_err()
            {
                return;
            }
            if let Ok(conn) = punch::answer(prepared, offered).await {
                let _ = incoming.send(conn).await;
            }
        });
    }
}

enum End {
    Shutdown,
    Reconfigured,
    Failed(RelayStatus),
}

fn build_register(nonce: &[u8; 32], device_id: &str, registrations: Registrations) -> Register {
    let signed = registrations
        .psks
        .iter()
        .map(|psk| {
            let key = signing_key(psk);
            let id = key.verifying_key().to_bytes();
            let slot = slot(psk, device_id);
            Signed {
                id,
                slot,
                sig: key.sign(&signed_message(nonce, &id, &slot)).to_bytes(),
            }
        })
        .collect();
    Register {
        signed,
        open: registrations.invitation.map(HexId).into_iter().collect(),
    }
}

fn fingerprint(register: &Register) -> Vec<Id> {
    let mut ids: Vec<Id> = register
        .signed
        .iter()
        .map(|signed| signed.id)
        .chain(register.open.iter().map(|HexId(id)| *id))
        .collect();
    ids.sort_unstable();
    ids
}

async fn accept(config: &RelayConfig, ticket: Ticket) -> Result<Conn, TransportError> {
    timeout(CONNECT_TIMEOUT, async {
        let mut socket = open_socket(config).await?;
        send(
            &mut socket,
            &Hello::Accept {
                token: config.token.clone(),
                ticket,
            },
        )
        .await?;
        await_ready(&mut socket).await?;
        let addr = relay_addr(&socket);
        Ok(Conn::relayed(Box::new(WsFrames(socket)), addr))
    })
    .await
    .unwrap_or(Err(TransportError::Handshake))
}

fn ws_config() -> WebSocketConfig {
    WebSocketConfig {
        max_message_size: Some(MAX_FRAME_BYTES),
        max_frame_size: Some(MAX_FRAME_BYTES),
        ..WebSocketConfig::default()
    }
}

async fn open_socket(config: &RelayConfig) -> Result<Socket, TransportError> {
    match timeout(
        CONNECT_TIMEOUT,
        tokio_tungstenite::connect_async_with_config(config.url.as_str(), Some(ws_config()), true),
    )
    .await
    {
        Ok(Ok((socket, _))) => Ok(socket),
        Ok(Err(error)) => {
            debug!(%error, "could not reach the relay");
            Err(TransportError::Io(std::io::Error::other(
                "relay unreachable",
            )))
        }
        Err(_) => Err(TransportError::Handshake),
    }
}

async fn send(socket: &mut Socket, hello: &Hello) -> Result<(), TransportError> {
    let text = serde_json::to_string(hello).map_err(|_| TransportError::Codec)?;
    socket
        .send(Message::Text(text))
        .await
        .map_err(|error| TransportError::Io(std::io::Error::other(error)))
}

async fn await_ready(socket: &mut Socket) -> Result<(), TransportError> {
    match next_event(socket).await {
        Some(Event::Ready) => Ok(()),
        _ => Err(TransportError::Handshake),
    }
}

async fn next_event(socket: &mut Socket) -> Option<Event> {
    loop {
        match socket.next().await? {
            Ok(Message::Text(text)) => return parse_event(&text),
            Ok(Message::Ping(_) | Message::Pong(_)) => {}
            _ => return None,
        }
    }
}

fn parse_event(text: &str) -> Option<Event> {
    (text.len() <= MAX_CONTROL_BYTES)
        .then(|| serde_json::from_str(text).ok())
        .flatten()
}

fn signing_key(psk: &[u8; TOKEN_LEN]) -> SigningKey {
    let mut seed = Zeroizing::new([0u8; 32]);
    Hkdf::<Sha256>::new(None, psk)
        .expand(b"copypaste/relay/v1/rendezvous", seed.as_mut_slice())
        .expect("32 bytes is a valid HKDF-SHA256 output length");
    SigningKey::from_bytes(&seed)
}

/// The relay routing key for one pairing: an Ed25519 public key whose private
/// half only the two paired devices can derive.
pub(crate) fn rendezvous_id(psk: &[u8; TOKEN_LEN]) -> Id {
    signing_key(psk).verifying_key().to_bytes()
}

/// Distinguishes the two devices of a pairing on the relay, and stays stable
/// across reconnects so a new presence replaces a stale one.
fn slot(psk: &[u8; TOKEN_LEN], device_id: &str) -> Slot {
    let mut slot = [0u8; 16];
    Hkdf::<Sha256>::new(None, psk)
        .expand_multi_info(
            &[b"copypaste/relay/v1/slot", device_id.as_bytes()],
            &mut slot,
        )
        .expect("16 bytes is a valid HKDF-SHA256 output length");
    slot
}

fn invitation_id(code: &PairingCode) -> Id {
    Sha256::new()
        .chain_update(b"copypaste/relay/v1/invitation")
        .chain_update([code.nameplate()])
        .finalize()
        .into()
}

fn random_slot() -> Slot {
    let mut slot = [0u8; 16];
    OsRng.fill_bytes(&mut slot);
    slot
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_accepts_relay_urls_and_keeps_the_token_out_of_the_url() {
        let config = RelayConfig::parse(" wss://relay.example.com/sync?token=s3cret#x ").unwrap();
        assert_eq!(config.url, "wss://relay.example.com/sync");
        assert_eq!(config.token.as_deref(), Some("s3cret"));
        assert!(!format!("{config:?}").contains("s3cret"));
        for invalid in [
            "https://relay.example.com",
            "ws://",
            "ws://user:pw@host",
            "relay",
        ] {
            assert_eq!(RelayConfig::parse(invalid), Err(RelayConfigError::Invalid));
        }
    }

    #[test]
    fn both_devices_derive_the_same_id_but_distinct_slots() {
        let psk = [5u8; TOKEN_LEN];
        assert_eq!(rendezvous_id(&psk), rendezvous_id(&psk));
        assert_ne!(rendezvous_id(&psk), rendezvous_id(&[6u8; TOKEN_LEN]));
        assert_ne!(slot(&psk, "a"), slot(&psk, "b"));
        let register = build_register(
            &[1; 32],
            "a",
            Registrations {
                psks: vec![Zeroizing::new(psk)],
                invitation: None,
            },
        );
        assert!(copypaste_relay::proto::verify(
            &[1; 32],
            &register.signed[0]
        ));
    }
}

//! A direct path across NATs: both devices learn their public UDP address
//! from the relay's observer, trade it through the relay, open their NATs
//! towards each other and connect with QUIC from the same socket.
//!
//! When the user allows it, the device also asks its router (UPnP IGD) to
//! forward the socket's port for a short lease; the mapped address is one more
//! candidate, and the one that also crosses symmetric NATs.
//!
//! QUIC only provides the reliable stream. Its TLS uses a throwaway
//! self-signed certificate that nobody verifies, because the Noise session
//! that runs inside the stream is what authenticates the peer.

use std::io;
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll};
use std::time::Duration;

use copypaste_relay::proto::{
    observe_request, parse_observe_reply, Event, Hello, Id, Slot, MAX_CANDIDATES,
    OBSERVE_REQUEST_BYTES,
};
use futures_util::{Sink, Stream};
use quinn::crypto::rustls::{QuicClientConfig, QuicServerConfig};
use rand::{rngs::OsRng, RngCore};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use tokio::net::{lookup_host, UdpSocket};
use tokio::time::timeout;
use tokio_util::bytes::{Bytes, BytesMut};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use super::{next_event, open_socket, send, RelayConfig};
use crate::transport::{codec, Conn, TransportError};

/// The whole budget for one punch before the spliced path takes over.
pub(super) const PUNCH_BUDGET: Duration = Duration::from_secs(5);
const OBSERVE_ATTEMPTS: usize = 3;
const OBSERVE_WAIT: Duration = Duration::from_millis(600);
/// Datagrams sent to each peer candidate to open this side's NAT.
const HOLE_PACKETS: usize = 3;
const ALPN: &[u8] = b"copypaste-p2p/1";
const SERVER_NAME: &str = "copypaste";
/// A router mapping outlives one punch and then lapses on its own.
const MAPPING_LEASE_SECS: u32 = 120;
const MAPPING_BUDGET: Duration = Duration::from_millis(1500);

/// A UDP socket with the public address the relay saw it come from.
pub(super) struct Prepared {
    socket: UdpSocket,
    pub candidates: Vec<SocketAddr>,
}

/// Bind a fresh socket and ask the observer at `observe` where it appears;
/// with `map_port`, also ask the router to forward it.
pub(super) async fn prepare(observe: &str, map_port: bool) -> io::Result<Prepared> {
    let observer = lookup_host(observe)
        .await?
        .find(SocketAddr::is_ipv4)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no IPv4 observer"))?;
    let socket = UdpSocket::bind(("0.0.0.0", 0)).await?;
    let mut nonce = [0u8; 16];
    OsRng.fill_bytes(&mut nonce);
    let request = observe_request(&nonce);
    let mut reply = [0u8; OBSERVE_REQUEST_BYTES];
    for _ in 0..OBSERVE_ATTEMPTS {
        socket.send_to(&request, observer).await?;
        if let Ok(Ok((len, from))) = timeout(OBSERVE_WAIT, socket.recv_from(&mut reply)).await {
            if from == observer {
                if let Some(public) = parse_observe_reply(&reply[..len], &nonce) {
                    let mut candidates = vec![public];
                    if map_port {
                        let port = socket.local_addr()?.port();
                        match router_mapping(port).await {
                            Some(mapped) => {
                                tracing::debug!("the router forwarded a direct-path port");
                                if mapped != public {
                                    candidates.push(mapped);
                                }
                            }
                            None => tracing::debug!("the router did not forward a port"),
                        }
                    }
                    return Ok(Prepared { socket, candidates });
                }
            }
        }
    }
    Err(io::ErrorKind::TimedOut.into())
}

/// Ask the router for a UDP mapping of `port` to this host, returning the
/// public address it forwards. `None` when there is no IGD or it refuses.
async fn router_mapping(port: u16) -> Option<SocketAddr> {
    let Some(IpAddr::V4(local)) = crate::netif::routable_ip() else {
        return None;
    };
    timeout(MAPPING_BUDGET, async {
        let mut options = igd_next::SearchOptions::default();
        options.timeout = Some(MAPPING_BUDGET);
        let gateway = igd_next::aio::tokio::search_gateway(options).await.ok()?;
        let external = gateway.get_external_ip().await.ok()?;
        gateway
            .add_port(
                igd_next::PortMappingProtocol::UDP,
                port,
                SocketAddr::new(local.into(), port),
                MAPPING_LEASE_SECS,
                "CopyPaste sync",
            )
            .await
            .ok()?;
        Some(SocketAddr::new(external, port))
    })
    .await
    .ok()
    .flatten()
}

/// Offer this device's candidates to the device on `id` and connect to it.
pub(super) async fn dial(
    config: &RelayConfig,
    observe: &str,
    map_port: bool,
    id: Id,
    slot: Slot,
) -> Result<Conn, TransportError> {
    let prepared = prepare(observe, map_port)
        .await
        .map_err(TransportError::Io)?;
    let mut signal = open_socket(config).await?;
    send(
        &mut signal,
        &Hello::Punch {
            token: config.token.clone(),
            id,
            slot,
            candidates: prepared.candidates.clone(),
        },
    )
    .await?;
    let Some(Event::PunchAnswer { candidates }) = next_event(&mut signal).await else {
        return Err(TransportError::Handshake);
    };
    drop(signal);
    let peers = usable(candidates);
    if peers.is_empty() {
        return Err(TransportError::Handshake);
    }
    open_holes(&prepared.socket, &peers).await;
    let mut endpoint = endpoint(prepared.socket, None)?;
    endpoint.set_default_client_config(client_config()?);
    let attempts = peers.iter().map(|peer| {
        let connecting = endpoint.connect(*peer, SERVER_NAME);
        Box::pin(async move {
            connecting
                .map_err(|_| TransportError::Handshake)?
                .await
                .map_err(|_| TransportError::Handshake)
        })
    });
    let (connection, _) = futures_util::future::select_ok(attempts).await?;
    let (send, recv) = connection
        .open_bi()
        .await
        .map_err(|_| TransportError::Handshake)?;
    Ok(conn(endpoint, connection, send, recv))
}

/// Answer a peer's offer: open this side's NAT towards it, then accept its
/// QUIC connection. The caller has already handed back this side's
/// candidates from the same prepared socket.
pub(super) async fn answer(
    prepared: Prepared,
    offered: Vec<SocketAddr>,
) -> Result<Conn, TransportError> {
    let peers = usable(offered);
    open_holes(&prepared.socket, &peers).await;
    let endpoint = endpoint(prepared.socket, Some(server_config()?))?;
    let accepted = timeout(PUNCH_BUDGET, async {
        while let Some(incoming) = endpoint.accept().await {
            // Anyone can send a QUIC Initial here; only the offered peer is
            // worth a handshake, and Noise still decides who it really is.
            if !peers.contains(&incoming.remote_address()) {
                incoming.ignore();
                continue;
            }
            let connection = incoming.await.map_err(|_| TransportError::Handshake)?;
            let (send, recv) = connection
                .accept_bi()
                .await
                .map_err(|_| TransportError::Handshake)?;
            return Ok((connection, send, recv));
        }
        Err(TransportError::Handshake)
    })
    .await
    .unwrap_or(Err(TransportError::Handshake))?;
    let (connection, send, recv) = accepted;
    Ok(conn(endpoint, connection, send, recv))
}

fn usable(candidates: Vec<SocketAddr>) -> Vec<SocketAddr> {
    candidates
        .into_iter()
        .filter(|candidate| candidate.is_ipv4() && candidate.port() != 0)
        .take(MAX_CANDIDATES)
        .collect()
}

async fn open_holes(socket: &UdpSocket, peers: &[SocketAddr]) {
    for _ in 0..HOLE_PACKETS {
        for peer in peers {
            let _ = socket.send_to(&[0], peer).await;
        }
    }
}

fn endpoint(
    socket: UdpSocket,
    server: Option<quinn::ServerConfig>,
) -> Result<quinn::Endpoint, TransportError> {
    let runtime = quinn::default_runtime()
        .ok_or_else(|| TransportError::Io(io::Error::other("no async runtime")))?;
    quinn::Endpoint::new(
        quinn::EndpointConfig::default(),
        server,
        socket.into_std().map_err(TransportError::Io)?,
        runtime,
    )
    .map_err(TransportError::Io)
}

fn conn(
    endpoint: quinn::Endpoint,
    connection: quinn::Connection,
    send: quinn::SendStream,
    recv: quinn::RecvStream,
) -> Conn {
    let peer = connection.remote_address();
    Conn::relayed(
        Box::new(QuicFrames {
            framed: Framed::new(tokio::io::join(recv, send), codec()),
            connection,
            _endpoint: endpoint,
        }),
        peer,
    )
}

/// One QUIC stream as Noise frames. The endpoint and connection live exactly
/// as long as the session does.
struct QuicFrames {
    framed: Framed<tokio::io::Join<quinn::RecvStream, quinn::SendStream>, LengthDelimitedCodec>,
    connection: quinn::Connection,
    _endpoint: quinn::Endpoint,
}

impl Drop for QuicFrames {
    fn drop(&mut self) {
        self.connection.close(0u32.into(), b"");
    }
}

impl Stream for QuicFrames {
    type Item = io::Result<BytesMut>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.framed).poll_next(cx)
    }
}

impl Sink<Bytes> for QuicFrames {
    type Error = io::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.framed).poll_ready(cx)
    }

    fn start_send(mut self: Pin<&mut Self>, item: Bytes) -> io::Result<()> {
        Pin::new(&mut self.framed).start_send(item)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.framed).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.framed).poll_close(cx)
    }
}

fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

fn transport() -> Arc<quinn::TransportConfig> {
    let mut transport = quinn::TransportConfig::default();
    transport
        .max_idle_timeout(Some(
            Duration::from_secs(30)
                .try_into()
                .expect("30 s is a valid QUIC idle timeout"),
        ))
        .keep_alive_interval(Some(Duration::from_secs(10)))
        .max_concurrent_bidi_streams(1u32.into())
        .max_concurrent_uni_streams(0u32.into());
    Arc::new(transport)
}

fn tls_error(_: impl std::fmt::Debug) -> TransportError {
    TransportError::Handshake
}

fn client_config() -> Result<quinn::ClientConfig, TransportError> {
    let provider = provider();
    let mut tls = rustls::ClientConfig::builder_with_provider(Arc::clone(&provider))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(tls_error)?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoiseAuthenticates(provider)))
        .with_no_client_auth();
    tls.alpn_protocols = vec![ALPN.to_vec()];
    let mut config = quinn::ClientConfig::new(Arc::new(
        QuicClientConfig::try_from(tls).map_err(tls_error)?,
    ));
    config.transport_config(transport());
    Ok(config)
}

/// One self-signed certificate per process, made on the first answered punch.
fn server_config() -> Result<quinn::ServerConfig, TransportError> {
    static CONFIG: OnceLock<Option<quinn::ServerConfig>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let key = rcgen::generate_simple_self_signed(vec![SERVER_NAME.to_string()]).ok()?;
            let certificate = key.cert.der().clone();
            let private = PrivatePkcs8KeyDer::from(key.key_pair.serialize_der());
            let mut tls = rustls::ServerConfig::builder_with_provider(provider())
                .with_protocol_versions(&[&rustls::version::TLS13])
                .ok()?
                .with_no_client_auth()
                .with_single_cert(vec![certificate], private.into())
                .ok()?;
            tls.alpn_protocols = vec![ALPN.to_vec()];
            let mut config =
                quinn::ServerConfig::with_crypto(Arc::new(QuicServerConfig::try_from(tls).ok()?));
            config.transport_config(transport());
            Some(config)
        })
        .clone()
        .ok_or(TransportError::Handshake)
}

/// Accepts any certificate: the Noise session inside the stream is the only
/// authentication, so QUIC's certificate proves nothing worth checking.
#[derive(Debug)]
struct NoiseAuthenticates(Arc<CryptoProvider>);

impl ServerCertVerifier for NoiseAuthenticates {
    fn verify_server_cert(
        &self,
        _: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

use std::net::SocketAddr;
use std::time::Duration;

use clap::Parser;
use copypaste_relay::{observe, serve, Config};
use tokio::net::{TcpListener, UdpSocket};
use tracing_subscriber::EnvFilter;

/// CopyPaste sync relay. Terminate TLS in a reverse proxy for `wss://`.
#[derive(Parser)]
#[command(version)]
struct Args {
    #[arg(long, env = "COPYPASTE_RELAY_LISTEN", default_value = "0.0.0.0:8080")]
    listen: SocketAddr,
    /// Shared secret clients must present (`?token=` in the relay URL).
    #[arg(long, env = "COPYPASTE_RELAY_TOKEN")]
    token: Option<String>,
    #[arg(long, env = "COPYPASTE_RELAY_MAX_CONNECTIONS", default_value_t = 1024)]
    max_connections: usize,
    #[arg(long, env = "COPYPASTE_RELAY_MAX_PER_IP", default_value_t = 32)]
    max_connections_per_ip: usize,
    /// Total MiB one relayed session may forward.
    #[arg(long, env = "COPYPASTE_RELAY_SESSION_MIB", default_value_t = 256)]
    session_mib: u64,
    /// KiB per second per session direction; 0 disables the limit.
    #[arg(long, env = "COPYPASTE_RELAY_RATE_KIB", default_value_t = 4096)]
    rate_kib: u64,
    /// UDP address of the observer that lets devices connect directly; empty
    /// turns direct paths off.
    #[arg(
        long,
        env = "COPYPASTE_RELAY_UDP_LISTEN",
        default_value = "0.0.0.0:3478"
    )]
    udp_listen: String,
    /// Where devices reach the observer when it differs from the relay host,
    /// as `host:port` (for example when TLS goes through a proxy).
    #[arg(long, env = "COPYPASTE_RELAY_UDP_PUBLIC")]
    udp_public: Option<String>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let args = Args::parse();
    let listener = TcpListener::bind(args.listen).await?;
    let udp = if args.udp_listen.is_empty() {
        None
    } else {
        Some(UdpSocket::bind(&args.udp_listen).await?)
    };
    let observe_at = match (&udp, args.udp_public.filter(|public| !public.is_empty())) {
        (Some(_), Some(public)) => Some(public),
        (Some(socket), None) => Some(format!(":{}", socket.local_addr()?.port())),
        (None, _) => None,
    };
    let (stop, stopped) = tokio::sync::watch::channel(false);
    if let Some(socket) = udp {
        let mut stopped = stopped.clone();
        tokio::spawn(observe(socket, async move {
            let _ = stopped.changed().await;
        }));
    }
    let config = Config {
        token: args.token.filter(|token| !token.is_empty()),
        max_connections: args.max_connections,
        max_connections_per_ip: args.max_connections_per_ip,
        session_bytes: args.session_mib.saturating_mul(1024 * 1024),
        session_bytes_per_sec: args.rate_kib.saturating_mul(1024),
        observe: observe_at,
        ..Config::default()
    };
    serve(listener, config, async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await;
    let _ = stop.send(true);
    // Give closing sockets a moment to flush their close frames.
    tokio::time::sleep(Duration::from_millis(50)).await;
    Ok(())
}

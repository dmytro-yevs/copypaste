# copypaste-p2p

## Purpose
Direct device-to-device transport for CopyPaste: mutual-TLS over TCP, mDNS-SD peer discovery, and PAKE-based pairing. No relay server required when peers are on the same LAN.

## Public API
From `src/lib.rs`:

- TLS transport — `SelfSignedCert`, `CertError`, `fingerprint_of`; `PeerTransport`, `PeerStream`, `PeerClientStream`, `PairedPeers`, `DeviceFingerprint`, `TransportError`.
- mDNS-SD discovery — `DiscoveryService`, `PeerInfo`, `SERVICE_TYPE`, `DiscoveryError`.
- PAKE pairing (ADR-008) — `PakeInitiator`, `PakeResponder`, `PasswordFile`, `SessionKey`, `PakeError`.
- `DEFAULT_P2P_PORT = 51515`.

Each device generates a self-signed X.509 certificate at first run; its SHA-256 fingerprint is the device identity. Peers exchange fingerprints out-of-band (QR / relay), then validate via mutual TLS.

## Platform support
All platforms.

## Status
beta.

## Internal vs published
Internal workspace crate. Not published to crates.io.

## Quick example

```rust,no_run
use copypaste_p2p::{PeerTransport, PairedPeers, SelfSignedCert, DiscoveryService};
use tokio::net::TcpListener;

# async fn example() -> anyhow::Result<()> {
let my_cert = SelfSignedCert::generate("my-device-id")?;
let mut peers = PairedPeers::new();
peers.add("abc123...peer_fp...", "Alice's MacBook");

let transport = PeerTransport::from_cert(my_cert.cert_der, my_cert.key_der, peers);
let listener = TcpListener::bind("0.0.0.0:51515").await?;
let (_addr, _stream) = transport.accept(&listener).await?;
# Ok(())
# }
```

## Tests
5 integration tests under `tests/`: disconnect handling, mDNS discovery, network scan, PAKE roundtrip, TLS mutual auth.

```bash
cargo test -p copypaste-p2p
```

## Related ADRs
- [ADR-008](../../docs/adr/ADR-008-pake-protocol-choice.md) — PAKE protocol choice.

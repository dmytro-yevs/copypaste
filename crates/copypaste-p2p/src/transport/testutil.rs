//! Helpers shared by the transport's test modules.
//!
//! Test-only. It exists because two properties are checked from more than one
//! file: that no rendering of a secret ever reaches a string (`assert_no_secret`
//! is used by both the token's `Debug` test and the session's), and that a
//! loopback listener is set up identically everywhere.

use std::net::SocketAddr;

use tokio::net::TcpListener;

use super::PairingToken;

/// A small serialisable message, for exercising send/recv.
#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) struct Ping {
    pub seq: u64,
    pub note: String,
}

/// Bind a listener on loopback and hand back its address.
pub(super) async fn loopback() -> (TcpListener, SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback bind");
    let addr = listener.local_addr().expect("local addr");
    (listener, addr)
}

/// Fails if `text` contains the token in any encoding this codebase uses.
pub(super) fn assert_no_secret(text: &str, token: &PairingToken) {
    let psk = token.psk();
    assert!(!text.contains(&hex::encode(psk)), "leaked hex token");
    assert!(
        !text.contains(&hex::encode_upper(psk)),
        "leaked upper-hex token"
    );
    assert!(!text.contains(&token.to_code()), "leaked pairing code");
    // Rust's default `{:?}` for a byte array.
    let debug_bytes = format!("{:?}", &psk[..]);
    assert!(!text.contains(&debug_bytes), "leaked debug byte array");
    // Any run of four consecutive token bytes rendered as decimal, which is
    // what a derived Debug on `[u8; 32]` would produce.
    let decimal = psk
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    assert!(!text.contains(&decimal), "leaked decimal byte array");
}

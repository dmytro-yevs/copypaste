//! Best-effort public / WAN IP discovery via STUN.
//!
//! Sends a single RFC 5389 STUN Binding Request to a public STUN server
//! (`stun.l.google.com:19302` by default) over UDP, then parses the
//! XOR-MAPPED-ADDRESS attribute from the response to learn the reflexive
//! (NAT-external) address.
//!
//! ## Why STUN, not an HTTP echo?
//! * No new crate dependencies: uses only `std::net::UdpSocket` + stdlib.
//! * Faster: one UDP round-trip (~20–80 ms LAN-to-Google) vs an HTTPS
//!   connection + TLS handshake.
//! * Privacy-equivalent: the STUN binding request contains no personal data
//!   (it is a 20-byte probe with a random transaction-ID).
//!
//! ## Opt-out
//! The call site in `daemon.rs` gates this behind
//! `AppConfig::collect_public_ip`. When `false`, this function is never
//! called and `public_ip` remains `None` forever.
//!
//! ## Error handling
//! All errors are logged at `debug` level and return `None` — they MUST NOT
//! propagate to the caller or block startup.

use std::net::UdpSocket;
use std::time::Duration;
use tracing::debug;

/// STUN server used for the Binding Request.
/// Google's servers are stable, publicly documented, and require no auth.
const STUN_SERVER: &str = "stun.l.google.com:19302";

/// Total wall-clock budget for the STUN exchange (connect + send + recv).
/// 5 s is generous; typical round-trips to stun.l.google.com are < 100 ms.
const STUN_TIMEOUT: Duration = Duration::from_secs(5);

/// STUN message type: Binding Request (RFC 5389 §6).
const MSG_TYPE_BINDING_REQUEST: u16 = 0x0001;
/// STUN message type: Binding Success Response.
const MSG_TYPE_BINDING_RESPONSE: u16 = 0x0101;

/// XOR-MAPPED-ADDRESS attribute type (RFC 5389 §15.2).
const ATTR_XOR_MAPPED_ADDRESS: u16 = 0x0020;
/// MAPPED-ADDRESS attribute type (RFC 3489 §11.2, kept as fallback).
const ATTR_MAPPED_ADDRESS: u16 = 0x0001;

/// Magic cookie mandated by RFC 5389 §6.  Also used as the XOR mask for
/// the port in XOR-MAPPED-ADDRESS.
const MAGIC_COOKIE: u32 = 0x2112_A442;

/// Perform a STUN Binding Request and return the reflexive IPv4 address, or
/// `None` on any failure (network unreachable, timeout, parse error, …).
///
/// This is a **blocking** function (uses `UdpSocket` with a read timeout).
/// Call it via `tokio::task::spawn_blocking` in async contexts.
pub fn resolve_public_ip() -> Option<String> {
    resolve_public_ip_via(STUN_SERVER)
}

/// Same as [`resolve_public_ip`] but accepts a custom server address.
/// Extracted so unit tests can drive the logic with a loopback fixture.
pub fn resolve_public_ip_via(server: &str) -> Option<String> {
    // Build the 20-byte STUN Binding Request.
    //   0-1:  message type
    //   2-3:  message length (0 — no attributes in the request)
    //   4-7:  magic cookie
    //   8-19: transaction ID (96-bit random)
    let transaction_id = random_transaction_id();
    let mut req = [0u8; 20];
    req[0..2].copy_from_slice(&MSG_TYPE_BINDING_REQUEST.to_be_bytes());
    req[2..4].copy_from_slice(&0u16.to_be_bytes()); // length = 0
    req[4..8].copy_from_slice(&MAGIC_COOKIE.to_be_bytes());
    req[8..20].copy_from_slice(&transaction_id);

    // Bind to an ephemeral local port; "0.0.0.0:0" lets the OS choose.
    let sock = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(e) => {
            debug!("public_ip: UDP bind failed: {e}");
            return None;
        }
    };
    if let Err(e) = sock.set_read_timeout(Some(STUN_TIMEOUT)) {
        debug!("public_ip: set_read_timeout failed: {e}");
        return None;
    }
    if let Err(e) = sock.connect(server) {
        debug!("public_ip: UDP connect to {server} failed: {e}");
        return None;
    }
    if let Err(e) = sock.send(&req) {
        debug!("public_ip: STUN send failed: {e}");
        return None;
    }

    let mut buf = [0u8; 512];
    let n = match sock.recv(&mut buf) {
        Ok(n) => n,
        Err(e) => {
            debug!("public_ip: STUN recv failed: {e}");
            return None;
        }
    };

    parse_stun_response(&buf[..n], &transaction_id)
}

/// Parse a raw STUN response and extract the reflexive IPv4 address string.
/// Returns `None` for any malformed or unexpected response.
pub(crate) fn parse_stun_response(buf: &[u8], transaction_id: &[u8; 12]) -> Option<String> {
    if buf.len() < 20 {
        debug!("public_ip: STUN response too short ({} bytes)", buf.len());
        return None;
    }

    let msg_type = u16::from_be_bytes([buf[0], buf[1]]);
    if msg_type != MSG_TYPE_BINDING_RESPONSE {
        debug!("public_ip: unexpected STUN message type 0x{msg_type:04x}");
        return None;
    }

    // Verify magic cookie.
    let cookie = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
    if cookie != MAGIC_COOKIE {
        debug!("public_ip: magic cookie mismatch (got 0x{cookie:08x})");
        return None;
    }

    // Verify transaction ID matches what we sent.
    if &buf[8..20] != transaction_id.as_ref() {
        debug!("public_ip: transaction ID mismatch");
        return None;
    }

    let attr_len = u16::from_be_bytes([buf[2], buf[3]]) as usize;
    let payload_end = 20usize.saturating_add(attr_len).min(buf.len());
    let attrs = &buf[20..payload_end];

    // Walk the TLV attribute list; prefer XOR-MAPPED-ADDRESS, fall back to
    // MAPPED-ADDRESS if the former is absent (some older servers).
    let mut fallback: Option<String> = None;
    let mut offset = 0usize;
    while offset + 4 <= attrs.len() {
        let attr_type = u16::from_be_bytes([attrs[offset], attrs[offset + 1]]);
        let value_len = u16::from_be_bytes([attrs[offset + 2], attrs[offset + 3]]) as usize;
        offset += 4;
        if offset + value_len > attrs.len() {
            break;
        }
        let value = &attrs[offset..offset + value_len];

        match attr_type {
            ATTR_XOR_MAPPED_ADDRESS => {
                if let Some(ip) = decode_xor_mapped_address(value) {
                    return Some(ip);
                }
            }
            ATTR_MAPPED_ADDRESS => {
                if let Some(ip) = decode_mapped_address(value) {
                    fallback = Some(ip);
                }
            }
            _ => {}
        }

        // Attributes are padded to 4-byte boundaries.
        offset += (value_len + 3) & !3;
    }

    fallback
}

/// Decode a MAPPED-ADDRESS value (RFC 3489 §11.2).
/// Format: 1 byte reserved | 1 byte family | 2 bytes port | 4 bytes IPv4
fn decode_mapped_address(value: &[u8]) -> Option<String> {
    if value.len() < 8 {
        return None;
    }
    let family = value[1];
    if family != 0x01 {
        // IPv6 (family == 0x02) — not supported for display; return None.
        return None;
    }
    let ip = std::net::Ipv4Addr::new(value[4], value[5], value[6], value[7]);
    Some(ip.to_string())
}

/// Decode an XOR-MAPPED-ADDRESS value (RFC 5389 §15.2).
/// Format: 1 byte reserved | 1 byte family | 2 bytes XOR'd port | 4 bytes XOR'd IPv4
fn decode_xor_mapped_address(value: &[u8]) -> Option<String> {
    if value.len() < 8 {
        return None;
    }
    let family = value[1];
    if family != 0x01 {
        // IPv6 — not supported for display.
        return None;
    }
    // Port is XOR'd with the high 16 bits of the magic cookie (0x2112).
    // Address bytes are XOR'd with the full magic cookie.
    let cookie_bytes = MAGIC_COOKIE.to_be_bytes();
    let b0 = value[4] ^ cookie_bytes[0];
    let b1 = value[5] ^ cookie_bytes[1];
    let b2 = value[6] ^ cookie_bytes[2];
    let b3 = value[7] ^ cookie_bytes[3];
    let ip = std::net::Ipv4Addr::new(b0, b1, b2, b3);
    Some(ip.to_string())
}

/// Generate a 12-byte cryptographically adequate random transaction ID using
/// only `std` (no external crate).  We use the current timestamp combined
/// with a thread-local counter and the stack address of a local variable as
/// entropy sources.  This is sufficient for correlation purposes; STUN
/// transaction IDs are NOT a security primitive.
fn random_transaction_id() -> [u8; 12] {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    // Mix in a thread-local counter so rapid successive calls differ.
    thread_local! {
        static COUNTER: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    }
    let counter = COUNTER.with(|c| {
        let v = c.get().wrapping_add(1);
        c.set(v);
        v
    });
    // Stack address as additional entropy.
    let mut tid = [0u8; 12];
    let addr_bits = &tid as *const _ as u64;
    let mix = ts as u64 ^ addr_bits ^ ((counter as u64) << 32);
    tid[0..8].copy_from_slice(&mix.to_le_bytes());
    tid[8..12].copy_from_slice(&counter.to_be_bytes());
    tid
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A well-formed XOR-MAPPED-ADDRESS response for 203.0.113.42.
    ///
    /// The XOR encoding:
    ///   cookie bytes: [0x21, 0x12, 0xA4, 0x42]
    ///   raw IP:       [203, 0, 113, 42]
    ///   XOR'd:        [203^0x21, 0^0x12, 113^0xA4, 42^0x42]
    ///                = [0xCA^0x21, 0^0x12, 0x71^0xA4, 0x2A^0x42]
    ///                = [0xEB, 0x12, 0xD5, 0x68]
    fn make_binding_response(transaction_id: &[u8; 12], use_xor: bool) -> Vec<u8> {
        // XOR-MAPPED-ADDRESS attribute value (8 bytes):
        //   reserved=0x00, family=0x01, port_xor'd (ignored here), IP xor'd
        let raw_ip = [203u8, 0, 113, 42];
        let cookie = MAGIC_COOKIE.to_be_bytes();
        let xor_ip = [
            raw_ip[0] ^ cookie[0],
            raw_ip[1] ^ cookie[1],
            raw_ip[2] ^ cookie[2],
            raw_ip[3] ^ cookie[3],
        ];
        // Attribute value: reserved | family | port (2) | ip (4)
        let attr_val = if use_xor {
            vec![
                0x00, 0x01, 0x00, 0x00, xor_ip[0], xor_ip[1], xor_ip[2], xor_ip[3],
            ]
        } else {
            vec![
                0x00, 0x01, 0x00, 0x00, raw_ip[0], raw_ip[1], raw_ip[2], raw_ip[3],
            ]
        };
        let attr_type: u16 = if use_xor {
            ATTR_XOR_MAPPED_ADDRESS
        } else {
            ATTR_MAPPED_ADDRESS
        };
        // TLV header: type (2) | length (2)
        let attr_len = attr_val.len() as u16;
        let mut msg = Vec::with_capacity(20 + 4 + attr_val.len());
        msg.extend_from_slice(&MSG_TYPE_BINDING_RESPONSE.to_be_bytes());
        msg.extend_from_slice(&(4u16 + attr_len).to_be_bytes()); // message length
        msg.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
        msg.extend_from_slice(transaction_id);
        msg.extend_from_slice(&attr_type.to_be_bytes());
        msg.extend_from_slice(&attr_len.to_be_bytes());
        msg.extend_from_slice(&attr_val);
        msg
    }

    #[test]
    fn parse_xor_mapped_address_returns_correct_ip() {
        let tid = [1u8; 12];
        let resp = make_binding_response(&tid, true);
        let ip = parse_stun_response(&resp, &tid).expect("must parse XOR-MAPPED-ADDRESS");
        assert_eq!(ip, "203.0.113.42");
    }

    #[test]
    fn parse_mapped_address_fallback() {
        let tid = [2u8; 12];
        let resp = make_binding_response(&tid, false);
        let ip = parse_stun_response(&resp, &tid).expect("must parse MAPPED-ADDRESS fallback");
        assert_eq!(ip, "203.0.113.42");
    }

    #[test]
    fn parse_rejects_wrong_transaction_id() {
        let tid = [3u8; 12];
        let resp = make_binding_response(&tid, true);
        let wrong_tid = [0u8; 12];
        let result = parse_stun_response(&resp, &wrong_tid);
        assert!(
            result.is_none(),
            "mismatched transaction ID must be rejected"
        );
    }

    #[test]
    fn parse_rejects_too_short_response() {
        let tid = [4u8; 12];
        let result = parse_stun_response(&[0u8; 10], &tid);
        assert!(result.is_none(), "short buffer must return None");
    }

    #[test]
    fn parse_rejects_wrong_message_type() {
        let tid = [5u8; 12];
        let mut resp = make_binding_response(&tid, true);
        resp[0] = 0xFF; // corrupt message type
        let result = parse_stun_response(&resp, &tid);
        assert!(result.is_none(), "wrong message type must return None");
    }

    #[test]
    fn parse_rejects_wrong_magic_cookie() {
        let tid = [6u8; 12];
        let mut resp = make_binding_response(&tid, true);
        resp[4] = 0xFF; // corrupt magic cookie
        let result = parse_stun_response(&resp, &tid);
        assert!(result.is_none(), "wrong magic cookie must return None");
    }
}

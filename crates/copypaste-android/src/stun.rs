//! Best-effort public / WAN IP discovery via STUN for Android.
//!
//! Sends a single RFC 5389 STUN Binding Request to a public STUN server
//! (see `copypaste_core::net::STUN_SERVERS`, tried in order) over UDP, then
//! parses the XOR-MAPPED-ADDRESS attribute from the response to learn the
//! reflexive (NAT-external) address.
//!
//! This is the Android-side counterpart of `copypaste_daemon::public_ip`.
//! The logic is byte-for-byte equivalent: same STUN server list, same
//! 5-second timeout, same parse/XOR logic.  Factored here rather than in
//! the daemon so the Android FFI can call it without depending on
//! `copypaste-daemon`. The server list itself lives in `copypaste-core`,
//! which both crates already depend on (CopyPaste-8ebg.60).
//!
//! ## Opt-out
//! The FFI wrapper (`resolve_stun_public_ip`) is always available, but
//! Kotlin MUST gate it behind the user's `collect_public_ip` setting before
//! calling — exactly as the macOS daemon gates `resolve_public_ip` behind
//! `AppConfig::collect_public_ip`.
//!
//! ## Error handling
//! All errors are logged at `debug` level and return `None` — they MUST NOT
//! propagate to the caller.

use copypaste_core::net::STUN_SERVERS;
use std::net::UdpSocket;
use std::time::Duration;
use tracing::debug;

/// Total wall-clock budget for the STUN exchange.
const STUN_TIMEOUT: Duration = Duration::from_secs(5);

/// STUN message type: Binding Request (RFC 5389 §6).
const MSG_TYPE_BINDING_REQUEST: u16 = 0x0001;
/// STUN message type: Binding Success Response.
const MSG_TYPE_BINDING_RESPONSE: u16 = 0x0101;

/// XOR-MAPPED-ADDRESS attribute type (RFC 5389 §15.2).
const ATTR_XOR_MAPPED_ADDRESS: u16 = 0x0020;
/// MAPPED-ADDRESS attribute type (RFC 3489 §11.2, kept as fallback).
const ATTR_MAPPED_ADDRESS: u16 = 0x0001;

/// Magic cookie mandated by RFC 5389 §6.
const MAGIC_COOKIE: u32 = 0x2112_A442;

/// Perform a STUN Binding Request and return the reflexive IPv4 address, or
/// `None` on any failure (network unreachable, timeout, parse error, …).
///
/// This is a **blocking** function (uses `UdpSocket` with a read timeout).
/// Kotlin MUST call this from a background thread / IO dispatcher and MUST
/// gate the call behind the user's `collect_public_ip` setting.
///
/// Tries each server in [`STUN_SERVERS`] in order, returning the first
/// success (CopyPaste-8ebg.60: fallback list, was a single hardcoded server).
pub fn resolve_public_ip() -> Option<String> {
    STUN_SERVERS
        .iter()
        .find_map(|server| resolve_public_ip_via(server))
}

/// Same as [`resolve_public_ip`] but accepts a custom server address.
/// Extracted so unit tests can drive the logic with a loopback fixture.
pub fn resolve_public_ip_via(server: &str) -> Option<String> {
    let transaction_id = random_transaction_id();
    let mut req = [0u8; 20];
    req[0..2].copy_from_slice(&MSG_TYPE_BINDING_REQUEST.to_be_bytes());
    req[2..4].copy_from_slice(&0u16.to_be_bytes());
    req[4..8].copy_from_slice(&MAGIC_COOKIE.to_be_bytes());
    req[8..20].copy_from_slice(&transaction_id);

    let sock = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(e) => {
            debug!("android/stun: UDP bind failed: {e}");
            return None;
        }
    };
    if let Err(e) = sock.set_read_timeout(Some(STUN_TIMEOUT)) {
        debug!("android/stun: set_read_timeout failed: {e}");
        return None;
    }
    if let Err(e) = sock.connect(server) {
        debug!("android/stun: UDP connect to {server} failed: {e}");
        return None;
    }
    if let Err(e) = sock.send(&req) {
        debug!("android/stun: STUN send failed: {e}");
        return None;
    }

    let mut buf = [0u8; 512];
    let n = match sock.recv(&mut buf) {
        Ok(n) => n,
        Err(e) => {
            debug!("android/stun: STUN recv failed: {e}");
            return None;
        }
    };

    parse_stun_response(&buf[..n], &transaction_id)
}

/// Parse a raw STUN response and extract the reflexive IPv4 address string.
/// Returns `None` for any malformed or unexpected response.
pub(crate) fn parse_stun_response(buf: &[u8], transaction_id: &[u8; 12]) -> Option<String> {
    if buf.len() < 20 {
        debug!(
            "android/stun: STUN response too short ({} bytes)",
            buf.len()
        );
        return None;
    }

    let msg_type = u16::from_be_bytes([buf[0], buf[1]]);
    if msg_type != MSG_TYPE_BINDING_RESPONSE {
        debug!("android/stun: unexpected STUN message type 0x{msg_type:04x}");
        return None;
    }

    let cookie = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
    if cookie != MAGIC_COOKIE {
        debug!("android/stun: magic cookie mismatch (got 0x{cookie:08x})");
        return None;
    }

    if &buf[8..20] != transaction_id.as_ref() {
        debug!("android/stun: transaction ID mismatch");
        return None;
    }

    let attr_len = u16::from_be_bytes([buf[2], buf[3]]) as usize;
    let payload_end = 20usize.saturating_add(attr_len).min(buf.len());
    let attrs = &buf[20..payload_end];

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

        offset += (value_len + 3) & !3;
    }

    fallback
}

fn decode_mapped_address(value: &[u8]) -> Option<String> {
    if value.len() < 8 {
        return None;
    }
    let family = value[1];
    if family != 0x01 {
        return None;
    }
    let ip = std::net::Ipv4Addr::new(value[4], value[5], value[6], value[7]);
    Some(ip.to_string())
}

fn decode_xor_mapped_address(value: &[u8]) -> Option<String> {
    if value.len() < 8 {
        return None;
    }
    let family = value[1];
    if family != 0x01 {
        return None;
    }
    let cookie_bytes = MAGIC_COOKIE.to_be_bytes();
    let b0 = value[4] ^ cookie_bytes[0];
    let b1 = value[5] ^ cookie_bytes[1];
    let b2 = value[6] ^ cookie_bytes[2];
    let b3 = value[7] ^ cookie_bytes[3];
    let ip = std::net::Ipv4Addr::new(b0, b1, b2, b3);
    Some(ip.to_string())
}

fn random_transaction_id() -> [u8; 12] {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    thread_local! {
        static COUNTER: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    }
    let counter = COUNTER.with(|c| {
        let v = c.get().wrapping_add(1);
        c.set(v);
        v
    });
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

    fn make_binding_response(transaction_id: &[u8; 12], use_xor: bool) -> Vec<u8> {
        let raw_ip = [203u8, 0, 113, 42];
        let cookie = MAGIC_COOKIE.to_be_bytes();
        let xor_ip = [
            raw_ip[0] ^ cookie[0],
            raw_ip[1] ^ cookie[1],
            raw_ip[2] ^ cookie[2],
            raw_ip[3] ^ cookie[3],
        ];
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
        let attr_len = attr_val.len() as u16;
        let mut msg = Vec::with_capacity(20 + 4 + attr_val.len());
        msg.extend_from_slice(&MSG_TYPE_BINDING_RESPONSE.to_be_bytes());
        msg.extend_from_slice(&(4u16 + attr_len).to_be_bytes());
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
}

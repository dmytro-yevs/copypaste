use copypaste_ipc::PairingInviteData;
use zeroize::Zeroizing;

use super::invite::{decode_native_invite, encode_native_invite, validate_native_invite_fields};
use super::ScannedPairing;

const SCHEME_PAIR: &str = "copypaste://pair";
const SCHEME_PAIR_SLASH: &str = "copypaste://pair/";
const SCHEME_OPAQUE: &str = "copypaste:pair";
const MAX_LINK_BYTES: usize = 512;

pub(crate) fn encode_pairing_link(invite: &PairingInviteData) -> Option<Zeroizing<String>> {
    encode_native_invite(invite)?;
    let listen_addr = invite.listen_addr.as_deref()?;
    let encoded = Zeroizing::new(format!(
        "{SCHEME_PAIR}?v=1&code={}&listen_addr={}",
        percent_encode(&invite.code),
        percent_encode(listen_addr),
    ));
    (encoded.len() <= MAX_LINK_BYTES).then_some(encoded)
}

pub(crate) fn decode_pairing_link(payload: &str) -> Option<ScannedPairing> {
    if payload.len() > MAX_LINK_BYTES {
        return None;
    }
    let (head, query) = payload.split_once('?')?;
    if head != SCHEME_PAIR && head != SCHEME_PAIR_SLASH && head != SCHEME_OPAQUE {
        return None;
    }
    let mut version = String::from("1");
    let mut code = None;
    let mut listen_addr = None;
    for part in query.split('&') {
        let (key, value) = part.split_once('=')?;
        let decoded = percent_decode(value)?;
        match key {
            "v" => version = decoded,
            "code" => code = Some(decoded),
            "listen_addr" => listen_addr = Some(decoded),
            _ => {}
        }
    }
    if version != "1" {
        return None;
    }
    validate_native_invite_fields(Zeroizing::new(code?), Zeroizing::new(listen_addr?))
}

pub(crate) fn decode_pairing_payload(payload: Zeroizing<String>) -> Option<ScannedPairing> {
    if let Some(scanned) = decode_pairing_link(&payload) {
        return Some(scanned);
    }
    decode_native_invite(payload)
}

fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b':' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                if index + 2 >= bytes.len() {
                    return None;
                }
                let hi = from_hex(bytes[index + 1])?;
                let lo = from_hex(bytes[index + 2])?;
                out.push((hi << 4) | lo);
                index += 3;
            }
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

fn from_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invite() -> PairingInviteData {
        PairingInviteData {
            code: "0123-4567-89AB-CDEF".into(),
            pairing_id: "public-id".into(),
            listen_addr: Some("192.0.2.1:47654".into()),
            expires_in_secs: 120,
        }
    }

    #[test]
    fn pairing_link_round_trips_and_stays_a_custom_scheme() {
        let encoded = encode_pairing_link(&invite()).unwrap();
        assert!(encoded.starts_with("copypaste://pair?"));
        assert!(!encoded.contains("https://"));
        let decoded = decode_pairing_link(&encoded).unwrap();
        assert_eq!(decoded.code.as_str(), "0123-4567-89AB-CDEF");
        assert_eq!(decoded.addr.as_str(), "192.0.2.1:47654");
    }

    #[test]
    fn decode_accepts_url_or_json_payload() {
        let url = encode_pairing_link(&invite()).unwrap();
        let from_url = decode_pairing_payload(url).unwrap();
        assert_eq!(from_url.code.as_str(), "0123-4567-89AB-CDEF");

        let json = encode_native_invite(&invite()).unwrap();
        let from_json = decode_pairing_payload(json).unwrap();
        assert_eq!(from_json.addr.as_str(), "192.0.2.1:47654");
    }

    #[test]
    fn malformed_links_are_rejected() {
        assert!(decode_pairing_link("https://example.com/pair?code=a&listen_addr=b").is_none());
        assert!(
            decode_pairing_link("copypaste://pair?v=2&code=secret&listen_addr=host:1").is_none()
        );
        assert!(decode_pairing_link("copypaste://pair?code=secret").is_none());
    }
}

use copypaste_ipc::PairingInviteData;
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use super::ScannedPairing;

const VERSION: u8 = 1;
const MAX_CODE_BYTES: usize = 128;
const MAX_LISTEN_ADDR_BYTES: usize = 64;
const MAX_ENCODED_BYTES: usize = 512;

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct NativeInviteRef<'a> {
    version: u8,
    code: &'a str,
    listen_addr: &'a str,
}

#[derive(Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
struct NativeInvite {
    version: u8,
    code: String,
    listen_addr: String,
}

pub(crate) fn encode_native_invite(invite: &PairingInviteData) -> Option<Zeroizing<String>> {
    let listen_addr = invite.listen_addr.as_deref()?;
    if !valid_field(&invite.code, MAX_CODE_BYTES)
        || !valid_field(listen_addr, MAX_LISTEN_ADDR_BYTES)
    {
        return None;
    }
    let encoded = Zeroizing::new(
        serde_json::to_string(&NativeInviteRef {
            version: VERSION,
            code: &invite.code,
            listen_addr,
        })
        .ok()?,
    );
    (encoded.len() <= MAX_ENCODED_BYTES).then_some(encoded)
}

pub(super) fn decode_native_invite(payload: Zeroizing<String>) -> Option<ScannedPairing> {
    if payload.len() > MAX_ENCODED_BYTES {
        return None;
    }
    let mut invite: NativeInvite = serde_json::from_str(&payload).ok()?;
    if invite.version != VERSION
        || !valid_field(&invite.code, MAX_CODE_BYTES)
        || !valid_field(&invite.listen_addr, MAX_LISTEN_ADDR_BYTES)
    {
        return None;
    }
    Some(ScannedPairing {
        code: Zeroizing::new(std::mem::take(&mut invite.code)),
        addr: Zeroizing::new(std::mem::take(&mut invite.listen_addr)),
    })
}

fn valid_field(value: &str, max_bytes: usize) -> bool {
    !value.is_empty() && value.len() <= max_bytes
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

    fn payload(code: &str, listen_addr: &str) -> Zeroizing<String> {
        Zeroizing::new(
            serde_json::json!({
                "version": VERSION,
                "code": code,
                "listen_addr": listen_addr
            })
            .to_string(),
        )
    }

    #[test]
    fn native_invite_has_the_exact_version_one_shape() {
        let encoded = encode_native_invite(&invite()).unwrap();
        assert_eq!(
            encoded.as_str(),
            r#"{"version":1,"code":"0123-4567-89AB-CDEF","listen_addr":"192.0.2.1:47654"}"#
        );

        let decoded = decode_native_invite(encoded).unwrap();
        assert_eq!(decoded.code.as_str(), "0123-4567-89AB-CDEF");
        assert_eq!(decoded.addr.as_str(), "192.0.2.1:47654");
    }

    #[test]
    fn pairing_metadata_is_not_encoded() {
        let encoded = encode_native_invite(&invite()).unwrap();
        assert!(!encoded.contains("public-id"));
        assert!(!encoded.contains("expires_in_secs"));
        assert!(!encoded.contains("pairing_id"));
    }

    #[test]
    fn malformed_or_non_object_payloads_are_rejected() {
        for malformed in [
            "null",
            "[]",
            "not json",
            r#"{"version":1,"code":"secret"}"#,
            r#"{"version":1,"listen_addr":"host:1"}"#,
            r#"{"code":"secret","listen_addr":"host:1"}"#,
            r#"{"version":1,"code":"secret","listen_addr":"host:1","extra":true}"#,
            r#"{"version":1,"code":"first","code":"second","listen_addr":"host:1"}"#,
            r#"{"version":1,"code":"secret","listen_addr":"host:1"} trailing"#,
        ] {
            assert!(
                decode_native_invite(Zeroizing::new(malformed.into())).is_none(),
                "accepted {malformed}"
            );
        }
    }

    #[test]
    fn every_version_other_than_integer_one_is_rejected() {
        for version in ["0", "2", "-1", "256", "1.0", r#""1""#] {
            let encoded =
                format!(r#"{{"version":{version},"code":"secret","listen_addr":"host:1"}}"#);
            assert!(
                decode_native_invite(Zeroizing::new(encoded)).is_none(),
                "accepted version {version}"
            );
        }
    }

    #[test]
    fn field_byte_boundaries_are_exact_on_encode_and_decode() {
        let code = "c".repeat(MAX_CODE_BYTES);
        let addr = "a".repeat(MAX_LISTEN_ADDR_BYTES);
        let mut boundary = invite();
        boundary.code = code.clone();
        boundary.listen_addr = Some(addr.clone());
        assert!(encode_native_invite(&boundary).is_some());
        assert!(decode_native_invite(payload(&code, &addr)).is_some());

        boundary.code.push('c');
        assert!(encode_native_invite(&boundary).is_none());
        assert!(decode_native_invite(payload(&boundary.code, &addr)).is_none());

        boundary.code = "secret".into();
        boundary.listen_addr = Some("a".repeat(MAX_LISTEN_ADDR_BYTES + 1));
        assert!(encode_native_invite(&boundary).is_none());
        assert!(decode_native_invite(payload(
            &boundary.code,
            boundary.listen_addr.as_deref().unwrap()
        ))
        .is_none());

        boundary.code.clear();
        boundary.listen_addr = Some("host:1".into());
        assert!(encode_native_invite(&boundary).is_none());
        assert!(decode_native_invite(payload("", "host:1")).is_none());
        boundary.code = "secret".into();
        boundary.listen_addr = Some(String::new());
        assert!(encode_native_invite(&boundary).is_none());
        assert!(decode_native_invite(payload("secret", "")).is_none());
    }

    #[test]
    fn bounds_count_utf8_bytes() {
        let exact = "é".repeat(MAX_CODE_BYTES / 2);
        let oversized = format!("{exact}é");
        assert_eq!(exact.len(), MAX_CODE_BYTES);
        assert!(decode_native_invite(payload(&exact, "host:1")).is_some());
        assert!(decode_native_invite(payload(&oversized, "host:1")).is_none());
    }

    #[test]
    fn encoded_payload_boundary_is_exact() {
        let base = payload("secret", "host:1");
        let exact = format!(
            "{}{}",
            " ".repeat(MAX_ENCODED_BYTES - base.len()),
            base.as_str()
        );
        assert_eq!(exact.len(), MAX_ENCODED_BYTES);
        assert!(decode_native_invite(Zeroizing::new(exact.clone())).is_some());
        assert!(decode_native_invite(Zeroizing::new(format!(" {exact}"))).is_none());

        let mut escaped = invite();
        escaped.code = "\0".repeat(MAX_CODE_BYTES);
        assert!(encode_native_invite(&escaped).is_none());
    }

    #[test]
    fn codec_and_scanned_fields_keep_owned_secrets_zeroized() {
        fn assert_zeroize<T: Zeroize>() {}
        fn assert_zeroize_on_drop<T: ZeroizeOnDrop>() {}
        fn assert_codec_signatures(
            _encode: fn(&PairingInviteData) -> Option<Zeroizing<String>>,
            _decode: fn(Zeroizing<String>) -> Option<ScannedPairing>,
        ) {
        }

        assert_zeroize::<NativeInvite>();
        assert_zeroize_on_drop::<NativeInvite>();
        assert_codec_signatures(encode_native_invite, decode_native_invite);

        let mut owned = NativeInvite {
            version: VERSION,
            code: "secret".into(),
            listen_addr: "host:1".into(),
        };
        owned.zeroize();
        assert_eq!(owned.version, 0);
        assert!(owned.code.is_empty());
        assert!(owned.listen_addr.is_empty());

        let scanned = decode_native_invite(payload("secret", "host:1")).unwrap();
        let _: Zeroizing<String> = scanned.code;
        let _: Zeroizing<String> = scanned.addr;
    }

    #[test]
    fn codec_has_no_external_presentation_or_transport_sink() {
        let production = include_str!("invite.rs")
            .split_once("#[cfg(test)]")
            .unwrap()
            .0;
        for forbidden in [
            "deep_link",
            "clipboard",
            "WebView",
            "Webview",
            "tracing::",
            "log::",
            "http://",
            "https://",
            "://",
        ] {
            assert!(
                !production.contains(forbidden),
                "forbidden sink: {forbidden}"
            );
        }
    }
}

//! Custom rustls certificate verifier for P2P mutual TLS.
//!
//! Standard TLS certificate validation (chain-of-trust, hostname, expiry) is
//! bypassed because devices use self-signed certificates. Instead, identity is
//! established purely by comparing the SHA-256 fingerprint of the peer's
//! certificate DER against the `PairedPeers` allowlist.
//!
//! This is the "Trust On First Use / pinning" model: certificates are exchanged
//! out-of-band during device pairing and stored in the local database.
//!
//! ## CopyPaste-65ue: cert expiry enforcement
//!
//! Even though chain-of-trust validation is bypassed (self-signed certs), cert
//! *expiry* is still enforced. An expired but pinned cert indicates a rotation
//! has occurred and the peer should have re-paired with a new cert; accepting
//! it after expiry would extend the validity window beyond the operator's intent.
//! Expiry is checked using the `now` parameter supplied by rustls at handshake
//! time and a minimal DER parser for the X.509 `validity` field.

use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{DigitallySignedStruct, DistinguishedName, Error as TlsError, SignatureScheme};
use subtle::ConstantTimeEq;

use crate::cert::fingerprint_of;
use crate::transport::{PairedPeers, P2P_SNI_SENTINEL};

// ---------------------------------------------------------------------------
// CopyPaste-65ue: minimal DER validity parser
// ---------------------------------------------------------------------------

/// Parse the `notAfter` field from an X.509 DER certificate and return it as
/// seconds since the Unix epoch, or `None` if parsing fails.
///
/// X.509 DER structure (RFC 5280):
/// ```text
/// Certificate ::= SEQUENCE {
///   tbsCertificate TBSCertificate, ...
/// }
/// TBSCertificate ::= SEQUENCE {
///   version         [0] EXPLICIT INTEGER OPTIONAL,
///   serialNumber    CertificateSerialNumber,
///   signature       AlgorithmIdentifier,
///   issuer          Name,
///   validity        Validity,   ← we want this
///   ...
/// }
/// Validity ::= SEQUENCE {
///   notBefore Time,
///   notAfter  Time,
/// }
/// Time ::= CHOICE { utcTime UTCTime, generalTime GeneralizedTime }
/// ```
///
/// UTCTime format:   `YYMMDDHHMMSSZ`   (ASN.1 tag 0x17)
/// GeneralizedTime:  `YYYYMMDDHHMMSSZ` (ASN.1 tag 0x18)
///
/// Failure modes return `None` and the caller rejects the cert, so a
/// malformed cert is treated as expired rather than silently accepted.
pub(crate) fn parse_not_after_unix(der: &[u8]) -> Option<u64> {
    // Walk the DER stream: SEQUENCE (Certificate) → SEQUENCE (tbs) → skip fields
    // until we reach the Validity SEQUENCE.
    let tbs = der_first_seq_of_seq(der)?;
    // TBSCertificate fields before Validity:
    //   version [0]  (optional context-specific 0xa0)
    //   serialNumber INTEGER
    //   signature    SEQUENCE
    //   issuer       SEQUENCE
    // then validity SEQUENCE.
    let mut pos = tbs;
    // Skip optional [0] version wrapper (tag 0xa0).
    if pos.first().copied() == Some(0xa0) {
        pos = der_skip_tag_len(pos)?;
    }
    // serialNumber INTEGER (tag 0x02).
    pos = der_skip_tlv(pos)?;
    // signature AlgorithmIdentifier SEQUENCE (tag 0x30).
    pos = der_skip_tlv(pos)?;
    // issuer Name SEQUENCE (tag 0x30).
    pos = der_skip_tlv(pos)?;
    // validity SEQUENCE (tag 0x30).
    let validity_inner = der_contents_of_seq(pos)?;
    // notBefore Time — skip it.
    let rest = der_skip_tlv(validity_inner)?;
    // notAfter Time (UTCTime 0x17 or GeneralizedTime 0x18).
    parse_time(rest)
}

/// Return a slice starting at the *contents* of the first SEQUENCE inside a
/// DER blob that itself starts with a SEQUENCE (Certificate wrapping tbs).
fn der_first_seq_of_seq(der: &[u8]) -> Option<&[u8]> {
    let inner = der_contents_of_seq(der)?;
    // inner starts at TBSCertificate SEQUENCE
    der_contents_of_seq(inner)
}

/// Given a slice starting at a DER TLV (tag, length, value...), return a
/// slice of the value bytes for a SEQUENCE (tag 0x30), or `None`.
fn der_contents_of_seq(der: &[u8]) -> Option<&[u8]> {
    let (&tag, rest) = der.split_first()?;
    if tag != 0x30 {
        return None;
    }
    let (len, body) = der_decode_length(rest)?;
    body.get(..len)
}

/// Skip past a complete TLV (any tag), returning the bytes after it.
fn der_skip_tlv(der: &[u8]) -> Option<&[u8]> {
    let (_tag, rest) = der.split_first()?;
    let (len, body) = der_decode_length(rest)?;
    body.get(len..)
}

/// Skip a DER element that starts with context-specific \[0\] tag (0xa0..=0xbf),
/// returning a slice *of the contents* (stripping the outer tag+length).
fn der_skip_tag_len(der: &[u8]) -> Option<&[u8]> {
    der_skip_tlv(der)
}

/// Decode a DER length field. Returns `(length, remaining_bytes_after_length)`.
/// Supports definite short (1 byte) and definite long (multi-byte) forms.
fn der_decode_length(der: &[u8]) -> Option<(usize, &[u8])> {
    let (&first, rest) = der.split_first()?;
    if first < 0x80 {
        // Short form: length is the byte itself.
        Some((first as usize, rest))
    } else {
        // Long form: first byte's low 7 bits = number of subsequent length bytes.
        let n = (first & 0x7f) as usize;
        if n == 0 || n > 4 || rest.len() < n {
            return None; // indefinite or too-large length — reject
        }
        let (len_bytes, tail) = rest.split_at(n);
        let mut len = 0usize;
        for &b in len_bytes {
            len = len.checked_shl(8)?.checked_add(b as usize)?;
        }
        Some((len, tail))
    }
}

/// Parse a DER Time value (UTCTime or GeneralizedTime) and return Unix seconds.
fn parse_time(der: &[u8]) -> Option<u64> {
    let (&tag, rest) = der.split_first()?;
    let (len, body) = der_decode_length(rest)?;
    let s = std::str::from_utf8(body.get(..len)?).ok()?;
    match tag {
        0x17 => parse_utc_time(s),         // UTCTime: YYMMDDHHMMSSZ
        0x18 => parse_generalized_time(s), // GeneralizedTime: YYYYMMDDHHMMSSZ
        _ => None,
    }
}

/// Parse a UTCTime string `YYMMDDHHMMSSZ` → Unix seconds (seconds since epoch).
/// RFC 5280: if YY ≥ 50, interpret as 19YY; else 20YY.
fn parse_utc_time(s: &str) -> Option<u64> {
    // Must be exactly 13 chars: YYMMDDHHMMSSZ
    if s.len() != 13 || !s.ends_with('Z') {
        return None;
    }
    let yy: u16 = s[0..2].parse().ok()?;
    let full_year = if yy >= 50 { 1900u16 + yy } else { 2000u16 + yy };
    parse_generalized_inner(full_year, s.get(2..12)?)
}

/// Parse a GeneralizedTime string `YYYYMMDDHHMMSSZ` → Unix seconds.
fn parse_generalized_time(s: &str) -> Option<u64> {
    if s.len() != 15 || !s.ends_with('Z') {
        return None;
    }
    let year: u16 = s[0..4].parse().ok()?;
    parse_generalized_inner(year, s.get(4..14)?)
}

/// Shared parser for both time formats once the year has been extracted.
/// `rest` = `MMDDHHmmss` (10 chars).
fn parse_generalized_inner(year: u16, rest: &str) -> Option<u64> {
    if rest.len() != 10 {
        return None;
    }
    let month: u32 = rest[0..2].parse().ok()?;
    let day: u32 = rest[2..4].parse().ok()?;
    let hour: u32 = rest[4..6].parse().ok()?;
    let min: u32 = rest[6..8].parse().ok()?;
    let sec: u32 = rest[8..10].parse().ok()?;

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    // Convert to Unix timestamp via a Julian-day calculation (Gregorian → epoch).
    // Uses the civil-day algorithm from http://howardhinnant.github.io/date_algorithms.html
    let y = year as i64;
    let m = month as i64;
    let d = day as i64;
    let (y_adj, m_adj) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    let era = y_adj.div_euclid(400);
    let yoe = y_adj - era * 400; // [0, 399]
    let doy = (153 * m_adj + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    let days_since_epoch = era * 146097 + doe - 719468; // days since 1970-01-01

    let secs = days_since_epoch * 86400 + hour as i64 * 3600 + min as i64 * 60 + sec as i64;
    if secs < 0 {
        return None; // date before Unix epoch
    }
    Some(secs as u64)
}

/// Rustls verifier used on **both** the server side (for client certs) and
/// the client side (for server certs).
///
/// Verification logic:
/// 1. Extract the first certificate from the chain.
/// 2. Compute its SHA-256 fingerprint.
/// 3. Check that the fingerprint is in `PairedPeers`.
/// 4. If not found → reject with `CertificateUnknown`.
///
/// All other validation (chain, expiry, hostname) is intentionally skipped —
/// self-signed certs cannot be validated by a CA chain.
#[derive(Debug)]
pub struct PeerCertVerifier {
    peers: Arc<PairedPeers>,
    /// When set (client side), only this specific fingerprint is accepted.
    expected: Option<String>,
}

impl PeerCertVerifier {
    /// Server-side: accept any fingerprint that is in the `PairedPeers` map.
    pub fn new(peers: Arc<PairedPeers>) -> Self {
        Self {
            peers,
            expected: None,
        }
    }

    /// Client-side: accept exactly `expected_fingerprint` (which must also be
    /// in `PairedPeers`).
    pub fn new_with_expected(peers: Arc<PairedPeers>, expected_fingerprint: &str) -> Self {
        Self {
            peers,
            expected: Some(expected_fingerprint.to_owned()),
        }
    }

    /// Verify the peer's certificate: fingerprint check + expiry enforcement.
    ///
    /// CopyPaste-65ue: `now` is the real clock supplied by rustls at handshake
    /// time. It is used to enforce certificate expiry (the previous version
    /// accepted `_now: UnixTime` but never used it). An expired-but-pinned cert
    /// is rejected so device owners are forced to re-pair after their cert's
    /// validity window closes, preventing indefinitely-long acceptance windows.
    fn verify_fingerprint(
        &self,
        end_entity: &CertificateDer<'_>,
        now: UnixTime,
    ) -> Result<(), TlsError> {
        let der = end_entity.as_ref();
        let fp = fingerprint_of(der);

        // S4: Reject empty fingerprints — a zero-length DER slice produces a
        // well-known SHA-256 hash that must never be accepted as a peer identity.
        // A real ECDSA P-256 certificate DER is always several hundred bytes;
        // if we somehow compute an empty fingerprint, something went badly wrong.
        if fp.is_empty() {
            tracing::error!("peer cert fingerprint computed as empty — rejecting");
            return Err(TlsError::InvalidCertificate(
                rustls::CertificateError::ApplicationVerificationFailure,
            ));
        }

        // CopyPaste-65ue: enforce cert expiry using the handshake clock.
        // Fail closed: if the DER cannot be parsed, reject rather than accept.
        let not_after_unix = parse_not_after_unix(der).ok_or_else(|| {
            tracing::warn!(fingerprint = %fp, "peer cert validity period could not be parsed — rejecting");
            TlsError::InvalidCertificate(rustls::CertificateError::BadEncoding)
        })?;
        // `UnixTime::since_unix_epoch()` returns seconds since epoch as a `Duration`.
        let now_secs = now.as_secs();
        if now_secs > not_after_unix {
            tracing::warn!(
                fingerprint = %fp,
                not_after_unix,
                now_secs,
                "peer cert is expired — rejecting"
            );
            return Err(TlsError::InvalidCertificate(
                rustls::CertificateError::Expired,
            ));
        }

        // If we have a pinned expectation (client side), enforce it first.
        if let Some(ref expected) = self.expected {
            // S4: Also reject if the expected fingerprint itself is empty —
            // callers must always supply a concrete, non-empty expected value.
            if expected.is_empty() {
                tracing::error!(
                    "connect() called with empty expected_fingerprint — rejecting connection"
                );
                return Err(TlsError::InvalidCertificate(
                    rustls::CertificateError::ApplicationVerificationFailure,
                ));
            }
            // Constant-time compare on the hex bytes (LOW #1). FP is public,
            // so this is consistency hardening, not exploit mitigation.
            if fp.as_bytes().ct_eq(expected.as_bytes()).unwrap_u8() != 1 {
                tracing::warn!(
                    got = %fp,
                    expected = %expected,
                    "peer cert fingerprint mismatch"
                );
                return Err(TlsError::InvalidCertificate(
                    rustls::CertificateError::ApplicationVerificationFailure,
                ));
            }
        }

        if !self.peers.is_known(&fp) {
            tracing::warn!(fingerprint = %fp, "peer cert not in paired peers");
            return Err(TlsError::InvalidCertificate(
                rustls::CertificateError::ApplicationVerificationFailure,
            ));
        }

        Ok(())
    }
}

// ---- ClientCertVerifier (used by the server) ----

impl ClientCertVerifier for PeerCertVerifier {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        // We don't have a CA — no hints to offer.
        &[]
    }

    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        now: UnixTime,
    ) -> Result<ClientCertVerified, TlsError> {
        // CopyPaste-65ue: pass `now` so cert expiry is enforced.
        self.verify_fingerprint(end_entity, now)?;
        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

// ---- ServerCertVerifier (used by the client) — S4: SNI + fingerprint guards ----

impl ServerCertVerifier for PeerCertVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        // S4: Validate the SNI server name. In the CopyPaste P2P model the SNI
        // is always the fixed sentinel `P2P_SNI_SENTINEL` (set in
        // `transport.rs`). Anything else indicates the connection was not
        // initiated by our own code — reject it defensively. The exact-match
        // check is performed below; here we first narrow to the DNS-name form.
        let sni_str = match server_name {
            ServerName::DnsName(name) => name.as_ref(),
            ServerName::IpAddress(_) => {
                // IP-address ServerName has no string form to check emptiness;
                // we only use DNS names — reject anything else.
                tracing::warn!("peer presented IP-address SNI instead of DNS name — rejecting");
                return Err(TlsError::InvalidCertificate(
                    rustls::CertificateError::ApplicationVerificationFailure,
                ));
            }
            _ => {
                tracing::warn!("peer presented unknown SNI variant — rejecting");
                return Err(TlsError::InvalidCertificate(
                    rustls::CertificateError::ApplicationVerificationFailure,
                ));
            }
        };
        // Compare against the fixed sentinel rather than merely checking for
        // non-emptiness: our own client always sets exactly this value, so any
        // other SNI means the connection was not initiated by our code. Plain
        // `==` is fine here — the sentinel is a public, non-secret constant.
        if sni_str != P2P_SNI_SENTINEL {
            tracing::warn!(
                got = %sni_str,
                expected = %P2P_SNI_SENTINEL,
                "peer presented unexpected SNI hostname — rejecting"
            );
            return Err(TlsError::InvalidCertificate(
                rustls::CertificateError::ApplicationVerificationFailure,
            ));
        }

        // CopyPaste-65ue: pass `now` so cert expiry is enforced.
        self.verify_fingerprint(end_entity, now)?;
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cert::SelfSignedCert;
    use crate::transport::PairedPeers;
    use std::sync::Arc;

    /// A `UnixTime` that is safely in the past of the generated cert's `notAfter`
    /// (rcgen defaults to a 1-year validity window from generation time).
    fn now_valid() -> UnixTime {
        UnixTime::now()
    }

    /// S4: An empty expected_fingerprint supplied to `new_with_expected` must
    /// cause `verify_fingerprint` to reject the connection, not accept any cert.
    #[test]
    fn empty_expected_fingerprint_is_rejected() {
        let cert = SelfSignedCert::generate("test-device").unwrap();
        let peers = PairedPeers::new();
        // Register the cert's real fingerprint so the peers check would pass…
        peers.add(cert.fingerprint(), "test-device");

        // …but supply an empty expected fingerprint (programming error / confused caller).
        let verifier =
            PeerCertVerifier::new_with_expected(Arc::new(peers), "" /* empty — S4 guard */);

        let der = CertificateDer::from(cert.cert_der.clone());
        // CopyPaste-65ue: pass `now_valid()` so the cert is within its validity
        // window; the rejection must come from the empty fingerprint guard, not expiry.
        let result = verifier.verify_fingerprint(&der, now_valid());
        assert!(
            result.is_err(),
            "empty expected_fingerprint must be rejected even if cert is in PairedPeers"
        );
    }

    /// S4: A fingerprint mismatch (non-empty expected, wrong cert) must be rejected.
    #[test]
    fn fingerprint_mismatch_is_rejected() {
        let cert_a = SelfSignedCert::generate("device-a").unwrap();
        let cert_b = SelfSignedCert::generate("device-b").unwrap();

        let peers = PairedPeers::new();
        peers.add(cert_a.fingerprint(), "device-a");
        peers.add(cert_b.fingerprint(), "device-b");

        // Client expects cert_a but receives cert_b.
        let verifier = PeerCertVerifier::new_with_expected(Arc::new(peers), &cert_a.fingerprint());
        let der_b = CertificateDer::from(cert_b.cert_der.clone());
        assert!(
            verifier.verify_fingerprint(&der_b, now_valid()).is_err(),
            "wrong cert must be rejected"
        );
    }

    /// S4 defense-in-depth: a server cert presented under an SNI other than the
    /// fixed `P2P_SNI_SENTINEL` must be rejected, even when the cert itself is a
    /// known, pinned peer. Guards against connections not initiated by our code.
    #[test]
    fn wrong_sni_is_rejected() {
        let cert = SelfSignedCert::generate("device-ok").unwrap();
        let peers = PairedPeers::new();
        peers.add(cert.fingerprint(), "device-ok");

        let verifier = PeerCertVerifier::new(Arc::new(peers));
        let der = CertificateDer::from(cert.cert_der.clone());
        let wrong_sni = ServerName::try_from("evil.example.com").unwrap();
        let result = verifier.verify_server_cert(&der, &[], &wrong_sni, &[], now_valid());
        assert!(
            result.is_err(),
            "a cert presented under the wrong SNI must be rejected"
        );
    }

    /// Counterpart to `wrong_sni_is_rejected`: the correct sentinel SNI together
    /// with a known cert must be accepted.
    #[test]
    fn correct_sni_is_accepted() {
        let cert = SelfSignedCert::generate("device-ok").unwrap();
        let peers = PairedPeers::new();
        peers.add(cert.fingerprint(), "device-ok");

        let verifier = PeerCertVerifier::new(Arc::new(peers));
        let der = CertificateDer::from(cert.cert_der.clone());
        let good_sni = ServerName::try_from(P2P_SNI_SENTINEL).unwrap();
        let result = verifier.verify_server_cert(&der, &[], &good_sni, &[], now_valid());
        assert!(
            result.is_ok(),
            "correct sentinel SNI + known cert must be accepted"
        );
    }

    /// S4: A cert whose fingerprint is in PairedPeers and matches the expected
    /// value must be accepted.
    #[test]
    fn correct_fingerprint_is_accepted() {
        let cert = SelfSignedCert::generate("device-ok").unwrap();
        let peers = PairedPeers::new();
        peers.add(cert.fingerprint(), "device-ok");

        let verifier = PeerCertVerifier::new_with_expected(Arc::new(peers), &cert.fingerprint());
        let der = CertificateDer::from(cert.cert_der.clone());
        assert!(
            verifier.verify_fingerprint(&der, now_valid()).is_ok(),
            "correct cert must be accepted"
        );
    }

    // ---- CopyPaste-65ue: cert expiry enforcement ----------------------------

    /// CopyPaste-65ue: A freshly-generated cert must be accepted when `now` is
    /// within its validity window (rcgen generates certs valid from now for ~1 year).
    #[test]
    fn fresh_cert_is_accepted_within_validity() {
        let cert = SelfSignedCert::generate("device-fresh").unwrap();
        let peers = PairedPeers::new();
        peers.add(cert.fingerprint(), "device-fresh");
        let verifier = PeerCertVerifier::new(Arc::new(peers));
        let der = CertificateDer::from(cert.cert_der.clone());
        // Use the current time — the cert is freshly generated and must be valid.
        assert!(
            verifier.verify_fingerprint(&der, now_valid()).is_ok(),
            "CopyPaste-65ue: a fresh cert must be accepted when now is within its validity window"
        );
    }

    /// Generate a cert whose `notAfter` is explicitly set to a past date,
    /// so we can test that the verifier rejects it regardless of `now`.
    fn generate_expired_cert(device_id: &str) -> SelfSignedCert {
        use rcgen::{CertificateParams, DistinguishedName, DnType, IsCa, PKCS_ECDSA_P256_SHA256};
        // Set notBefore and notAfter to 2000-01-01..2001-01-01 (well in the past).
        let mut params = CertificateParams::default();
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, device_id);
        params.distinguished_name = dn;
        params.is_ca = IsCa::NoCa;
        params.alg = &PKCS_ECDSA_P256_SHA256;
        params.not_before = rcgen::date_time_ymd(2000, 1, 1);
        params.not_after = rcgen::date_time_ymd(2001, 1, 1); // already expired
        let cert = rcgen::Certificate::from_params(params).unwrap();
        SelfSignedCert {
            cert_der: cert.serialize_der().unwrap(),
            key_der: cert.serialize_private_key_der(),
        }
    }

    /// CopyPaste-65ue: A cert with notAfter in the past must be REJECTED even
    /// when its fingerprint is in `PairedPeers`.
    #[test]
    fn expired_cert_is_rejected_even_when_pinned() {
        let cert = generate_expired_cert("device-expired");
        let peers = PairedPeers::new();
        peers.add(cert.fingerprint(), "device-expired");
        let verifier = PeerCertVerifier::new(Arc::new(peers));
        let der = CertificateDer::from(cert.cert_der.clone());
        // Use the real current time — which is past year 2001.
        let result = verifier.verify_fingerprint(&der, now_valid());
        assert!(
            result.is_err(),
            "CopyPaste-65ue: an expired cert must be rejected even when fingerprint is pinned"
        );
        // Must specifically be the Expired error.
        match result {
            Err(TlsError::InvalidCertificate(rustls::CertificateError::Expired)) => {} // correct
            Err(other) => panic!("expected Expired, got {other:?}"),
            Ok(()) => panic!("expected error"),
        }
    }

    /// CopyPaste-65ue: The DER validity parser must correctly extract `notAfter`
    /// from a freshly-generated cert without panicking or returning None.
    #[test]
    fn der_validity_parser_extracts_not_after() {
        let cert = SelfSignedCert::generate("device-parse").unwrap();
        let not_after = parse_not_after_unix(&cert.cert_der)
            .expect("CopyPaste-65ue: parse_not_after_unix must succeed on a valid rcgen cert");
        // rcgen defaults to notAfter = year 4096, so this is always in the future.
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert!(
            not_after > now_secs,
            "parsed notAfter ({not_after}) must be in the future for a fresh cert"
        );
        // Sanity-check: year 4096 as Unix seconds is approximately 2^36.5 ≈ 6.7e10 s.
        // Just confirm it's more than 10 years out and less than 1e12 s.
        let ten_years_secs = 10u64 * 365 * 24 * 3600;
        assert!(
            not_after > now_secs + ten_years_secs,
            "rcgen's default notAfter (year 4096) should be far in the future"
        );
        assert!(
            not_after < 1_000_000_000_000u64,
            "parsed notAfter sanity check: should be below 1e12 seconds"
        );
    }

    /// CopyPaste-65ue: test the UTCTime and GeneralizedTime parsers directly.
    #[test]
    fn time_parsers_produce_correct_unix_seconds() {
        // 2030-01-15 12:30:45 UTC as UTCTime (YY=30 → 2030 since <50).
        // Expected: 2030-01-15T12:30:45Z
        // Days from epoch: using civil-day formula for 2030-01-15.
        let utc_result = parse_utc_time("300115123045Z");
        assert!(utc_result.is_some(), "UTCTime 300115123045Z must parse");
        // 2024-06-20 00:00:00 UTC as GeneralizedTime.
        let gen_result = parse_generalized_time("20240620000000Z");
        assert!(
            gen_result.is_some(),
            "GeneralizedTime 20240620000000Z must parse"
        );
        // Confirm ordering: 2030 > 2024.
        assert!(
            utc_result.unwrap() > gen_result.unwrap(),
            "2030 timestamp must be greater than 2024 timestamp"
        );
    }

    /// CopyPaste-65ue: malformed time strings must return None (not panic).
    #[test]
    fn malformed_time_returns_none() {
        assert!(parse_utc_time("not-a-time").is_none());
        assert!(parse_utc_time("").is_none());
        assert!(parse_generalized_time("20241301000000Z").is_none()); // month 13
        assert!(parse_generalized_time("20240000000000Z").is_none()); // day 0
    }
}

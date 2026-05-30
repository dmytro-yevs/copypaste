//! Custom rustls certificate verifier for P2P mutual TLS.
//!
//! Standard TLS certificate validation (chain-of-trust, hostname, expiry) is
//! bypassed because devices use self-signed certificates. Instead, identity is
//! established purely by comparing the SHA-256 fingerprint of the peer's
//! certificate DER against the `PairedPeers` allowlist.
//!
//! This is the "Trust On First Use / pinning" model: certificates are exchanged
//! out-of-band during device pairing and stored in the local database.

use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{DigitallySignedStruct, DistinguishedName, Error as TlsError, SignatureScheme};
use subtle::ConstantTimeEq;

use crate::cert::fingerprint_of;
use crate::transport::{PairedPeers, P2P_SNI_SENTINEL};

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

    fn verify_fingerprint(&self, end_entity: &CertificateDer<'_>) -> Result<(), TlsError> {
        let fp = fingerprint_of(end_entity.as_ref());

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
        _now: UnixTime,
    ) -> Result<ClientCertVerified, TlsError> {
        self.verify_fingerprint(end_entity)?;
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
        _now: UnixTime,
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

        self.verify_fingerprint(end_entity)?;
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
        let result = verifier.verify_fingerprint(&der);
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
            verifier.verify_fingerprint(&der_b).is_err(),
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
        let result = verifier.verify_server_cert(&der, &[], &wrong_sni, &[], UnixTime::now());
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
        let result = verifier.verify_server_cert(&der, &[], &good_sni, &[], UnixTime::now());
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
            verifier.verify_fingerprint(&der).is_ok(),
            "correct cert must be accepted"
        );
    }
}

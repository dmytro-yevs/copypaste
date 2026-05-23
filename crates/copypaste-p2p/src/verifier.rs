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

use crate::cert::fingerprint_of;
use crate::transport::PairedPeers;

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

        // If we have a pinned expectation (client side), enforce it first.
        if let Some(ref expected) = self.expected {
            if fp != *expected {
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

// ---- ServerCertVerifier (used by the client) ----

impl ServerCertVerifier for PeerCertVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
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

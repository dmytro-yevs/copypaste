//! Certificate generation and fingerprint computation for P2P mutual TLS.
//!
//! Each device has a self-signed X.509 certificate (ECDSA P-256). The
//! certificate fingerprint is SHA-256(DER bytes) encoded as lowercase hex —
//! this is the device identity exchanged out-of-band during pairing.

use rcgen::{Certificate, CertificateParams, DistinguishedName, DnType, IsCa, PKCS_ECDSA_P256_SHA256};
use sha2::Digest;
use thiserror::Error;

/// Errors produced by certificate operations.
#[derive(Debug, Error)]
pub enum CertError {
    #[error("rcgen error: {0}")]
    Generate(#[from] rcgen::Error),
}

/// A self-signed certificate together with its private key, ready for use in
/// TLS handshakes.
pub struct SelfSignedCert {
    /// DER-encoded certificate bytes (the "public" half, sent to peers).
    pub cert_der: Vec<u8>,
    /// DER-encoded private key bytes (kept secret, never sent).
    pub key_der: Vec<u8>,
}

impl SelfSignedCert {
    /// Generate a fresh self-signed ECDSA P-256 certificate.
    ///
    /// The CN is set to `device_id` so certificates are nominally tied to a
    /// device, but the real identity check uses the fingerprint — not the CN.
    pub fn generate(device_id: &str) -> Result<Self, CertError> {
        let mut params = CertificateParams::default();
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, device_id);
        params.distinguished_name = dn;
        // This is a leaf certificate only — it signs itself, not others.
        params.is_ca = IsCa::NoCa;
        params.alg = &PKCS_ECDSA_P256_SHA256;

        let cert = Certificate::from_params(params)?;
        let cert_der = cert.serialize_der()?;
        let key_der = cert.serialize_private_key_der();

        Ok(Self { cert_der, key_der })
    }

    /// Compute the fingerprint of this certificate.
    ///
    /// Fingerprint = lowercase hex(SHA-256(cert_der)).
    pub fn fingerprint(&self) -> String {
        fingerprint_of(&self.cert_der)
    }
}

/// Compute the fingerprint of a raw DER certificate byte slice.
///
/// Fingerprint = lowercase hex(SHA-256(cert_der)).
pub fn fingerprint_of(cert_der: &[u8]) -> String {
    let hash = sha2::Sha256::digest(cert_der);
    hex::encode(hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_cert_and_key() {
        let cert = SelfSignedCert::generate("device-abc").unwrap();
        assert!(!cert.cert_der.is_empty(), "cert DER must not be empty");
        assert!(!cert.key_der.is_empty(), "key DER must not be empty");
    }

    #[test]
    fn fingerprint_is_64_hex_chars() {
        let cert = SelfSignedCert::generate("device-abc").unwrap();
        let fp = cert.fingerprint();
        assert_eq!(fp.len(), 64, "SHA-256 hex fingerprint must be 64 chars");
        assert!(fp.chars().all(|c| c.is_ascii_hexdigit()), "fingerprint must be hex");
    }

    #[test]
    fn fingerprint_is_deterministic_for_same_der() {
        let cert = SelfSignedCert::generate("device-abc").unwrap();
        assert_eq!(cert.fingerprint(), fingerprint_of(&cert.cert_der));
    }

    #[test]
    fn different_certs_have_different_fingerprints() {
        let a = SelfSignedCert::generate("device-a").unwrap();
        let b = SelfSignedCert::generate("device-b").unwrap();
        assert_ne!(a.fingerprint(), b.fingerprint());
    }

    #[test]
    fn fingerprint_of_empty_bytes_is_sha256_of_empty() {
        // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let fp = fingerprint_of(&[]);
        assert_eq!(fp, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    }
}

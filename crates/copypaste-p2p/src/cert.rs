//! Certificate generation and fingerprint computation for P2P mutual TLS.
//!
//! Each device has a self-signed X.509 certificate (ECDSA P-256). The
//! certificate fingerprint is SHA-256(DER bytes) encoded as lowercase hex —
//! this is the device identity exchanged out-of-band during pairing.

use std::io::Write as _;
use std::path::Path;

use base64::Engine as _;
use rcgen::{
    Certificate, CertificateParams, DistinguishedName, DnType, IsCa, PKCS_ECDSA_P256_SHA256,
};
use sha2::Digest;
use thiserror::Error;

/// On-disk envelope for a persisted self-signed identity.
///
/// Both DER blobs are base64 (standard alphabet) so the file is plain JSON.
/// The `fingerprint` field is advisory — it lets `load_or_create` detect a
/// corrupted/tampered file by recomputing SHA-256 over the stored cert and
/// comparing — but the authoritative identity is always the cert DER itself.
#[derive(serde::Serialize, serde::Deserialize)]
struct StoredIdentity {
    cert_der_b64: String,
    key_der_b64: String,
    /// lowercase-hex SHA-256(cert_der) at write time.
    fingerprint: String,
}

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
    ///
    /// # S10 — cert rotation race (resolved)
    ///
    /// When a certificate is rotated (re-generated via this function) while an
    /// in-flight TLS handshake is using the previous certificate, the handshake
    /// could previously fail transiently because the new fingerprint was not yet
    /// in the peer's `PairedPeers` table. This is now handled by atomic cert
    /// rotation with grace-period dual-fingerprint acceptance:
    /// [`PairedPeers::rotate_peer`](crate::transport::PairedPeers::rotate_peer)
    /// installs the new fingerprint as active while keeping the previous one
    /// valid for [`CERT_ROTATION_GRACE`](crate::transport::CERT_ROTATION_GRACE),
    /// so handshakes (and `transport::connect_with_retry` attempts) that still
    /// present the old certificate continue to verify during the window.
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

    /// Load a persisted identity from `path`, or generate + persist a fresh one
    /// if the file does not exist.
    ///
    /// # Why this matters
    ///
    /// The P2P identity *is* the device's mTLS fingerprint that peers pin at
    /// pairing time. If a fresh cert were generated on every daemon launch (as
    /// [`generate`](Self::generate) does), every restart would change the
    /// fingerprint and silently break all existing pairings — P2P sync would
    /// never survive a restart. Persisting the cert keeps the fingerprint
    /// **stable** across restarts.
    ///
    /// # Behaviour
    ///
    /// * If `path` exists, the stored cert/key DER are read back and the
    ///   in-memory fingerprint is re-verified against the stored one. A
    ///   mismatch (corruption / tamper) is a hard error rather than a silent
    ///   regeneration, because silently regenerating would invalidate pairings.
    /// * Otherwise [`generate`](Self::generate) produces a new identity which is
    ///   then written atomically with mode `0600` (temp file in the same
    ///   directory + `rename`), mirroring `peers.rs::save_peers`.
    ///
    /// Returns an [`io::Error`](std::io::Error) so callers in the daemon can
    /// treat persistence failures uniformly with other filesystem startup
    /// errors.
    pub fn load_or_create(path: &Path, device_id: &str) -> std::io::Result<Self> {
        match std::fs::read(path) {
            Ok(bytes) => {
                let stored: StoredIdentity = serde_json::from_slice(&bytes).map_err(|e| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("corrupt p2p identity file {}: {e}", path.display()),
                    )
                })?;
                let b64 = base64::engine::general_purpose::STANDARD;
                let cert_der = b64.decode(stored.cert_der_b64.as_bytes()).map_err(|e| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("p2p identity cert_der base64 decode failed: {e}"),
                    )
                })?;
                let key_der = b64.decode(stored.key_der_b64.as_bytes()).map_err(|e| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("p2p identity key_der base64 decode failed: {e}"),
                    )
                })?;
                let cert = Self { cert_der, key_der };
                let actual = cert.fingerprint();
                if actual != stored.fingerprint {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!(
                            "p2p identity fingerprint mismatch in {} (stored {}, computed {}) — \
                             refusing to load a corrupt identity",
                            path.display(),
                            stored.fingerprint,
                            actual
                        ),
                    ));
                }
                Ok(cert)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let cert = Self::generate(device_id).map_err(|e| {
                    std::io::Error::other(format!("p2p identity generation failed: {e}"))
                })?;
                cert.persist(path)?;
                Ok(cert)
            }
            Err(e) => Err(e),
        }
    }

    /// Atomically write this identity to `path` with mode `0600`.
    ///
    /// Mirrors the discipline in `copypaste-daemon::peers::save_peers`: write
    /// to a uniquely-named temp file in the **same** directory (so `rename` is
    /// atomic on the same filesystem), create it `0600` from the first byte so
    /// the private key is never momentarily group/other-readable, then rename
    /// over the destination.
    fn persist(&self, path: &Path) -> std::io::Result<()> {
        let parent = path.parent().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("p2p identity path has no parent: {}", path.display()),
            )
        })?;
        std::fs::create_dir_all(parent)?;

        let b64 = base64::engine::general_purpose::STANDARD;
        let stored = StoredIdentity {
            cert_der_b64: b64.encode(&self.cert_der),
            key_der_b64: b64.encode(&self.key_der),
            fingerprint: self.fingerprint(),
        };
        let json = serde_json::to_string_pretty(&stored)
            .map_err(|e| std::io::Error::other(format!("p2p identity serialize failed: {e}")))?;

        let tmp = parent.join(format!(
            ".p2p_identity.json.tmp.{}.{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));

        let write_result = (|| -> std::io::Result<()> {
            let mut opts = std::fs::OpenOptions::new();
            opts.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                opts.mode(0o600);
            }
            let mut f = opts.open(&tmp)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
            }
            f.write_all(json.as_bytes())?;
            f.flush()?;
            f.sync_all()?;
            Ok(())
        })();

        if let Err(e) = write_result {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
        if let Err(e) = std::fs::rename(&tmp, path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
        Ok(())
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
        assert!(
            fp.chars().all(|c| c.is_ascii_hexdigit()),
            "fingerprint must be hex"
        );
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
    fn load_or_create_returns_stable_fingerprint_across_calls() {
        let dir = std::env::temp_dir().join(format!(
            "copypaste-p2p-cert-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("p2p_identity.json");

        // First call: file does not exist → generate + persist.
        assert!(!path.exists());
        let first = SelfSignedCert::load_or_create(&path, "device-xyz").unwrap();
        assert!(path.exists(), "identity file must be created on first call");
        let fp1 = first.fingerprint();

        // Second call against the SAME path must reload the identical identity,
        // NOT regenerate — this is the whole point (stable across restarts).
        let second = SelfSignedCert::load_or_create(&path, "device-xyz").unwrap();
        let fp2 = second.fingerprint();

        assert_eq!(fp1, fp2, "fingerprint must be stable across reloads");
        assert_eq!(
            first.cert_der, second.cert_der,
            "reloaded cert DER must be byte-identical"
        );
        assert_eq!(
            first.key_der, second.key_der,
            "reloaded key DER must be byte-identical"
        );

        // The persisted file must be 0600 on unix (private key protection).
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "identity file must be 0600");
        }

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_or_create_rejects_corrupt_fingerprint() {
        let dir = std::env::temp_dir().join(format!(
            "copypaste-p2p-cert-corrupt-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("p2p_identity.json");

        // Write a well-formed envelope whose fingerprint does NOT match the cert.
        let cert = SelfSignedCert::generate("device-corrupt").unwrap();
        let b64 = base64::engine::general_purpose::STANDARD;
        let bad = format!(
            r#"{{"cert_der_b64":"{}","key_der_b64":"{}","fingerprint":"{}"}}"#,
            b64.encode(&cert.cert_der),
            b64.encode(&cert.key_der),
            "0".repeat(64),
        );
        std::fs::write(&path, bad).unwrap();

        // Avoid `expect_err` (would require `SelfSignedCert: Debug`, but it
        // holds the private key and must not be Debug-printable).
        match SelfSignedCert::load_or_create(&path, "device-corrupt") {
            Ok(_) => panic!("must reject a fingerprint mismatch"),
            Err(err) => assert_eq!(err.kind(), std::io::ErrorKind::InvalidData),
        }

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn fingerprint_of_empty_bytes_is_sha256_of_empty() {
        // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let fp = fingerprint_of(&[]);
        assert_eq!(
            fp,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}

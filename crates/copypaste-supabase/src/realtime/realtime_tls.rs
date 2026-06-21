//! TLS cert-pinning layer for the Supabase Realtime WebSocket client.
//!
//! Handles:
//! - SPKI SHA-256 pin set ([`SpkiPins`])
//! - [`PinningVerifier`]: rustls `ServerCertVerifier` that runs WebPKI chain
//!   validation first, then enforces SPKI pinning
//! - [`DerReader`] + [`extract_spki_der`]: minimal ASN.1 DER navigator for
//!   extracting the SubjectPublicKeyInfo field from an X.509 certificate
//! - [`build_rustls_connector`]: assembles a `tokio_tungstenite::Connector`
//!   with the custom verifier

use sha2::Digest as _;
use tokio_tungstenite::Connector;

use rustls::RootCertStore;

// ── SPKI cert pinning (CopyPaste-qkao) ───────────────────────────────────────

/// A set of SHA-256 SPKI (Subject Public Key Info) pin hashes for
/// certificate pinning of the Supabase Realtime WSS endpoint.
///
/// # How to obtain a pin
/// ```sh
/// openssl s_client -connect <project>.supabase.co:443 </dev/null \
///     | openssl x509 -noout -pubkey \
///     | openssl pkey -pubin -outform DER \
///     | openssl dgst -sha256 -binary \
///     | base64
/// ```
///
/// Store the hex form (not base64) as a 32-byte array in `RealtimeConfig.spki_pins`.
///
/// # Empty set (default)
/// When `spki_pins` is empty no additional SPKI check is performed —
/// standard WebPKI chain validation still applies. This keeps the default
/// behavior compatible with deployments that do not (yet) pin certificates.
/// Set at least one pin in production to enable actual pinning.
#[derive(Debug, Clone, Default)]
pub struct SpkiPins {
    /// SHA-256 hashes of the DER-encoded SubjectPublicKeyInfo of acceptable
    /// end-entity certificates. Each entry is 32 bytes (256 bits).
    pub pins: Vec<[u8; 32]>,
}

impl SpkiPins {
    /// Return `true` when the set is empty (pinning not configured).
    pub fn is_empty(&self) -> bool {
        self.pins.is_empty()
    }

    /// Return `true` if `spki_der` hashes (SHA-256) to one of the stored pins.
    pub fn matches(&self, spki_der: &[u8]) -> bool {
        let hash: [u8; 32] = sha2::Sha256::digest(spki_der).into();
        self.pins.iter().any(|p| p == &hash)
    }
}

/// A rustls `ServerCertVerifier` that delegates chain / name validation to
/// `WebPkiServerVerifier` and additionally enforces SPKI pinning.
///
/// If `pins` is empty the SPKI check is skipped (standard PKI only).
/// If `pins` is non-empty and the end-entity certificate's SPKI hash does
/// not match any pin, the connection is refused with
/// `CertificateError::ApplicationVerificationFailure`.
#[derive(Debug)]
struct PinningVerifier {
    inner: std::sync::Arc<rustls::client::WebPkiServerVerifier>,
    pins: SpkiPins,
}

impl rustls::client::danger::ServerCertVerifier for PinningVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &rustls::pki_types::CertificateDer<'_>,
        intermediates: &[rustls::pki_types::CertificateDer<'_>],
        server_name: &rustls::pki_types::ServerName<'_>,
        ocsp_response: &[u8],
        now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        // Run the standard WebPKI chain + name check first. If this fails,
        // reject immediately (don't bother with pin check).
        let result = self.inner.verify_server_cert(
            end_entity,
            intermediates,
            server_name,
            ocsp_response,
            now,
        )?;

        // Additional SPKI pin check (only when pins are configured).
        if !self.pins.is_empty() {
            // Parse the end-entity cert to extract the raw SubjectPublicKeyInfo DER.
            // `rcgen` is not available here; we use the low-level ring/webpki path.
            // The DER cert is already validated by the inner verifier, so we only
            // need to locate the SPKI field. Use a minimal manual parse: X.509
            // TBSCertificate.subjectPublicKeyInfo is a named field we can reach via
            // rustls-webpki's `EndEntityCert` if exposed, or via `x509-parser`.
            // Since x509-parser is not in our dep tree, we extract SPKI using the
            // `rustls::pki_types` + a small DER walk.
            //
            // For robustness we hash the ENTIRE end-entity cert DER when we cannot
            // extract the SPKI cleanly; a production deployment should use a proper
            // DER ASN.1 parser. The pin generation command above produces the SPKI
            // hash, so callers must pin the SPKI hash (not the full cert hash).
            // We implement a minimal ASN.1 SEQUENCE navigator to reach the SPKI.
            match extract_spki_der(end_entity) {
                Some(spki) => {
                    if !self.pins.matches(&spki) {
                        tracing::error!(
                            "TLS cert pinning failed: SPKI hash does not match any known pin"
                        );
                        return Err(rustls::Error::InvalidCertificate(
                            rustls::CertificateError::ApplicationVerificationFailure,
                        ));
                    }
                    tracing::debug!("TLS cert pinning: SPKI pin matched");
                }
                None => {
                    // Could not extract SPKI (malformed cert — unlikely after WebPKI
                    // validation, but safe to reject).
                    tracing::error!("TLS cert pinning: could not extract SPKI from cert DER");
                    return Err(rustls::Error::InvalidCertificate(
                        rustls::CertificateError::BadEncoding,
                    ));
                }
            }
        }

        Ok(result)
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.inner.supported_verify_schemes()
    }
}

/// Minimal ASN.1 DER reader: skip past the outermost SEQUENCE tag/length and
/// skip TBSCertificate fields until we reach subjectPublicKeyInfo, then return
/// its raw DER bytes (tag + length + value).
///
/// X.509v3 TBSCertificate structure (RFC 5280 §4.1):
/// ```text
/// TBSCertificate ::= SEQUENCE {
///   version         [0] EXPLICIT INTEGER OPTIONAL,
///   serialNumber        INTEGER,
///   signature           AlgorithmIdentifier,
///   issuer              Name,
///   validity            Validity,
///   subject             Name,
///   subjectPublicKeyInfo SubjectPublicKeyInfo,  -- we want this
///   ...
/// }
/// Certificate ::= SEQUENCE {
///   tbsCertificate      TBSCertificate,          -- outer SEQUENCE
///   ...
/// }
/// ```
///
/// Returns `None` if the DER is too short or structurally invalid.
///
/// This is a best-effort extractor sufficient for SPKI pinning; it does not
/// attempt full validation (the inner WebPKI verifier already did that).
pub(crate) fn extract_spki_der(cert_der: &[u8]) -> Option<Vec<u8>> {
    // The outer structure is:
    //   Certificate ::= SEQUENCE {
    //     tbsCertificate  TBSCertificate,   ← first element, a SEQUENCE
    //     ...
    //   }
    //
    // Step 1: peel the outer Certificate SEQUENCE to get its contents.
    let mut outer = DerReader::new(cert_der);
    let cert_contents = outer.read_sequence()?;

    // Step 2: the first element of cert_contents is the TBSCertificate SEQUENCE.
    // Read it to get ITS contents (the individual TBS fields).
    let mut cert_level = DerReader::new(cert_contents);
    let tbs_contents = cert_level.read_sequence()?;

    // Step 3: navigate the TBSCertificate fields to reach subjectPublicKeyInfo.
    let mut tbs = DerReader::new(tbs_contents);
    // Skip optional [0] EXPLICIT version
    if tbs.peek_tag() == Some(0xa0) {
        tbs.skip_element()?;
    }
    // serialNumber INTEGER
    tbs.skip_element()?;
    // signature AlgorithmIdentifier SEQUENCE
    tbs.skip_element()?;
    // issuer Name (SEQUENCE)
    tbs.skip_element()?;
    // validity Validity (SEQUENCE)
    tbs.skip_element()?;
    // subject Name (SEQUENCE)
    tbs.skip_element()?;
    // subjectPublicKeyInfo — return its full TLV (tag + length + value)
    tbs.read_raw_element()
}

/// Minimal DER/BER reader for the SPKI extractor above.
struct DerReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> DerReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn remaining(&self) -> &'a [u8] {
        &self.data[self.pos..]
    }

    fn peek_tag(&self) -> Option<u8> {
        self.remaining().first().copied()
    }

    /// Read and decode a DER length (short or long form). Returns the length
    /// value and advances the reader past the length octets.
    fn read_length(&mut self) -> Option<usize> {
        let first = *self.remaining().first()?;
        self.pos += 1;
        if first & 0x80 == 0 {
            Some(first as usize)
        } else {
            let n_bytes = (first & 0x7f) as usize;
            if n_bytes == 0 || n_bytes > 4 || self.remaining().len() < n_bytes {
                return None;
            }
            let mut len: usize = 0;
            for &b in &self.remaining()[..n_bytes] {
                len = len.checked_shl(8)?.checked_add(b as usize)?;
            }
            self.pos += n_bytes;
            Some(len)
        }
    }

    /// Read a SEQUENCE tag (0x30) and return the contents slice.
    fn read_sequence(&mut self) -> Option<&'a [u8]> {
        let tag = *self.remaining().first()?;
        if tag != 0x30 {
            return None;
        }
        self.pos += 1;
        let len = self.read_length()?;
        if self.remaining().len() < len {
            return None;
        }
        let contents = &self.remaining()[..len];
        self.pos += len;
        Some(contents)
    }

    /// Skip one complete TLV element (any tag, short or long form length).
    fn skip_element(&mut self) -> Option<()> {
        if self.remaining().is_empty() {
            return None;
        }
        self.pos += 1; // tag
        let len = self.read_length()?;
        if self.remaining().len() < len {
            return None;
        }
        self.pos += len;
        Some(())
    }

    /// Return the complete TLV (tag + encoded length + value) of the next
    /// element as a `Vec<u8>` without consuming it into an inner reader.
    fn read_raw_element(&mut self) -> Option<Vec<u8>> {
        let start = self.pos;
        // Peek tag (don't advance yet)
        if self.remaining().is_empty() {
            return None;
        }
        self.pos += 1; // tag
        let len = self.read_length()?;
        if self.remaining().len() < len {
            return None;
        }
        self.pos += len;
        Some(self.data[start..self.pos].to_vec())
    }
}

/// Build a WebPKI-backed rustls `ClientConfig` with SPKI pinning.
///
/// For loopback URLs (local dev) pinning is skipped even if pins are
/// configured in `RealtimeConfig`. This avoids breaking local test setups
/// that use self-signed certs.
///
/// Returns `None` if `ws_url` points to a loopback host AND no TLS is
/// needed (plain `ws://`), allowing the caller to fall back to
/// `connect_async` (no custom connector).
pub(crate) fn build_rustls_connector(ws_url: &str, pins: &SpkiPins) -> Option<Connector> {
    // Do not apply TLS for plain ws:// (loopback dev scenario).
    if ws_url.starts_with("ws://") {
        return None;
    }

    // Build standard WebPKI root cert store (same roots as the default
    // tokio-tungstenite connector uses when `rustls-tls-webpki-roots` is
    // enabled).
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let roots = std::sync::Arc::new(roots);

    // Build the inner standard verifier.
    let inner = rustls::client::WebPkiServerVerifier::builder(roots)
        .build()
        // Safe: empty CRL list, no revocation errors possible.
        .expect("WebPkiServerVerifier::build must not fail with default params");

    let verifier = PinningVerifier {
        inner,
        pins: pins.clone(),
    };

    let config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(std::sync::Arc::new(verifier))
        .with_no_client_auth();

    Some(Connector::Rustls(std::sync::Arc::new(config)))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── SPKI pins (CopyPaste-qkao) ────────────────────────────────────────────

    #[test]
    fn spki_pins_empty_matches_nothing() {
        let pins = SpkiPins::default();
        assert!(pins.is_empty());
        // Even an empty byte slice should not match an empty pin set.
        assert!(!pins.matches(b"anything"));
    }

    #[test]
    fn spki_pins_matches_correct_hash() {
        use sha2::Digest;
        let spki_bytes = b"fake-spki-der-content";
        let hash: [u8; 32] = sha2::Sha256::digest(spki_bytes).into();
        let pins = SpkiPins { pins: vec![hash] };

        assert!(!pins.is_empty());
        assert!(
            pins.matches(spki_bytes),
            "known SPKI must match its own SHA-256 pin"
        );
        assert!(
            !pins.matches(b"wrong-content"),
            "wrong content must not match"
        );
    }

    // ── extract_spki_der ──────────────────────────────────────────────────────

    /// Minimal DER structure: Certificate SEQUENCE → TBSCertificate SEQUENCE
    /// with enough filler fields so the SPKI field is at the right offset.
    ///
    /// We construct a synthetic (hand-crafted) DER to verify the extractor
    /// without depending on a real X.509 cert. Fields before SPKI in a
    /// TBSCertificate (RFC 5280):
    ///   [0] version (optional) | serialNumber | signature | issuer | validity | subject | SPKI
    ///
    /// We encode each as a minimal SEQUENCE or INTEGER so the extractor can
    /// skip them correctly.
    #[test]
    fn extract_spki_der_returns_correct_field() {
        // Each filler field as a minimal SEQUENCE: tag=0x30 len=0x00 (empty).
        let empty_seq = &[0x30u8, 0x00u8];
        // SPKI field: tag=0x30 len=0x04 content=[1,2,3,4].
        let spki_content = &[1u8, 2, 3, 4];
        let spki_tlv: Vec<u8> = {
            let mut v = vec![0x30u8, spki_content.len() as u8];
            v.extend_from_slice(spki_content);
            v
        };

        // Build TBSCertificate body (no version field for simplicity):
        // serialNumber INTEGER, signature SEQUENCE, issuer SEQUENCE,
        // validity SEQUENCE, subject SEQUENCE, SPKI SEQUENCE.
        // We use 0x02 0x01 0x01 (INTEGER value 1) for serialNumber.
        let serial: Vec<u8> = vec![0x02, 0x01, 0x01];
        let mut tbs_body: Vec<u8> = Vec::new();
        tbs_body.extend_from_slice(&serial);
        tbs_body.extend_from_slice(empty_seq); // signature
        tbs_body.extend_from_slice(empty_seq); // issuer
        tbs_body.extend_from_slice(empty_seq); // validity
        tbs_body.extend_from_slice(empty_seq); // subject
        tbs_body.extend_from_slice(&spki_tlv); // SPKI

        // Wrap TBSCertificate in a SEQUENCE.
        let mut tbs_seq: Vec<u8> = vec![0x30, tbs_body.len() as u8];
        tbs_seq.extend_from_slice(&tbs_body);

        // Outer Certificate SEQUENCE: just the TBS for this test.
        let mut cert_der: Vec<u8> = vec![0x30, tbs_seq.len() as u8];
        cert_der.extend_from_slice(&tbs_seq);

        let extracted = extract_spki_der(&cert_der);
        assert!(extracted.is_some(), "SPKI extraction must succeed");
        assert_eq!(
            extracted.unwrap(),
            spki_tlv,
            "extracted SPKI TLV must equal the expected bytes"
        );
    }
}

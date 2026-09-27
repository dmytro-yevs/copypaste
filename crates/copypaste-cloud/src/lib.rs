//! Cloud sync over Supabase.
//!
//! # The server never sees plaintext
//!
//! Rows are sealed on the client under a key derived from the user's passphrase
//! with Argon2id, and the passphrase never leaves the device. Supabase holds
//! ciphertext and metadata only.
//!
//! RLS is the second layer, not the first: a misconfigured policy exposes rows
//! that are still unreadable.
//!
//! # The metadata is signed, because encryption cannot order anything
//!
//! The fields the merge orders on travel in the clear — the backend pages on
//! them. Whoever can write to the table could therefore decide which version of
//! an item every device keeps, without ever decrypting anything. Every row
//! carries an HMAC over those fields and the ciphertext, under a key derived
//! from the same passphrase, and a row that does not verify is refused before it
//! reaches the merge. See [`crypto::sign`].

#![forbid(unsafe_code)]

pub mod auth;
pub mod credentials;
pub mod crypto;
pub mod realtime;
pub mod rest;
pub mod sync;

pub use auth::{AuthError, Session, SupabaseAuth};
pub use crypto::{decrypt_row, derive_sync_key, encrypt_row, CloudCrypto, SyncKey};
pub use realtime::{RealtimeError, RealtimeEvent, RealtimeSubscription};
pub use rest::{CloudItem, RestError, SupabaseRest};
pub use sync::{CloudSync, SyncError, SyncStats};

#[cfg(any(test, feature = "test-endpoints"))]
use url::Host;
use url::Url;

/// Where the deployment lives and which anon key to present.
#[derive(Clone)]
pub struct CloudConfig {
    url: Url,
    /// The publishable anon key. Not a secret in the usual sense — RLS is what
    /// restricts access — so it may be shipped in the binary.
    anon_key: String,
}

impl std::fmt::Debug for CloudConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloudConfig")
            .field("url", &self.url())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CloudConfigError {
    #[error("the cloud endpoint is not a valid URL")]
    InvalidUrl,
    #[error("the cloud endpoint must use HTTPS or WSS")]
    PlaintextEndpoint,
    #[error("the cloud endpoint uses an unsupported scheme")]
    UnsupportedScheme,
    #[cfg(any(test, feature = "test-endpoints"))]
    #[error("the plaintext cloud endpoint must use a loopback host")]
    NonLoopback,
}

impl CloudConfig {
    /// Build a production configuration. WSS is normalized to its HTTPS base.
    pub fn new(
        url: impl AsRef<str>,
        anon_key: impl Into<String>,
    ) -> Result<Self, CloudConfigError> {
        let mut url = parse_base_url(url.as_ref())?;
        match url.scheme() {
            "https" => {}
            "wss" => url
                .set_scheme("https")
                .map_err(|_| CloudConfigError::InvalidUrl)?,
            "http" | "ws" => return Err(CloudConfigError::PlaintextEndpoint),
            _ => return Err(CloudConfigError::UnsupportedScheme),
        }
        Ok(Self {
            url,
            anon_key: anon_key.into(),
        })
    }

    /// Build a configuration for a loopback-only plaintext test server.
    #[cfg(any(test, feature = "test-endpoints"))]
    pub fn new_loopback(
        url: impl AsRef<str>,
        anon_key: impl Into<String>,
    ) -> Result<Self, CloudConfigError> {
        let mut url = parse_base_url(url.as_ref())?;
        if !is_loopback(&url) {
            return Err(CloudConfigError::NonLoopback);
        }
        match url.scheme() {
            "http" => {}
            "ws" => url
                .set_scheme("http")
                .map_err(|_| CloudConfigError::InvalidUrl)?,
            "https" | "wss" => return Self::new(url.as_str(), anon_key),
            _ => return Err(CloudConfigError::UnsupportedScheme),
        }
        Ok(Self {
            url,
            anon_key: anon_key.into(),
        })
    }

    pub fn url(&self) -> &str {
        self.url.as_str()
    }

    pub fn anon_key(&self) -> &str {
        &self.anon_key
    }

    pub(crate) fn endpoint(&self, path: &str) -> Url {
        self.url
            .join(path)
            .expect("validated hierarchical cloud URL accepts absolute paths")
    }

    pub(crate) fn realtime_endpoint(&self) -> Url {
        let mut endpoint = self.endpoint("/realtime/v1/websocket");
        endpoint
            .set_scheme(if self.url.scheme() == "https" {
                "wss"
            } else {
                "ws"
            })
            .expect("validated HTTP schemes have websocket counterparts");
        endpoint
    }
}

fn parse_base_url(value: &str) -> Result<Url, CloudConfigError> {
    let url = Url::parse(value).map_err(|_| CloudConfigError::InvalidUrl)?;
    if url.cannot_be_a_base() || url.host().is_none() {
        return Err(CloudConfigError::InvalidUrl);
    }
    Ok(url)
}

#[cfg(any(test, feature = "test-endpoints"))]
fn is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        Some(Host::Domain(name)) => name.eq_ignore_ascii_case("localhost"),
        None => false,
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;

    #[test]
    fn production_accepts_secure_http_and_websocket_bases() {
        let https = CloudConfig::new("https://project.supabase.co", "key").unwrap();
        assert_eq!(https.url(), "https://project.supabase.co/");
        assert_eq!(https.realtime_endpoint().scheme(), "wss");

        let wss = CloudConfig::new("wss://project.supabase.co", "key").unwrap();
        assert_eq!(wss.url(), "https://project.supabase.co/");
        assert_eq!(wss.realtime_endpoint().scheme(), "wss");
    }

    #[test]
    fn production_rejects_plaintext_even_on_loopback() {
        for url in ["http://example.com", "ws://example.com", "http://127.0.0.1"] {
            assert_eq!(
                CloudConfig::new(url, "key").unwrap_err(),
                CloudConfigError::PlaintextEndpoint
            );
        }
    }

    #[test]
    fn malformed_and_unsupported_urls_are_rejected() {
        assert_eq!(
            CloudConfig::new("not a url", "key").unwrap_err(),
            CloudConfigError::InvalidUrl
        );
        assert_eq!(
            CloudConfig::new("ftp://example.com", "key").unwrap_err(),
            CloudConfigError::UnsupportedScheme
        );
    }

    #[test]
    fn explicit_loopback_path_accepts_http_and_ws_only_on_loopback() {
        let http = CloudConfig::new_loopback("http://127.0.0.1:54321", "key").unwrap();
        assert_eq!(http.url(), "http://127.0.0.1:54321/");
        assert_eq!(http.realtime_endpoint().scheme(), "ws");

        let ws = CloudConfig::new_loopback("ws://[::1]:54321", "key").unwrap();
        assert_eq!(ws.url(), "http://[::1]:54321/");
        assert_eq!(ws.realtime_endpoint().scheme(), "ws");

        assert_eq!(
            CloudConfig::new_loopback("http://example.com", "key").unwrap_err(),
            CloudConfigError::NonLoopback
        );
    }

    #[test]
    fn debug_output_excludes_key_material() {
        let config = CloudConfig::new("https://project.supabase.co", "anon-secret-key").unwrap();
        let rendered = format!("{config:?}");
        assert!(rendered.contains("https://project.supabase.co/"));
        assert!(!rendered.contains("anon-secret-key"));
    }
}

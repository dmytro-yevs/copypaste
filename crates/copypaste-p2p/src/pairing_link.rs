//! Versioned application URI for transferring one short-lived pairing invite.
//!
//! The URI is intentionally a custom scheme: the product owns the payload and
//! every shipped platform registers the same `copypaste://pair` handler. The
//! pairing token remains the actual authentication secret; opening the URI
//! never persists trust without the handshake-bound SAS confirmation.

use std::net::SocketAddr;

use thiserror::Error;
use url::Url;
use zeroize::Zeroizing;

use crate::PairingToken;

pub const PAIRING_URI_SCHEME: &str = "copypaste";
pub const PAIRING_URI_HOST: &str = "pair";
const PAIRING_URI_PATH: &str = "/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PairingLinkError {
    #[error("the pairing link is invalid")]
    Invalid,
    #[error("the pairing link has no reachable device address")]
    MissingAddress,
}

/// A validated invite URI whose secret code is wiped on drop.
pub struct PairingLink {
    code: Zeroizing<String>,
    pairing_id: String,
    address: Option<SocketAddr>,
}

impl PairingLink {
    pub fn new(code: &str, address: Option<SocketAddr>) -> Result<Self, PairingLinkError> {
        let token = PairingToken::parse(code).map_err(|_| PairingLinkError::Invalid)?;
        Ok(Self {
            code: Zeroizing::new(token.to_code()),
            pairing_id: token.pairing_id(),
            address,
        })
    }

    pub fn parse(value: &str) -> Result<Self, PairingLinkError> {
        let url = Url::parse(value).map_err(|_| PairingLinkError::Invalid)?;
        if url.scheme() != PAIRING_URI_SCHEME
            || url.host_str() != Some(PAIRING_URI_HOST)
            || url.path() != PAIRING_URI_PATH
            || !url.username().is_empty()
            || url.password().is_some()
            || url.port().is_some()
            || url.fragment().is_some()
        {
            return Err(PairingLinkError::Invalid);
        }

        let mut code = None;
        let mut address = None;
        for (key, value) in url.query_pairs() {
            match key.as_ref() {
                "code" if code.is_none() => code = Some(value.into_owned()),
                "address" if address.is_none() => {
                    address = Some(
                        value
                            .parse::<SocketAddr>()
                            .map_err(|_| PairingLinkError::Invalid)?,
                    );
                }
                "code" | "address" => return Err(PairingLinkError::Invalid),
                _ => {}
            }
        }
        Self::new(code.as_deref().ok_or(PairingLinkError::Invalid)?, address)
    }

    #[must_use]
    pub fn to_uri(&self) -> String {
        let mut url = Url::parse("copypaste://pair/v1").expect("static pairing URI is valid");
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("code", self.code.as_str());
            if let Some(address) = self.address {
                query.append_pair("address", &address.to_string());
            }
        }
        url.into()
    }

    #[must_use]
    pub fn pairing_id(&self) -> &str {
        &self.pairing_id
    }

    #[must_use]
    pub fn code(&self) -> &str {
        self.code.as_str()
    }

    pub fn address(&self) -> Result<SocketAddr, PairingLinkError> {
        self.address.ok_or(PairingLinkError::MissingAddress)
    }
}

impl std::fmt::Debug for PairingLink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PairingLink")
            .field("pairing_id", &self.pairing_id)
            .field("code", &"<redacted>")
            .field("address", &self.address)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pairing_link_round_trips_the_secret_and_ipv6_address() {
        let token = PairingToken::generate();
        let address = "[fd00::42]:47654".parse().unwrap();
        let link = PairingLink::new(&token.to_code(), Some(address)).unwrap();

        let parsed = PairingLink::parse(&link.to_uri()).unwrap();

        assert_eq!(parsed.code(), token.to_code());
        assert_eq!(parsed.pairing_id(), token.pairing_id());
        assert_eq!(parsed.address().unwrap(), address);
    }

    #[test]
    fn only_the_versioned_copypaste_pairing_route_is_accepted() {
        for value in [
            "https://pair/v1?code=x",
            "copypaste://other/v1?code=x",
            "copypaste://pair/v2?code=x",
            "copypaste://pair/v1#code=x",
            "copypaste://user@pair/v1?code=x",
        ] {
            assert_eq!(
                PairingLink::parse(value).unwrap_err(),
                PairingLinkError::Invalid
            );
        }
    }

    #[test]
    fn duplicate_secret_fields_are_rejected() {
        let token = PairingToken::generate().to_code();
        let value = format!("copypaste://pair/v1?code={token}&code={token}");
        assert_eq!(
            PairingLink::parse(&value).unwrap_err(),
            PairingLinkError::Invalid
        );
    }

    #[test]
    fn debug_never_contains_the_pairing_code() {
        let token = PairingToken::generate().to_code();
        let link = PairingLink::new(&token, None).unwrap();
        let rendered = format!("{link:?}");
        assert!(!rendered.contains(&token));
        assert!(rendered.contains("<redacted>"));
    }
}

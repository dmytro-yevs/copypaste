//! LAN peer discovery over mDNS-SD.
//!
//! # Discovery is a convenience, never a dependency
//!
//! Every path in the daemon works with an explicit `host:port`, so *nothing
//! here is allowed to fail loudly*: [`Discovery::start`] returns a working
//! handle even when the host forbids multicast, in which case
//! [`Discovery::peers`] stays empty and the reason is logged at debug. That
//! degraded mode is the normal case in our own test environment, so the tests
//! in [`service`] exercise it.
//!
//! # Presence is not trust
//!
//! A [`DiscoveredPeer`] means "something on this LAN answered an mDNS query and
//! claimed this pairing id", and anyone on the network can claim anything.
//! Trust comes only from the Noise `NNpsk0` handshake in [`crate::transport`],
//! which requires the pairing token; discovery supplies a candidate address to
//! try, so a hostile advertiser can waste one handshake attempt and no more.
//! Nothing derived from the token is ever advertised — see [`record`].

mod error;
mod names;
mod record;
mod service;
mod table;

pub use error::DiscoveryError;
pub use record::{MAX_ADVERTISED_PAIRING_IDS, MAX_PAIRING_IDS_PER_PEER};
pub use service::Discovery;
pub use table::{DiscoveredPeer, MAX_PEERS, PEER_TTL};

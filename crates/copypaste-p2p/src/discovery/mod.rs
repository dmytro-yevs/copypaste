//! LAN peer discovery over mDNS-SD.
//!
//! # Discovery is a convenience, never a dependency
//!
//! Every path in the daemon works with an explicit `host:port`. This module
//! only saves the user typing one. Consequently *nothing here is allowed to
//! fail loudly*: [`Discovery::start`] returns a working handle even when the
//! host forbids multicast, in which case [`Discovery::peers`] simply stays
//! empty and the reason is logged at debug. That degraded mode is the normal
//! case in our own test environment, so it is exercised by the tests in
//! [`service`].
//!
//! # Presence is not trust
//!
//! A [`DiscoveredPeer`] means "something on this LAN answered an mDNS query and
//! claimed this pairing id". Anyone on the network can claim anything. Trust
//! comes only from the Noise `NNpsk0` handshake in [`crate::transport`], which
//! requires the pairing token; discovery just supplies a candidate address to
//! try. A hostile advertiser can therefore waste one handshake attempt and
//! nothing more.
//!
//! # Why mdns-sd rather than our own responder
//!
//! Per `CLAUDE.md` rule 1: mDNS is a real protocol with probing, conflict
//! resolution, cache coherency and known-answer suppression. `mdns-sd` is
//! maintained, pure Rust, and works on both macOS and Android. The only code
//! here is the CopyPaste-specific part — what we put in TXT, and how long we
//! believe it.
//!
//! # Layout
//!
//! Split on the line between what is pure and what talks to a daemon, because
//! that is the line the tests need:
//!
//! * [`record`] — the TXT contract, encode and decode. Pure. Owns the rule
//!   that nothing derived from the pairing token may ever be advertised.
//! * [`table`] — what we currently believe about the LAN: expiry against a
//!   clock passed in, and the cap that makes an mDNS flood harmless. Pure.
//! * [`names`] — the sanitisers, shared by both directions: DNS label rules on
//!   the way out, hostile-input bounding on the way in. Pure.
//! * [`service`] — the mDNS daemon, the advertisement lifecycle, and the two
//!   background threads. The only file here that opens a socket.

mod error;
mod names;
mod record;
mod service;
mod table;

pub use error::DiscoveryError;
pub use record::{MAX_ADVERTISED_PAIRING_IDS, MAX_PAIRING_IDS_PER_PEER};
pub use service::Discovery;
pub use table::{DiscoveredPeer, MAX_PEERS, PEER_TTL};

//! The relay wire protocol, shared by the server and the device client.
//!
//! Every WebSocket opens with one text [`Hello`]. A presence connection then
//! answers a [`Event::Challenge`] with [`Control::Register`] and receives
//! events; a connect or accept connection waits for [`Event::Ready`], after
//! which only binary frames flow and the relay forwards them unchanged. A punch
//! connection trades UDP candidates with the peer and closes, so the devices
//! can try a direct path before falling back to a spliced one.
//!
//! The UDP observer answers one datagram with the address it came from, which
//! is how a device learns its public UDP address without a third party.

use std::net::SocketAddr;

use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

/// Largest binary frame: one Noise message.
pub const MAX_FRAME_BYTES: usize = 65_535;
/// Largest text control message.
pub const MAX_CONTROL_BYTES: usize = 8 * 1024;
/// Rendezvous ids one presence connection may hold.
pub const MAX_REGISTRATIONS: usize = 32;
/// UDP addresses one side may offer for a punch.
pub const MAX_CANDIDATES: usize = 4;
/// An observer request is padded to this size so the reply is never larger
/// than what was sent: the observer cannot amplify spoofed traffic.
pub const OBSERVE_REQUEST_BYTES: usize = 96;
const OBSERVE_MAGIC: &[u8; 5] = b"CPRO1";

pub type Id = [u8; 32];
pub type Slot = [u8; 16];
pub type Ticket = [u8; 16];
pub type Nonce = [u8; 32];

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Hello {
    Presence {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        token: Option<String>,
    },
    Connect {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        token: Option<String>,
        #[serde(with = "hex_bytes")]
        id: Id,
        #[serde(with = "hex_bytes")]
        slot: Slot,
    },
    Accept {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        token: Option<String>,
        #[serde(with = "hex_bytes")]
        ticket: Ticket,
    },
    /// Offer UDP candidates to the device registered on `id` and wait for its
    /// answer.
    Punch {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        token: Option<String>,
        #[serde(with = "hex_bytes")]
        id: Id,
        #[serde(with = "hex_bytes")]
        slot: Slot,
        candidates: Vec<SocketAddr>,
    },
}

/// What a presence connection sends after the challenge.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Control {
    Register(Register),
    PunchAnswer {
        #[serde(with = "hex_bytes")]
        ticket: Ticket,
        candidates: Vec<SocketAddr>,
    },
}

/// Replaces everything a presence connection is registered under.
#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Register {
    /// Pairing rendezvous, each proven by a signature over the challenge.
    pub signed: Vec<Signed>,
    /// Short-lived pairing invitations: unauthenticated by design, since the
    /// PAKE inside the relayed stream is what authenticates them.
    #[serde(default)]
    pub open: Vec<HexId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Signed {
    #[serde(with = "hex_bytes")]
    pub id: Id,
    #[serde(with = "hex_bytes")]
    pub slot: Slot,
    #[serde(with = "hex_bytes")]
    pub sig: [u8; 64],
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct HexId(#[serde(with = "hex_bytes")] pub Id);

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Event {
    Challenge {
        #[serde(with = "hex_bytes")]
        nonce: Nonce,
        /// Where the UDP observer listens: `host:port`, or `:port` on the
        /// relay's own host. Absent when the relay offers no direct paths.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        observe: Option<String>,
    },
    /// The signed ids on which another device is currently present.
    Online {
        ids: Vec<HexId>,
    },
    /// Someone dialled one of this connection's ids; open an accept
    /// connection with this ticket.
    Incoming {
        #[serde(with = "hex_bytes")]
        ticket: Ticket,
    },
    Ready,
    /// A peer offers UDP candidates; answer with [`Control::PunchAnswer`].
    Punch {
        #[serde(with = "hex_bytes")]
        ticket: Ticket,
        candidates: Vec<SocketAddr>,
    },
    PunchAnswer {
        candidates: Vec<SocketAddr>,
    },
    Error {
        code: ErrorCode,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Unauthorized,
    Offline,
    Busy,
    Invalid,
}

/// The bytes a registration signs. The nonce is fresh per connection, so a
/// captured registration cannot be replayed on another one.
#[must_use]
pub fn signed_message(nonce: &Nonce, id: &Id, slot: &Slot) -> Vec<u8> {
    let mut message = Vec::with_capacity(18 + 32 + 32 + 16);
    message.extend_from_slice(b"copypaste-relay/v1");
    message.extend_from_slice(nonce);
    message.extend_from_slice(id);
    message.extend_from_slice(slot);
    message
}

/// Whether `signed` proves possession of the key whose public half is its id.
#[must_use]
pub fn verify(nonce: &Nonce, signed: &Signed) -> bool {
    let Ok(key) = VerifyingKey::from_bytes(&signed.id) else {
        return false;
    };
    key.verify_strict(
        &signed_message(nonce, &signed.id, &signed.slot),
        &Signature::from_bytes(&signed.sig),
    )
    .is_ok()
}

/// An observer request: magic, `nonce`, zero padding.
#[must_use]
pub fn observe_request(nonce: &[u8; 16]) -> [u8; OBSERVE_REQUEST_BYTES] {
    let mut request = [0u8; OBSERVE_REQUEST_BYTES];
    request[..5].copy_from_slice(OBSERVE_MAGIC);
    request[5..21].copy_from_slice(nonce);
    request
}

/// The observer's reply to `request` from `from`: magic, the echoed nonce and
/// the address as text. `None` for anything that is not a request.
#[must_use]
pub fn observe_reply(request: &[u8], from: SocketAddr) -> Option<Vec<u8>> {
    if request.len() != OBSERVE_REQUEST_BYTES || &request[..5] != OBSERVE_MAGIC {
        return None;
    }
    let mut reply = request[..21].to_vec();
    reply.extend_from_slice(from.to_string().as_bytes());
    Some(reply)
}

/// The address an observer saw, if `reply` answers the request for `nonce`.
#[must_use]
pub fn parse_observe_reply(reply: &[u8], nonce: &[u8; 16]) -> Option<SocketAddr> {
    if reply.len() <= 21 || &reply[..5] != OBSERVE_MAGIC || &reply[5..21] != nonce {
        return None;
    }
    std::str::from_utf8(&reply[21..]).ok()?.parse().ok()
}

mod hex_bytes {
    use serde::{de::Error, Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer, const N: usize>(
        bytes: &[u8; N],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&hex::encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>, const N: usize>(
        deserializer: D,
    ) -> Result<[u8; N], D::Error> {
        let text = <&str>::deserialize(deserializer)?;
        let mut out = [0u8; N];
        hex::decode_to_slice(text, &mut out).map_err(D::Error::custom)?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    #[test]
    fn a_registration_verifies_only_under_its_own_challenge() {
        let key = SigningKey::from_bytes(&[7; 32]);
        let id = key.verifying_key().to_bytes();
        let slot = [1; 16];
        let nonce = [2; 32];
        let signed = Signed {
            id,
            slot,
            sig: key.sign(&signed_message(&nonce, &id, &slot)).to_bytes(),
        };
        assert!(verify(&nonce, &signed));
        assert!(!verify(&[3; 32], &signed));
        assert!(!verify(
            &nonce,
            &Signed {
                slot: [9; 16],
                ..signed
            }
        ));
    }

    #[test]
    fn the_observer_echoes_the_sender_address_without_amplifying() {
        let nonce = [4; 16];
        let from: SocketAddr = "203.0.113.7:40000".parse().unwrap();
        let request = observe_request(&nonce);
        let reply = observe_reply(&request, from).unwrap();
        assert!(reply.len() <= request.len());
        assert_eq!(parse_observe_reply(&reply, &nonce), Some(from));
        assert_eq!(parse_observe_reply(&reply, &[5; 16]), None);
        assert_eq!(observe_reply(&request[..40], from), None);
    }

    #[test]
    fn messages_round_trip_as_tagged_json() {
        let hello = Hello::Connect {
            token: None,
            id: [0xab; 32],
            slot: [0xcd; 16],
        };
        let text = serde_json::to_string(&hello).unwrap();
        assert!(text.starts_with(r#"{"t":"connect""#));
        assert_eq!(serde_json::from_str::<Hello>(&text).unwrap(), hello);
        assert!(serde_json::from_str::<Hello>(r#"{"t":"accept","ticket":"zz"}"#).is_err());
    }
}

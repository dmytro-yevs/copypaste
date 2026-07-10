//! Discovery-pairing state machine domain types (role, terminal-outputs
//! payload, and the machine itself). Ported from the macOS daemon's
//! `pairing_sm::PairingState` — see the `pairing` module doc comment for the
//! full protocol/security rationale.

use crate::SyncProvisioning;

/// Which side of the discovery handshake this device is playing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingRole {
    /// This device dialed the peer (`pair_with_discovered`).
    Initiator,
    /// This device accepted an inbound discovery-pair connection (standing
    /// responder — makes Android pairable FROM macOS).
    Responder,
}

impl PairingRole {
    /// Lowercase wire string surfaced in the `pair_get_sas` FFI response.
    pub fn as_str(self) -> &'static str {
        match self {
            PairingRole::Initiator => "initiator",
            PairingRole::Responder => "responder",
        }
    }
}

/// The FFI-safe outputs of a CONFIRMED bootstrap pairing, captured into the
/// terminal `Confirmed` state so Kotlin can persist them after polling.
///
/// # SECURITY NOTE — `session_key` crosses the FFI boundary unzeroized.
/// UniFFI copies it into a Kotlin `ByteArray`. The Kotlin layer MUST zero that
/// array after deriving the content sync key from it (KEK-wrap into the
/// AndroidKeystore); never log it.
#[derive(Debug, Clone)]
pub struct ConfirmedPairing {
    /// The peer's pinned cert fingerprint (hex SHA-256 of its cert DER).
    pub peer_fingerprint: String,
    /// The peer's P2P sync-listener address (`host:port`), sent in-band.
    pub peer_sync_addr: String,
    /// The 32-byte PAKE+channel-bound session key (identical on both sides).
    pub session_key: Vec<u8>,
    /// Sync-account provisioning the PEER advertised over the authenticated
    /// tunnel. `None` when the peer advertised nothing or is a legacy build.
    pub peer_provisioning: Option<SyncProvisioning>,
    /// HB-1b (ABI 14): the PEER's device metadata learned in-band during the
    /// discovery/SAS pairing, sourced from `BootstrapPairing.peer_*`. All `None`
    /// for a legacy peer. Surfaced to Kotlin via [`super::dto::PairStatus`] on
    /// `confirmed`.
    pub peer_model: Option<String>,
    pub peer_os: Option<String>,
    pub peer_app_version: Option<String>,
    pub peer_local_ip: Option<String>,
    pub peer_public_ip: Option<String>,
    /// ABI 17 (CopyPaste-3k6m): the PEER's stable device UUID (from
    /// `PeerMeta.device_id`), learned in-band during the discovery/SAS pairing.
    /// `None` for legacy peers. Surfaced to Kotlin via `PairStatus` on
    /// `confirmed` state.
    pub peer_device_id: Option<String>,
    /// ABI 19 (CopyPaste-gldr): the PEER's non-secret Supabase/cloud account
    /// id, learned in-band during the discovery/SAS pairing, sourced from
    /// `BootstrapPairing.peer_supabase_account_id`. `None` for legacy peers or
    /// when the peer has no cloud account configured. Surfaced to Kotlin via
    /// `PairStatus` on `confirmed` state so Android can detect cross-account
    /// pairing mismatches, at parity with the macOS daemon (CopyPaste-yw2k).
    pub peer_supabase_account_id: Option<String>,
}

/// The discovery-pairing state machine. Ported from the macOS daemon's
/// `pairing_sm::PairingState`, with the terminal `Confirmed` extended to carry
/// the FFI-safe outputs ([`ConfirmedPairing`]) so Kotlin persists them.
///
/// Non-terminal: `Idle`, `Initiating`, `AwaitingSas`.
/// Terminal: `Confirmed`, `Rejected`, `Aborted`, `TimedOut`.
// ABI 14 grew `ConfirmedPairing` (the `Confirmed` payload) with five peer_*
// metadata strings, tripping `clippy::large_enum_variant`. Boxing the payload
// would add an allocation + indirection on the hot read path of a state machine
// that holds AT MOST ONE live instance at a time (single active pairing), so the
// "wasted bytes in the other variants" the lint warns about never materialise in
// practice — there is never an array/Vec of PairingState. The clarity of an
// inline payload outweighs shaving one short-lived enum's footprint.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum PairingState {
    /// No pairing in flight — the only state in which a new pairing may start.
    Idle,
    /// A pairing has started; the bootstrap handshake is running but the SAS is
    /// not yet known (pre-frame-9).
    Initiating {
        /// Role this device is playing.
        role: PairingRole,
    },
    /// The handshake reached frame 9, the SAS is derived, and the daemon is
    /// waiting for the local user's accept/reject decision.
    AwaitingSas {
        /// The 6-digit decimal SAS to display to the user.
        sas: String,
        /// Role this device is playing.
        role: PairingRole,
    },
    /// Both sides accepted the SAS — the FFI-safe outputs are captured here for
    /// Kotlin to persist.
    Confirmed(ConfirmedPairing),
    /// The local user (or the peer) rejected the SAS — keys dropped, nothing
    /// persisted.
    Rejected,
    /// The pairing was explicitly aborted (`pair_abort`) — keys dropped.
    Aborted,
    /// The confirmation window elapsed with no decision — keys dropped.
    TimedOut,
}

impl PairingState {
    /// `true` when a new pairing may be started (the machine is `Idle`).
    pub fn is_idle(&self) -> bool {
        matches!(self, PairingState::Idle)
    }

    /// `true` when the machine is mid-pairing (not `Idle`, not terminal).
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            PairingState::Initiating { .. } | PairingState::AwaitingSas { .. }
        )
    }

    /// `true` for the four terminal outcomes.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            PairingState::Confirmed(_)
                | PairingState::Rejected
                | PairingState::Aborted
                | PairingState::TimedOut
        )
    }

    /// Lowercase wire string surfaced in the `pair_get_sas` FFI response so
    /// Kotlin can branch on a stable, serialisable token.
    pub fn as_str(&self) -> &'static str {
        match self {
            PairingState::Idle => "idle",
            PairingState::Initiating { .. } => "initiating",
            PairingState::AwaitingSas { .. } => "awaiting_sas",
            PairingState::Confirmed(_) => "confirmed",
            PairingState::Rejected => "rejected",
            PairingState::Aborted => "aborted",
            PairingState::TimedOut => "timed_out",
        }
    }

    /// The SAS string when in [`PairingState::AwaitingSas`], else `None`.
    pub fn sas(&self) -> Option<&str> {
        match self {
            PairingState::AwaitingSas { sas, .. } => Some(sas.as_str()),
            _ => None,
        }
    }

    /// The role when in an active state, else `None`.
    pub fn role(&self) -> Option<PairingRole> {
        match self {
            PairingState::Initiating { role } | PairingState::AwaitingSas { role, .. } => {
                Some(*role)
            }
            _ => None,
        }
    }

    /// The captured outputs when in [`PairingState::Confirmed`], else `None`.
    pub fn confirmed(&self) -> Option<&ConfirmedPairing> {
        match self {
            PairingState::Confirmed(c) => Some(c),
            _ => None,
        }
    }
}

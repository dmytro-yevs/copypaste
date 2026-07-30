//! The peer operations, on the same [`CopyPaste`] object.
//!
//! Separate file because these are the only calls in the crate that touch the
//! network, and the only ones that are `async`. UniFFI turns an exported
//! `async fn` into a Kotlin `suspend fun`, which is what a Compose ViewModel
//! wants; the history operations in [`crate::store`] stay blocking and belong
//! on `Dispatchers.IO`.
//!
//! # The pairing code is a credential
//!
//! [`CopyPaste::create_pairing`] renders it exactly once, into the return
//! value. Nothing here logs it — not at trace, not in an error, not through a
//! `Debug` impl, because `PairingToken`'s `Debug` is redacted and `to_code` is
//! called in exactly one place in this crate. Every message about a pairing
//! uses the derived, non-secret `pairing_id` instead.
//!
//! # What is missing, and why it is reported rather than approximated
//!
//! [`CopyPaste::sync_peer`] returns [`CopyPasteError::SyncUnavailable`]. It is
//! not a stub that pretends: it is a capability this crate cannot provide on
//! `copypaste-core`'s public API, and the honest failure is the one that does
//! not silently corrupt a user's history.
//!
//! A sync session is driven by `copypaste_p2p::sync::run_initiator`, which
//! needs a `SyncSource`. Three of that trait's four methods need facts about an
//! item that `copypaste_core::Store` does not expose:
//!
//! * `summaries()` needs `content_hash` and the tombstone flag for every item,
//!   live and deleted. `StoredItem` carries neither, and `Store::list` filters
//!   tombstones out — so this device could not advertise its deletes, and
//!   delete-wins (a binding rule of manifest 05) would resurrect every item the
//!   user had deleted, on every sync, forever.
//! * `fetch()` needs `content_hash` and `origin_device_id`. There is no origin
//!   column at all, and the origin is the merge's tie-break: without it the
//!   comparator is not total and two devices can disagree about who won.
//! * `apply()` needs an **upsert** — a remote version that supersedes a local
//!   one has to overwrite it. `Store` has `insert`, which collides on the
//!   primary key, and `delete`. There is no way to express the write.
//!
//! The daemon closes this gap in `copypaste-daemon`'s `p2p::meta` by opening
//! the same SQLCipher file on a second connection and reading the columns
//! directly. That module's own header calls it "a layering compromise, and it
//! should be repaid", and names the fix: `content_hash` and `deleted` on
//! `StoredItem`, `Store::summaries()`, `Store::upsert()`, and an
//! `origin_device_id` column. It also lives in a binary crate, so it cannot be
//! reused from here.
//!
//! Reproducing it in this crate would be a **third** implementation of "what is
//! in this device's history" — precisely the duplication `CLAUDE.md` rule 1
//! exists to stop, and precisely the kind that drifts the first time an
//! eviction lands on one copy and not the other. So it is not reproduced. When
//! those four additions land in `copypaste-core`, the body of `sync_peer`
//! becomes the same twenty lines the daemon's `run_session` already is.
//!
//! What *is* implemented is everything that does not need a session:
//! [`CopyPaste::create_pairing`], [`CopyPaste::accept_pairing`] (which performs
//! a real Noise handshake, so the pairing is proven rather than assumed),
//! [`CopyPaste::list_peers`], [`CopyPaste::unpair`] and
//! [`CopyPaste::check_peer`].

use std::net::SocketAddr;

use copypaste_p2p::peers::Peer;
use copypaste_p2p::transport::{PairingToken, Session};

use crate::error::CopyPasteError;
use crate::store::CopyPaste;
use crate::types::{NewPairing, PairedDevice, SyncReport};

/// How long a handshake attempt is given before the peer is called unreachable.
///
/// `Session::connect` has its own `HANDSHAKE_TIMEOUT`, but that starts once the
/// TCP connection is up. A phone on a captive-portal Wi-Fi can sit in `connect`
/// for a great deal longer than that, and a spinner on the Devices screen needs
/// to end.
const DIAL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

#[uniffi::export(async_runtime = "tokio")]
impl CopyPaste {
    /// Mint a pairing and return the code to read out to the other device.
    ///
    /// [`NewPairing::code`] is a live credential and the caveats on
    /// [`NewPairing`] apply in full: show it, do not log it, do not put it on
    /// the clipboard, and hold [`NewPairing::pairing_id`] rather than the code
    /// for everything that comes after.
    ///
    /// The peer is stored **before this returns**, before the other device has
    /// been heard from, because the pre-shared key has to be in this device's
    /// candidate list before the other half can dial in. `name` is a
    /// placeholder until the first session tells us what the peer calls itself.
    pub fn create_pairing(&self, name: String) -> Result<NewPairing, CopyPasteError> {
        let token = PairingToken::generate();
        let pairing_id = token.pairing_id();

        self.peers.upsert(Peer {
            pairing_id: pairing_id.clone(),
            name: placeholder_name(&name),
            psk: token.psk(),
            last_addr: None,
            last_seen_ms: 0,
        })?;

        // The id, never the code. This is the only `tracing` call in the crate
        // anywhere near a token, and what it prints is a one-way digest.
        tracing::info!(%pairing_id, "minted a pairing");

        Ok(NewPairing {
            // The one and only rendering of the secret in this crate.
            code: token.to_code(),
            pairing_id,
        })
    }

    /// Consume a code minted on another device, and prove the pairing works.
    ///
    /// Parses the code, dials `addr` (`host:port` — a hostname is fine, which
    /// is what mDNS discovery would have given the user) and completes the
    /// Noise `NNpsk0` handshake. Possession of the token *is* the
    /// authentication: there is no certificate and no trust store, so a
    /// handshake that completes is proof that both ends hold the same secret.
    ///
    /// The peer is persisted **only after the handshake succeeds**. A code that
    /// does not parse, an address that does not answer, or a handshake the
    /// other end refuses all leave the paired-device list untouched: a stored
    /// pairing is one that has worked at least once.
    ///
    /// # Errors
    ///
    /// [`CopyPasteError::InvalidPairingCode`] — deliberately without saying how
    /// the code was wrong. [`CopyPasteError::InvalidAddress`],
    /// [`CopyPasteError::PairingRefused`], [`CopyPasteError::PeerUnreachable`],
    /// [`CopyPasteError::PeerStore`].
    pub async fn accept_pairing(
        &self,
        code: String,
        addr: String,
        name: String,
    ) -> Result<PairedDevice, CopyPasteError> {
        // Nothing is logged about a parse failure: the input is a secret, and
        // saying *how* it was wrong is a hint about what a valid one looks like.
        let token = PairingToken::parse(&code).map_err(|_| CopyPasteError::InvalidPairingCode)?;
        let pairing_id = token.pairing_id();

        let addr = resolve(&addr)
            .await
            .ok_or(CopyPasteError::InvalidAddress)?;

        let session = dial(addr, &token.psk()).await?;
        // Nothing else is said over this connection — a session needs a
        // `SyncSource` this crate cannot build (see the module docs) — so the
        // channel is shut down cleanly rather than left for the peer's read
        // deadline to collect.
        if let Err(e) = session.close().await {
            tracing::debug!(%pairing_id, error = %e, "pairing session did not close cleanly");
        }

        let peer = Peer {
            pairing_id: pairing_id.clone(),
            name: placeholder_name(&name),
            psk: token.psk(),
            last_addr: Some(addr),
            last_seen_ms: copypaste_core::now_ms(),
        };
        let device = describe(&peer);
        self.peers.upsert(peer)?;

        tracing::info!(%pairing_id, "accepted a pairing");
        Ok(device)
    }

    /// Every paired device, newest contact first.
    pub fn list_peers(&self) -> Vec<PairedDevice> {
        let mut peers: Vec<PairedDevice> = self.peers.list().iter().map(describe).collect();
        // Total order: `last_seen_ms` ties (two peers that have never been
        // reached both sit at 0) break on the id, so the list does not shuffle
        // between polls and a Compose `key {}` stays stable.
        peers.sort_by(|a, b| {
            b.last_seen_ms
                .cmp(&a.last_seen_ms)
                .then_with(|| a.pairing_id.cmp(&b.pairing_id))
        });
        peers
    }

    /// Forget a pairing. Returns whether there was one to forget.
    ///
    /// Local and immediate: the pre-shared key is dropped, so this device will
    /// no longer accept a connection from that peer and can no longer dial it.
    /// The other device keeps its half until it is unpaired there too — there
    /// is no account and no server to revoke through, which is the same
    /// property that means there is nothing to leak.
    pub fn unpair(&self, pairing_id: String) -> Result<bool, CopyPasteError> {
        let removed = self.peers.remove(&pairing_id)?;
        if removed {
            tracing::info!(%pairing_id, "unpaired a device");
        }
        Ok(removed)
    }

    /// Dial a paired device and confirm it is reachable and still holds the
    /// pairing, updating its last-seen time.
    ///
    /// This is a handshake, not a sync: it proves the pre-shared key and the
    /// address, which is what the Devices screen needs to show a peer as
    /// online without claiming any clipboard history moved.
    ///
    /// # Errors
    ///
    /// [`CopyPasteError::PeerNotFound`],
    /// [`CopyPasteError::PeerAddressUnknown`] — the pairing was minted here and
    /// the other device has never connected, so we have no address to dial.
    /// [`CopyPasteError::PeerUnreachable`], [`CopyPasteError::PairingRefused`].
    pub async fn check_peer(&self, pairing_id: String) -> Result<PairedDevice, CopyPasteError> {
        // `Peer` owns key material and zeroizes on drop, so it is not moved
        // apart field by field — the whole record is carried forward and the
        // two facts a successful check learns are written onto it.
        let mut peer = self
            .peers
            .get(&pairing_id)
            .ok_or(CopyPasteError::PeerNotFound)?;
        let addr = peer.last_addr.ok_or(CopyPasteError::PeerAddressUnknown)?;

        let session = dial(addr, &peer.psk).await?;
        if let Err(e) = session.close().await {
            tracing::debug!(%pairing_id, error = %e, "peer check did not close cleanly");
        }

        peer.last_addr = Some(addr);
        peer.last_seen_ms = copypaste_core::now_ms();
        let device = describe(&peer);
        self.peers.upsert(peer)?;
        Ok(device)
    }

    /// Sync clipboard history with a paired device.
    ///
    /// **Always returns [`CopyPasteError::SyncUnavailable`] in this build.**
    /// The reason is a missing capability in `copypaste-core`, not a runtime
    /// fault, and it is set out in full in this module's documentation. The
    /// method exists so the app's Devices screen, its per-peer result state and
    /// its error copy are built and exercised against the real signature; the
    /// day `Store` grows `summaries`/`upsert`/`origin_device_id` this body
    /// becomes a session and nothing above it changes.
    ///
    /// It is deliberately not approximated. A sync built on the columns that
    /// *are* public could not advertise tombstones, so every delete the user
    /// made would be undone by the next sync — silent, repeated data loss,
    /// which `CLAUDE.md` rule 4 ranks as the worst outcome available.
    pub async fn sync_peer(&self, pairing_id: String) -> Result<SyncReport, CopyPasteError> {
        // Still checked, so the app gets "no such device" for a stale id rather
        // than a capability error that would send a reader looking in the wrong
        // place.
        if self.peers.get(&pairing_id).is_none() {
            return Err(CopyPasteError::PeerNotFound);
        }
        tracing::warn!(%pairing_id, "sync requested, but this build cannot run a session");
        Err(CopyPasteError::SyncUnavailable)
    }
}

// --------------------------------------------------------------- private glue

/// Dial and handshake, with a bound on the whole attempt.
async fn dial(addr: SocketAddr, psk: &[u8; 32]) -> Result<Session, CopyPasteError> {
    match tokio::time::timeout(DIAL_TIMEOUT, Session::connect(addr, psk)).await {
        Ok(Ok(session)) => Ok(session),
        Ok(Err(e)) => {
            // `TransportError`'s own variants are already indistinguishable
            // where it matters (manifest 02 I-15); this only separates "the
            // socket never came up" from "the other end said no".
            tracing::debug!(error = %e, "peer handshake failed");
            Err(e.into())
        }
        Err(_) => Err(CopyPasteError::PeerUnreachable),
    }
}

/// Resolve `host:port`, preferring IPv4.
///
/// A real lookup rather than a `SocketAddr` parse: `macbook.local:47654` is
/// exactly what mDNS discovery would have handed the user.
async fn resolve(addr: &str) -> Option<SocketAddr> {
    let resolved: Vec<SocketAddr> = tokio::net::lookup_host(addr).await.ok()?.collect();
    resolved
        .iter()
        .find(|a| a.is_ipv4())
        .or(resolved.first())
        .copied()
}

/// Project a peer into the value Kotlin sees. Never the PSK.
fn describe(peer: &Peer) -> PairedDevice {
    PairedDevice {
        pairing_id: peer.pairing_id.clone(),
        name: peer.name.clone(),
        last_addr: peer.last_addr.map(|a| a.to_string()),
        last_seen_ms: peer.last_seen_ms,
    }
}

/// A peer always has *some* name — the list is unreadable otherwise — and the
/// real one arrives with the first session.
fn placeholder_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        "Unnamed device".to_string()
    } else {
        trimmed.chars().take(64).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::open;

    #[test]
    fn a_new_pairing_is_stored_before_the_code_is_handed_out() {
        // The peer has to be in the candidate list before the other device can
        // dial in, so it is already listed when the caller gets the code.
        let (cp, _dir) = open();
        let minted = cp.create_pairing("MacBook".into()).unwrap();

        // The code round-trips to the same pairing id, which is what lets the
        // other device find its half — and what makes the id in the record the
        // right handle to keep.
        let parsed = PairingToken::parse(&minted.code).unwrap();
        assert_eq!(parsed.pairing_id(), minted.pairing_id);

        let peers = cp.list_peers();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].pairing_id, minted.pairing_id);
        assert_eq!(peers[0].name, "MacBook");
        assert_eq!(peers[0].last_addr, None);
    }

    #[test]
    fn the_pairing_code_is_not_the_pairing_id() {
        // The id is a one-way digest of the token. If either contained the
        // other, every log line carrying an id would be carrying the secret.
        let (cp, _dir) = open();
        let minted = cp.create_pairing("MacBook".into()).unwrap();
        assert_ne!(minted.code, minted.pairing_id);
        assert!(!minted.pairing_id.contains(&minted.code));
        assert!(!minted.code.contains(&minted.pairing_id));
    }

    #[test]
    fn debugging_a_new_pairing_does_not_print_the_code() {
        // The module header promises nothing logs the code. A derived `Debug`
        // would make that false the first time anyone wrote
        // `tracing::debug!(?minted, ..)`, and logs outlive the process.
        let (cp, _dir) = open();
        let minted = cp.create_pairing("MacBook".into()).unwrap();
        let rendered = format!("{minted:?}");
        assert!(!rendered.contains(&minted.code), "the code is in {rendered}");
        assert!(rendered.contains(&minted.pairing_id), "the id should be there");
        assert!(rendered.contains("redacted"));
    }

    #[test]
    fn no_value_that_leaves_this_module_carries_the_pre_shared_key() {
        // `PairedDevice` becomes a Kotlin `data class`, so its `toString` is
        // generated and prints every field. Nothing in it may be the secret.
        let (cp, _dir) = open();
        let minted = cp.create_pairing("MacBook".into()).unwrap();
        let token = PairingToken::parse(&minted.code).unwrap();
        let psk_hex = hex_of(&token.psk());

        for peer in cp.list_peers() {
            let rendered = format!("{peer:?}");
            assert!(!rendered.contains(&psk_hex), "PSK in {rendered}");
            assert!(!rendered.contains(&minted.code), "pairing code in {rendered}");
        }
    }

    #[test]
    fn a_pairing_survives_reopening_the_store() {
        // The paired-device file is the only copy: losing it costs the user a
        // manual re-pair of every device.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_string_lossy().into_owned();
        let cp = CopyPaste::open(path.clone(), [42u8; 32].to_vec(), "Pixel".into()).unwrap();
        let minted = cp.create_pairing("MacBook".into()).unwrap();
        drop(cp);

        let cp = CopyPaste::open(path, [42u8; 32].to_vec(), "Pixel".into()).unwrap();
        assert_eq!(cp.list_peers()[0].pairing_id, minted.pairing_id);
    }

    #[test]
    fn unpairing_removes_it_and_is_idempotent() {
        let (cp, _dir) = open();
        let minted = cp.create_pairing("MacBook".into()).unwrap();
        assert!(cp.unpair(minted.pairing_id.clone()).unwrap());
        assert!(!cp.unpair(minted.pairing_id).unwrap());
        assert!(cp.list_peers().is_empty());
    }

    #[test]
    fn a_peer_with_no_name_still_has_one() {
        let (cp, _dir) = open();
        cp.create_pairing("   ".into()).unwrap();
        assert!(!cp.list_peers()[0].name.is_empty());
    }

    #[test]
    fn the_peer_list_order_is_total_and_stable() {
        let (cp, _dir) = open();
        for n in 0..5 {
            cp.create_pairing(format!("device {n}")).unwrap();
        }
        // All five have never been reached, so every `last_seen_ms` is 0 and
        // only the id tiebreak keeps the order from shuffling under a Compose
        // `key {}`.
        let first = cp.list_peers();
        assert_eq!(first.len(), 5);
        for _ in 0..3 {
            assert_eq!(cp.list_peers(), first);
        }
    }

    #[tokio::test]
    async fn a_malformed_pairing_code_is_refused_without_touching_the_network() {
        let (cp, _dir) = open();
        let err = cp
            .accept_pairing(
                "not-a-code".into(),
                // Would be rejected as an address too, if we ever got there.
                "203.0.113.1:47654".into(),
                "MacBook".into(),
            )
            .await
            .unwrap_err();
        assert_eq!(err, CopyPasteError::InvalidPairingCode);
        assert!(cp.list_peers().is_empty(), "nothing may be stored");
    }

    #[tokio::test]
    async fn an_unresolvable_address_is_refused_and_stores_nothing() {
        let (cp, _dir) = open();
        let code = PairingToken::generate().to_code();
        let err = cp
            .accept_pairing(code, "this is not an address".into(), "MacBook".into())
            .await
            .unwrap_err();
        assert_eq!(err, CopyPasteError::InvalidAddress);
        assert!(cp.list_peers().is_empty());
    }

    #[tokio::test]
    async fn a_pairing_that_never_handshakes_is_not_stored() {
        // A stored pairing means a pairing that has worked at least once.
        // Port 1 on the loopback has nothing listening.
        let (cp, _dir) = open();
        let code = PairingToken::generate().to_code();
        let err = cp
            .accept_pairing(code, "127.0.0.1:1".into(), "MacBook".into())
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            CopyPasteError::PairingRefused | CopyPasteError::PeerUnreachable
        ));
        assert!(cp.list_peers().is_empty());
    }

    #[tokio::test]
    async fn accepting_a_pairing_completes_a_real_handshake_and_stores_the_peer() {
        // The other half is a bare `Session::accept` on a loopback listener:
        // this is the actual Noise NNpsk0 exchange, not a mock.
        let (cp, _dir) = open();
        let token = PairingToken::generate();
        let code = token.to_code();
        let psk = token.psk();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let peer_side = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            Session::accept(stream, &psk).await.map(|_| ())
        });

        let device = cp
            .accept_pairing(code, addr.to_string(), "MacBook".into())
            .await
            .expect("the handshake must succeed");

        assert!(peer_side.await.unwrap().is_ok());
        assert_eq!(device.pairing_id, token.pairing_id());
        assert_eq!(device.last_addr.as_deref(), Some(addr.to_string().as_str()));
        assert!(device.last_seen_ms > 0);
        assert_eq!(cp.list_peers().len(), 1);
    }

    #[tokio::test]
    async fn a_wrong_pairing_code_is_refused_by_the_handshake() {
        // Possession of the token *is* the authentication. A code that parses
        // but is not the peer's must not produce a stored pairing.
        let (cp, _dir) = open();
        let theirs = PairingToken::generate().psk();
        let ours = PairingToken::generate().to_code();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let _ = Session::accept(stream, &theirs).await;
        });

        let err = cp
            .accept_pairing(ours, addr.to_string(), "MacBook".into())
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            CopyPasteError::PairingRefused | CopyPasteError::PeerUnreachable
        ));
        assert!(cp.list_peers().is_empty());
    }

    #[tokio::test]
    async fn checking_a_peer_we_have_never_reached_says_so_specifically() {
        // Not "unreachable": we have no address at all, which is a different
        // thing for the user to do something about.
        let (cp, _dir) = open();
        let minted = cp.create_pairing("MacBook".into()).unwrap();
        assert_eq!(
            cp.check_peer(minted.pairing_id).await.unwrap_err(),
            CopyPasteError::PeerAddressUnknown
        );
    }

    #[tokio::test]
    async fn checking_a_peer_that_is_not_paired_says_so() {
        let (cp, _dir) = open();
        assert_eq!(
            cp.check_peer("nope".into()).await.unwrap_err(),
            CopyPasteError::PeerNotFound
        );
    }

    #[tokio::test]
    async fn checking_a_reachable_peer_refreshes_its_last_seen_time() {
        let (cp, _dir) = open();
        let token = PairingToken::generate();
        let psk = token.psk();
        let code = token.to_code();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let accepting = tokio::spawn(async move {
            for _ in 0..2 {
                let (stream, _) = listener.accept().await.unwrap();
                let _ = Session::accept(stream, &psk).await;
            }
        });

        let paired = cp
            .accept_pairing(code, addr.to_string(), "MacBook".into())
            .await
            .unwrap();
        let checked = cp.check_peer(paired.pairing_id.clone()).await.unwrap();

        assert_eq!(checked.pairing_id, paired.pairing_id);
        assert!(checked.last_seen_ms >= paired.last_seen_ms);
        accepting.abort();
    }

    #[tokio::test]
    async fn sync_reports_that_it_is_unavailable_rather_than_pretending() {
        // The point of this test is that the answer is a *capability* error and
        // never a fabricated success. If someone makes this method return an
        // `Ok(SyncReport { .. })` of zeros, this fails.
        let (cp, _dir) = open();
        let minted = cp.create_pairing("MacBook".into()).unwrap();
        assert_eq!(
            cp.sync_peer(minted.pairing_id).await.unwrap_err(),
            CopyPasteError::SyncUnavailable
        );
    }

    #[tokio::test]
    async fn syncing_an_unknown_peer_says_that_first() {
        let (cp, _dir) = open();
        assert_eq!(
            cp.sync_peer("nope".into()).await.unwrap_err(),
            CopyPasteError::PeerNotFound
        );
    }

    fn hex_of(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
}

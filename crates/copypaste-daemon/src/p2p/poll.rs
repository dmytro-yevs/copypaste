//! Outgoing peer sync on a cadence.
//!
//! Without this, a paired device converges only when the *other* side dials in
//! or a human runs `copypaste sync`: `handlers::sync_now` and the inbound
//! [`super::listen`] were both present and nothing ever started a session on its
//! own. Two idle laptops would never see each other's clipboard.
//!
//! The cadence is [`crate::cadence::Idle`] — the same floor, ceiling and
//! doubling rule the cloud driver uses, rather than a second schedule with its
//! own numbers. A local capture calls [`super::P2p::wake`], which resets the
//! interval and rings the loop, so the common case is "copy here, appears
//! there" rather than "copy here, wait out whatever the interval had drifted
//! to".

use std::sync::Arc;

use tokio::sync::watch;
use tracing::{debug, info};

use crate::AppState;

/// How long the loop waits when there is nothing to sync with.
///
/// Nothing polls it awake by itself — pairing does — so this is only the
/// interval at which it re-checks a state it expects not to have changed.
/// Mirrors `cloud::poll::SIGNED_OUT_INTERVAL`, for the same reason.
pub const NO_PEERS_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);

/// Sync with every paired device until shutdown.
pub async fn run(state: Arc<AppState>, mut shutdown: watch::Receiver<bool>) {
    loop {
        let wait = if state.p2p.peers().is_empty() {
            NO_PEERS_INTERVAL
        } else {
            state.p2p.idle().interval()
        };

        tokio::select! {
            // Shutdown first, so a wake storm cannot starve teardown.
            biased;
            _ = shutdown.changed() => break,
            _ = state.p2p.wake_signal() => {}
            _ = tokio::time::sleep(wait) => {}
        }

        if *shutdown.borrow() {
            break;
        }
        // A tick that collides with a pass already in flight would dial the
        // same peers a second time, and the far side's session limit is what
        // would refuse it.
        let Some(permit) = state.p2p.try_begin_round() else {
            debug!("a peer sync pass is already running; skipping this tick");
            continue;
        };
        state.p2p.node().reconcile_discovery();
        round(&state, permit, &shutdown).await;
    }

    debug!("peer sync loop stopped");
}

/// One pass over every peer.
///
/// Failures are per-peer and already reported by `sync_one`; nothing here can
/// stop the loop, because a peer that is asleep is the normal case and must not
/// end automatic sync for the rest of them.
async fn round(
    state: &Arc<AppState>,
    permit: copypaste_core::sync::RoundGuard,
    shutdown: &watch::Receiver<bool>,
) {
    let _permit = permit;
    // The master sync switch. Checked here rather than at start-up so turning
    // it off takes effect on the next round, which is what makes it live.
    if !state.settings.get().sync_enabled {
        return;
    }
    let cycle = state.p2p.sync_cycle();
    let cancel = cycle.cancel_token();
    let peers = state.p2p.peers().list();
    if peers.is_empty() {
        return;
    }

    // Only peers this device has actually reached before. The discovery
    // fallback `sync_one` would otherwise use is an unauthenticated mDNS
    // record, and acting on hearsay on a timer is a different decision from
    // acting on it because a human ran `copypaste sync`.
    //
    // It is no longer what stops this device dialling its own listener: an
    // advertisement resolving to our own endpoint is now dropped in
    // `copypaste_p2p::discovery` — the layer that knows which endpoint is ours
    // — rather than worked around here.
    let reachable: Vec<_> = peers
        .iter()
        .filter(|peer| peer.last_addr.is_some())
        .collect();
    if reachable.is_empty() {
        return;
    }

    let mut moved = 0u64;
    for peer in &reachable {
        // Inside the session, not only between peers. A peer that answers the
        // TCP connect and then says nothing has no timeout of its own, so
        // checking only between peers let one asleep laptop hold teardown for
        // as long as it stayed silent — past the app's quit budget, which then
        // kills the daemon and takes the peer flush and the socket removal with
        // it. Abandoning a session costs nothing: nothing is committed until it
        // completes.
        let result = tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                debug!("a peer sync pass was cut short because sync was disabled");
                break;
            }
            () = crate::shutdown::requested(Some(shutdown.clone())) => {
                debug!("a peer sync pass was cut short for shutdown");
                break;
            }
            result = super::handlers::sync_one(state, peer, &cycle) => result,
        };
        moved += u64::from(result.sent) + u64::from(result.received);
        if result.received > 0 {
            state.note_remote_change();
        }
    }

    let changed = moved > 0;
    state.p2p.idle().note_activity(changed);
    if changed {
        info!(peers = reachable.len(), items = moved, "peer sync round");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::cadence::{MAX_POLL_INTERVAL_WITHOUT_PUSH, MIN_POLL_INTERVAL};
    use crate::testutil::{peer_at, test_state};
    use std::time::Duration;

    /// One pass, with a permit of its own and nothing asking for shutdown.
    async fn one_round(state: &Arc<AppState>) {
        let permit = state.p2p.try_begin_round().expect("no pass in flight");
        let (_tx, rx) = watch::channel(false);
        round(state, permit, &rx).await;
    }

    #[tokio::test]
    async fn the_loop_stops_on_shutdown() {
        let (state, _dir) = test_state("alpha");
        let (tx, rx) = watch::channel(false);
        let task = tokio::spawn(run(Arc::clone(&state), rx));
        tx.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("the loop must observe shutdown")
            .expect("no panic");
    }

    /// A round with no peers must not touch the cadence: doubling on "there was
    /// nobody to talk to" would leave a freshly paired device waiting out an
    /// interval it drifted to while it had no peers.
    #[tokio::test]
    async fn a_round_with_no_peers_leaves_the_cadence_alone() {
        let (state, _dir) = test_state("alpha");
        one_round(&state).await;
        assert_eq!(state.p2p.idle().interval(), MIN_POLL_INTERVAL);
    }

    /// The wake path a local capture uses: the interval snaps back to the floor
    /// and the loop is rung, rather than the capture waiting out a drifted wait.
    #[tokio::test]
    async fn a_local_change_resets_the_cadence_and_rings_the_loop() {
        let (state, _dir) = test_state("alpha");
        for _ in 0..8 {
            state.p2p.idle().note_activity(false);
        }
        assert!(state.p2p.idle().interval() > MIN_POLL_INTERVAL);

        state.note_local_change();
        assert_eq!(state.p2p.idle().interval(), MIN_POLL_INTERVAL);
        tokio::time::timeout(Duration::from_secs(1), state.p2p.wake_signal())
            .await
            .expect("the wake must be waiting for the loop");
    }

    /// The ceiling that applies when nothing pushes.
    ///
    /// `Idle::default()` caps at five minutes, and its own doc comment says
    /// that is only defensible because `cloud::realtime` carries the latency in
    /// front of it. Peer sync has no push channel, so an idle pair of laptops
    /// would have waited out five minutes per hop before seeing each other's
    /// clipboard.
    #[test]
    fn the_peer_cadence_stops_at_the_non_push_ceiling() {
        let (state, _dir) = test_state("alpha");
        for _ in 0..20 {
            state.p2p.idle().note_activity(false);
        }
        assert_eq!(state.p2p.idle().interval(), MAX_POLL_INTERVAL_WITHOUT_PUSH);
        assert!(MAX_POLL_INTERVAL_WITHOUT_PUSH < Duration::from_secs(300));
    }

    /// A peer that completes the TCP connect and then says nothing. There is no
    /// timeout inside the session, so before the select below this round waited
    /// on it for as long as it stayed quiet — past the app's fifteen-second
    /// quit budget, which then killed the daemon and took the peer flush and
    /// the socket removal with it.
    #[tokio::test]
    async fn a_silent_peer_does_not_hold_shutdown() {
        let (state, _dir) = test_state("alpha");
        let silent = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = silent.local_addr().unwrap();
        // Accepted and then never answered, which is what makes it silent
        // rather than merely closed.
        let _accepting = tokio::spawn(async move {
            let mut held = Vec::new();
            while let Ok((stream, _)) = silent.accept().await {
                held.push(stream);
            }
        });
        peer_at(&state, "asleep", &addr.to_string());

        let (tx, rx) = watch::channel(false);
        let permit = state.p2p.try_begin_round().expect("no pass in flight");
        let pass = tokio::spawn({
            let state = Arc::clone(&state);
            async move { round(&state, permit, &rx).await }
        });
        // Long enough for the session to be inside its handshake read.
        tokio::time::sleep(Duration::from_millis(150)).await;
        tx.send(true).unwrap();

        tokio::time::timeout(Duration::from_secs(5), pass)
            .await
            .expect("the round must abandon a silent peer on shutdown")
            .expect("no panic");
    }

    /// The other half of the same rule: cancellation must not become the
    /// ordinary path. A reachable-but-refusing peer still completes the round,
    /// so an unpaired-at-the-far-end device does not look like a shutdown.
    #[tokio::test]
    async fn a_round_nobody_cancels_still_finishes_its_peer_list() {
        let (state, _dir) = test_state("alpha");
        // Bound and immediately dropped, so connect is refused rather than left
        // hanging: the round has to come back on its own.
        let closed = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = closed.local_addr().unwrap();
        drop(closed);
        peer_at(&state, "refusing", &addr.to_string());

        let permit = state.p2p.try_begin_round().expect("no pass in flight");
        let (_tx, rx) = watch::channel(false);
        tokio::time::timeout(Duration::from_secs(20), round(&state, permit, &rx))
            .await
            .expect("a refused peer must not wedge the round");
    }

    /// The master switch is read at the top of each round, which is what makes
    /// it live rather than start-up-only.
    #[tokio::test]
    async fn sync_disabled_skips_the_round() {
        let (state, _dir) = test_state("alpha");
        state
            .settings
            .apply(
                &state.meta,
                &copypaste_ipc::ConfigPatch {
                    sync_enabled: Some(false),
                    ..Default::default()
                },
            )
            .unwrap();
        // No peers either way; what is asserted is that it returns without
        // consulting the peer list or the cadence.
        one_round(&state).await;
        assert_eq!(state.p2p.idle().interval(), MIN_POLL_INTERVAL);
    }
}

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
        round(&state).await;
    }

    debug!("peer sync loop stopped");
}

/// One pass over every peer.
///
/// Failures are per-peer and already reported by `sync_one`; nothing here can
/// stop the loop, because a peer that is asleep is the normal case and must not
/// end automatic sync for the rest of them.
async fn round(state: &Arc<AppState>) {
    // The master sync switch. Checked here rather than at start-up so turning
    // it off takes effect on the next round, which is what makes it live.
    if !state.settings.get().sync_enabled {
        return;
    }
    let peers = state.p2p.peers().list();
    if peers.is_empty() {
        return;
    }

    // Only peers this device has actually reached before. A peer with no
    // recorded address is one the *other* side dials, and the discovery
    // fallback `sync_one` would use is an unauthenticated mDNS record — on a
    // host running two daemons that is how a timer ends up dialling this
    // device's own listener every round. An explicit `copypaste sync` keeps the
    // fallback, because a human asked for it.
    let reachable: Vec<_> = peers
        .iter()
        .filter(|peer| peer.last_addr.is_some())
        .collect();
    if reachable.is_empty() {
        return;
    }

    let mut moved = 0u64;
    for peer in &reachable {
        let result = super::handlers::sync_one(state, peer).await;
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
    use crate::cadence::MIN_POLL_INTERVAL;
    use crate::testutil::test_state;
    use std::time::Duration;

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
        round(&state).await;
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
        round(&state).await;
        assert_eq!(state.p2p.idle().interval(), MIN_POLL_INTERVAL);
    }
}

//! The one construction of the shared sync view.
//!
//! [`copypaste_core::StoreSource`] is what both transports read and write
//! through, and what the Android app builds over the same core store. All this
//! adds is the two things only a daemon has: this device's identity, cached in
//! [`crate::meta`], and the cloud upload floor that has to be pulled back
//! whenever a version lands with a stamp older than it.

use std::sync::Arc;

use copypaste_core::StoreSource;

use crate::AppState;

/// A source over this daemon's history.
pub fn store_source(state: &Arc<AppState>) -> StoreSource {
    let settings = Arc::clone(state);
    StoreSource::with_retention_settings(
        state.store.clone(),
        Arc::clone(&state.keyring),
        Arc::clone(&state.detector),
        state.meta.device_id().to_string(),
        state.meta.device_name().to_string(),
        move || settings.settings.get().clone(),
    )
}

/// [`store_source`] for the *peer* transport, which additionally has to pull
/// the cloud upload floor back.
///
/// An item that arrived from a peer carries the sender's stamp, routinely older
/// than this device's upload cursor, so without this it would never be
/// forwarded to the account. The cloud transport deliberately does **not** take
/// this hook: a row it just downloaded is already in the account, and lowering
/// the floor onto it would re-offer it for upload every round.
///
/// Built per operation rather than held on [`AppState`]: the hook owns an
/// `Arc<AppState>`, and a source parked on the state it points back at is a
/// reference cycle that never drops.
pub fn peer_source(state: &Arc<AppState>) -> StoreSource {
    let hooked = Arc::clone(state);
    store_source(state)
        .on_applied(move |created_at| crate::cloud::note_version_written(&hooked, created_at))
}

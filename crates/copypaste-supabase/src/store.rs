use crate::models::Session;

/// Pluggable persistence backend for auth sessions.
///
/// The default implementation ([`InMemoryStore`]) stores the session only in
/// RAM.  A production deployment can swap this for a keychain-backed store by
/// implementing this trait.
pub trait SessionStore: Send + Sync {
    /// Persist (or overwrite) the current session.
    fn save(&self, session: &Session);

    /// Return the most recently persisted session, if any.
    fn load(&self) -> Option<Session>;

    /// Remove any persisted session.
    fn clear(&self);
}

// ---------------------------------------------------------------------------
// In-memory store (default)
// ---------------------------------------------------------------------------

use std::sync::{Arc, Mutex};

/// A simple in-process session store backed by a `Mutex<Option<Session>>`.
///
/// Sessions are lost when the process exits.  Use this in tests and as a
/// default when no durable store is needed.
#[derive(Debug, Default, Clone)]
pub struct InMemoryStore {
    inner: Arc<Mutex<Option<Session>>>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SessionStore for InMemoryStore {
    fn save(&self, session: &Session) {
        // A poisoned lock only means a previous holder panicked; the session
        // slot itself is a plain `Option` with no broken invariant, so recover
        // the guard rather than propagating the panic (this is a best-effort
        // RAM cache — losing it is never worth crashing the daemon).
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        *guard = Some(session.clone());
    }

    fn load(&self) -> Option<Session> {
        // See `save`: recover from poisoning instead of panicking.
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn clear(&self) {
        // See `save`: recover from poisoning instead of panicking.
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        *guard = None;
    }
}

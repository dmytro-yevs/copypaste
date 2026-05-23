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
        let mut guard = self.inner.lock().expect("mutex poisoned");
        *guard = Some(session.clone());
    }

    fn load(&self) -> Option<Session> {
        self.inner.lock().expect("mutex poisoned").clone()
    }

    fn clear(&self) {
        let mut guard = self.inner.lock().expect("mutex poisoned");
        *guard = None;
    }
}

//! Tests for `InMemoryStore`, the `SessionStore` trait contract, and the
//! `Session` security invariants (debug redaction + token expiry helper).
//!
//! Coverage gap closed by CopyPaste-bn6: the supabase crate had zero tests
//! for the session-store abstraction (save/load/clear lifecycle) and for the
//! security-sensitive `Debug` impl that must not emit bearer tokens.

use copypaste_supabase::{InMemoryStore, Session, SessionStore, User};

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn make_session(access_token: &str, refresh_token: &str, expires_at: u64) -> Session {
    Session {
        access_token: access_token.to_owned(),
        refresh_token: refresh_token.to_owned(),
        expires_in: 3600,
        expires_at,
        token_type: "bearer".to_owned(),
        user: User {
            id: "user-1".to_owned(),
            email: Some("user@example.com".to_owned()),
            role: Some("authenticated".to_owned()),
            created_at: None,
            updated_at: None,
        },
    }
}

// ---------------------------------------------------------------------------
// InMemoryStore — SessionStore contract
// ---------------------------------------------------------------------------

#[test]
fn in_memory_store_starts_empty() {
    let store = InMemoryStore::new();
    assert!(store.load().is_none(), "fresh store must return None");
}

#[test]
fn save_then_load_returns_same_session() {
    let store = InMemoryStore::new();
    let session = make_session("tok-A", "rt-A", 9_999_999_999);

    store.save(&session);
    let loaded = store.load().expect("session must be present after save");

    assert_eq!(loaded.access_token, session.access_token);
    assert_eq!(loaded.refresh_token, session.refresh_token);
    assert_eq!(loaded.expires_at, session.expires_at);
    assert_eq!(loaded.user.id, session.user.id);
}

#[test]
fn second_save_overwrites_previous_session() {
    let store = InMemoryStore::new();
    store.save(&make_session("tok-OLD", "rt-OLD", 1_000));
    store.save(&make_session("tok-NEW", "rt-NEW", 2_000));

    let loaded = store.load().expect("session must be present");
    assert_eq!(
        loaded.access_token, "tok-NEW",
        "second save must overwrite first"
    );
    assert_eq!(loaded.expires_at, 2_000);
}

#[test]
fn clear_removes_session() {
    let store = InMemoryStore::new();
    store.save(&make_session("tok-X", "rt-X", 5_000));
    assert!(
        store.load().is_some(),
        "session must be present before clear"
    );

    store.clear();
    assert!(store.load().is_none(), "session must be absent after clear");
}

#[test]
fn clear_on_empty_store_is_idempotent() {
    let store = InMemoryStore::new();
    store.clear(); // must not panic
    assert!(store.load().is_none());
}

#[test]
fn store_is_clone_and_shares_state() {
    // Cloning the store (via Arc<Mutex<_>> inside) yields a view of the same
    // underlying slot — a write on one clone is visible on the other.
    let store = InMemoryStore::new();
    let clone = store.clone();

    store.save(&make_session("tok-shared", "rt-shared", 7_000));
    let loaded = clone.load().expect("clone must see the saved session");
    assert_eq!(loaded.access_token, "tok-shared");
}

// ---------------------------------------------------------------------------
// Session — security invariants
// ---------------------------------------------------------------------------

/// The `Debug` impl on `Session` MUST NOT include the bearer token strings.
///
/// A derived `Debug` would print them verbatim; the manual impl replaces them
/// with `"<redacted>"`. This test pins that contract so a future refactor
/// (e.g. adding `#[derive(Debug)]`) fails loudly.
#[test]
fn session_debug_redacts_access_and_refresh_tokens() {
    let session = make_session("super-secret-access-token", "super-secret-refresh-token", 0);
    let debug_str = format!("{session:?}");

    assert!(
        !debug_str.contains("super-secret-access-token"),
        "access_token must not appear in Debug output; got: {debug_str}"
    );
    assert!(
        !debug_str.contains("super-secret-refresh-token"),
        "refresh_token must not appear in Debug output; got: {debug_str}"
    );
    // The placeholder strings must be present so the output is still useful
    // for debugging (rather than the fields being silently omitted).
    assert!(
        debug_str.contains("<redacted>"),
        "Debug output must contain '<redacted>' placeholder; got: {debug_str}"
    );
}

/// `is_expired_with_margin(0)` on a past session returns `true`.
#[test]
fn session_expiry_past_returns_true() {
    let session = make_session("tok", "rt", 0); // expired at epoch
    assert!(
        session.is_expired_with_margin(0),
        "session at epoch must be expired"
    );
}

/// `is_expired_with_margin(0)` on a far-future session returns `false`.
#[test]
fn session_expiry_future_returns_false() {
    // Year 2100 in Unix seconds.
    let session = make_session("tok", "rt", 4_102_444_800);
    assert!(
        !session.is_expired_with_margin(0),
        "session expiring in 2100 must not be expired"
    );
}

/// `is_expired_with_margin` with a large margin on a future session returns
/// `true` (margin pushes the effective "now" past the expiry).
#[test]
fn session_expiry_large_margin_triggers_early_refresh() {
    // Expires in 30 seconds from now.
    let expires_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        + 30;
    let session = make_session("tok", "rt", expires_at);

    // With a 60-second margin the 30-second-future token is considered expired
    // (clients refresh before the token actually expires).
    assert!(
        session.is_expired_with_margin(60),
        "token expiring in 30s with a 60s margin should be considered expired"
    );
    // But with a 10-second margin it is still valid.
    assert!(
        !session.is_expired_with_margin(10),
        "token expiring in 30s with a 10s margin should still be valid"
    );
}

// ---------------------------------------------------------------------------
// InMemoryStore — thread-safety smoke test
// ---------------------------------------------------------------------------

/// Multiple threads writing and reading the same store must not corrupt state.
///
/// This is a smoke test, not a liveness proof — it verifies that the
/// `Arc<Mutex<_>>` interior does not panic under concurrent access.
#[test]
fn store_is_thread_safe_under_concurrent_access() {
    use std::sync::Arc;
    use std::thread;

    let store = Arc::new(InMemoryStore::new());
    let mut handles = Vec::with_capacity(8);

    for i in 0..8usize {
        let s = Arc::clone(&store);
        handles.push(thread::spawn(move || {
            let session = make_session(&format!("tok-{i}"), &format!("rt-{i}"), i as u64 * 1_000);
            s.save(&session);
            let _ = s.load();
        }));
    }

    for h in handles {
        h.join().expect("thread must not panic");
    }

    // After all threads have finished, some session must be present (the last
    // writer wins). The exact value is non-deterministic; we just assert the
    // store is in a valid state.
    assert!(
        store.load().is_some(),
        "store must contain a session after all writes"
    );
}

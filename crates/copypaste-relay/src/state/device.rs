//! Device registry types: [`DeviceRecord`], [`TokenEntry`], and token-auth impl.
//!
//! A `device_id` maps to exactly one [`DeviceRecord`] which holds a *set* of
//! [`TokenEntry`] values for shared-account co-registration (R1a).  Auth
//! (`verify_token`, `verify_token_at`) and activity tracking (`update_last_seen`,
//! `get_device`) are implemented on [`super::RelayStore`] here.

use std::net::IpAddr;
use std::time::Instant;

use subtle::ConstantTimeEq;

use crate::error::RelayError;
use crate::quota::Tier;

/// Maximum number of independent auth tokens a single `device_id` may hold at
/// once (R1a co-registration). A shared-account inbox is co-registered by every
/// device on the account — each co-registration mints a fresh, independent token
/// bound to the same `device_id` — so a record now maps to a *set* of valid
/// tokens, not one. This cap bounds per-device memory: an account churning
/// re-registrations (key rotation, reinstalls) can't grow the token set without
/// bound. When a new token would exceed the cap, expired tokens are pruned
/// first and, if still over, the oldest-issued token is evicted (FIFO).
pub(super) const MAX_TOKENS_PER_DEVICE: usize = 16;

/// One independently-issued bearer credential for a device record.
///
/// A `device_id` maps to a *set* of these (R1a co-registration): every device
/// on a shared account co-registers the same account-inbox `device_id` and is
/// minted its own `TokenEntry`. All non-expired entries authorize the device_id
/// equally, so any co-registered device can push to and read/subscribe the one
/// shared inbox.
#[derive(Debug, Clone)]
pub struct TokenEntry {
    /// Bearer token: 64 hex characters representing 32 random bytes (256-bit)
    /// from OsRng (CopyPaste-qvtg.3).
    /// Generated at registration time and stored verbatim — never recomputed
    /// from the public key (which would make it a deterministic oracle).
    pub token: String,
    /// Unix timestamp (seconds since epoch) when this token expires (1 year
    /// after the issuing registration).
    pub expires_at_unix: i64,
}

#[derive(Debug)]
pub struct DeviceRecord {
    pub device_id: String,
    pub device_name: String,
    // Read by `routes/devices.rs` (`GET /devices/:id` response body), but
    // `#[path]`-include test binaries that include state.rs without the routes
    // see this field as unreachable — allow suppresses the spurious warning in
    // those non-default-allow test crates (e.g. sse_subscribe.rs).
    #[allow(dead_code)]
    pub public_key_b64: String,
    /// The proof-of-possession (PoP) value stored at first registration.
    /// Subsequent co-registrations are verified against this using constant-time
    /// comparison — they must present the same PoP, proving all co-registrants
    /// hold the same sync key (same account). Stored as 32 raw bytes (before
    /// base64 decoding) so the comparison is length-constant.
    pub registered_pop: [u8; 32],
    /// The set of currently-issued bearer tokens for this device_id, in
    /// issuance order (oldest first). A device record maps to *many* tokens to
    /// support shared-account co-registration (R1a): see [`TokenEntry`] and
    /// [`DeviceRecord::add_token`]. `verify_token` accepts ANY non-expired entry.
    pub tokens: Vec<TokenEntry>,
    pub registered_at: Instant,
    /// Last time this device was seen making an authenticated request (push or
    /// pull). Updated by `update_last_seen`. Used by `cleanup_inactive_devices`
    /// instead of `registered_at` so that an active device that has drained its
    /// inbox is not evicted simply because it registered long ago.
    pub last_seen: Instant,
    /// Subscription tier — determines device count and history quotas.
    // Read by `push_item_decoded` via `effective_history_cap(record.tier)`, but
    // `#[path]`-include test binaries that compile state.rs without the routes
    // may not exercise that path and see the field as dead.  `Tier::Pro` is now
    // feature-gated, so production always stores/reads `Tier::Free`; keep the
    // allow to silence the lint in those test compilations.
    #[allow(dead_code)]
    pub tier: Tier,
    /// Source IP the device registered from, used as the *scope* for the
    /// per-scope device-count quota (H1). `None` when the relay is exercised
    /// without a real transport (unit/integration tests); all such devices
    /// share the single `None` scope, matching pre-IP behaviour.
    pub registered_from_ip: Option<IpAddr>,
}

impl DeviceRecord {
    /// Append a freshly-issued token, enforcing the per-device token cap.
    ///
    /// Pruning order when at/over `MAX_TOKENS_PER_DEVICE`:
    ///   1. drop every token already expired at `now_unix`;
    ///   2. if still at the cap, evict the oldest-issued token (front of the Vec) until there is room for the new one.
    ///
    /// The new token is then pushed at the back so the Vec stays in issuance
    /// order (oldest first), which is what the FIFO eviction relies on.
    pub(super) fn add_token(&mut self, token: String, expires_at_unix: i64, now_unix: i64) {
        // 1. Reclaim space from already-expired tokens first.
        self.tokens.retain(|t| now_unix <= t.expires_at_unix);
        // 2. Still over the cap (all live)? Evict oldest until there is room
        //    for one more. `> ` (not `>=`) because we want the post-push len
        //    to be `<= MAX_TOKENS_PER_DEVICE`.
        while self.tokens.len() + 1 > MAX_TOKENS_PER_DEVICE {
            self.tokens.remove(0);
        }
        self.tokens.push(TokenEntry {
            token,
            expires_at_unix,
        });
    }

    /// The latest expiry across all currently-held tokens, or 0 if none.
    /// Surfaced via `GET /devices/:id` as the device's `expires_at`.
    pub fn latest_expires_at(&self) -> i64 {
        self.tokens
            .iter()
            .map(|t| t.expires_at_unix)
            .max()
            .unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// RelayStore: auth + activity tracking
// ---------------------------------------------------------------------------

impl super::RelayStore {
    // -----------------------------------------------------------------------
    // Auth
    // -----------------------------------------------------------------------

    /// Verify that `token` matches the bearer token for `device_id`.
    /// Uses constant-time comparison to prevent timing-based token oracle attacks.
    ///
    /// M11 (audit 2026-05-27): also enforces `expires_at_unix`. An expired
    /// token returns `Unauthorized` (NOT a distinct error) so an attacker
    /// cannot distinguish "wrong token" from "expired token". The equality
    /// check still runs before the expiry branch so the constant-time
    /// comparison path is unconditional.
    ///
    /// Clock errors fail CLOSED: if `SystemTime::now()` cannot produce a
    /// valid duration (e.g. the system clock is set before UNIX_EPOCH), we
    /// treat the token as expired and return `Unauthorized`. The previous
    /// `unwrap_or_default()` yielded `now_unix = 0`, making
    /// `0 <= expires_at_unix` always true and tokens never expire on a
    /// broken clock — a fail-open security hole.
    pub fn verify_token(&self, device_id: &str, token: &str) -> Result<(), RelayError> {
        let now_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_secs() as i64);
        self.verify_token_at(device_id, token, now_unix)
    }

    /// Internal helper: verify token with an explicit `now_unix` timestamp.
    ///
    /// `now_unix = None` represents a clock error and fails CLOSED
    /// (returns `Unauthorized`). Extracted so tests can inject a
    /// simulated clock fault without touching the OS clock.
    ///
    /// Collapse missing-device to `Unauthorized` (not `DeviceNotFound`) on
    /// token-guarded routes: returning a distinct 404 would let an attacker
    /// enumerate which device IDs are registered by probing push/pull/delete
    /// with a garbage token. `Unauthorized` is indistinguishable from a wrong
    /// token, closing that oracle while preserving GET /devices/:id 404 for the
    /// unauthenticated device-info endpoint.
    pub fn verify_token_at(
        &self,
        device_id: &str,
        token: &str,
        now_unix: Option<i64>,
    ) -> Result<(), RelayError> {
        let record = match self.devices.get(device_id) {
            Some(record) => record,
            None => {
                // Timing-equalization (CopyPaste-qk0y): a registered device runs
                // the ct_eq token loop below, so returning immediately here would
                // let a network attacker distinguish "device registered" vs not by
                // wall-clock, despite the identical `Unauthorized` response. Run one
                // constant-time compare against a fixed sentinel so the absent-device
                // path performs comparable work. Result is discarded.
                let sentinel = [0u8; 32];
                let _dummy = sentinel.ct_eq(token.as_bytes());
                return Err(RelayError::Unauthorized);
            }
        };
        // A device_id maps to a SET of co-registered tokens (R1a): the request
        // authorizes if ANY currently-valid token matches. We evaluate every
        // entry (no early break) so the comparison work does not vary with how
        // many tokens precede the match — the per-entry compare is already
        // constant-time via `ct_eq`. A token authorizes iff it both matches and
        // is not expired; the equality check runs unconditionally so the
        // constant-time path is taken regardless of expiry.
        let mut authorized = subtle::Choice::from(0u8);
        for entry in &record.tokens {
            let matches = entry.token.as_bytes().ct_eq(token.as_bytes());
            // Fail closed on clock error: None = unknown time = treat as expired.
            let not_expired = match now_unix {
                Some(now) => now <= entry.expires_at_unix,
                None => false,
            };
            authorized |= matches & subtle::Choice::from(not_expired as u8);
        }
        if authorized.into() {
            Ok(())
        } else {
            Err(RelayError::Unauthorized)
        }
    }

    // -----------------------------------------------------------------------
    // Device info
    // -----------------------------------------------------------------------

    /// Return public info about a registered device. Bearer tokens are never included.
    pub fn get_device(&self, device_id: &str) -> Result<&DeviceRecord, RelayError> {
        self.devices
            .get(device_id)
            .ok_or(RelayError::DeviceNotFound)
    }

    // -----------------------------------------------------------------------
    // Activity tracking
    // -----------------------------------------------------------------------

    /// Stamp `last_seen` to `Instant::now()` for `device_id`.
    ///
    /// Call this after every successful `verify_token` so that `cleanup_inactive_devices`
    /// evicts on actual inactivity, not on registration age. A device that registers,
    /// drains its inbox, and then stays idle for the threshold will be evicted — but
    /// one that continues to pull (even an empty inbox) will not.
    // Called from routes/items.rs after every successful verify_token in the
    // push, pull, and delete_item handlers.
    pub fn update_last_seen(&mut self, device_id: &str) {
        if self.devices.get_mut(device_id).is_some() {
            // Borrow ends before the DB call below (NLL): set the in-memory
            // Instant first, then persist the wall-clock equivalent.
            if let Some(record) = self.devices.get_mut(device_id) {
                record.last_seen = Instant::now();
            }
            // R1b write-through: persist last_seen as Unix seconds so restart
            // preserves inactivity age. A persistence failure here must NOT
            // abort the request (the in-memory state is authoritative for the
            // live process and the signature is infallible) — log and continue.
            // Worst case a crash before the next successful write loses a few
            // seconds of last_seen freshness, which only affects 30-day idle
            // eviction timing, never correctness.
            if let Ok(d) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                let now_unix = d.as_secs() as i64;
                if let Err(e) = self.db.update_last_seen(device_id, now_unix) {
                    tracing::warn!(device_id, error = %e, "relay: failed to persist last_seen");
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use crate::error::RelayError;
    use crate::quota::Tier;
    use crate::state::test_helpers::*;

    use super::MAX_TOKENS_PER_DEVICE;

    #[test]
    fn register_returns_bearer_token() {
        let mut store = make_store();
        let (token, expires_at) = store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        // CopyPaste-qvtg.3: 32-byte (256-bit) token → 64 hex chars.
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(expires_at > 0);
    }

    #[test]
    fn issued_token_is_not_born_expired() {
        // Guards the near-epoch-clock outage fix: under any correctly-set host
        // clock the issued token must expire in the future (~1 year out), never
        // at-or-before "now". A bogus near-epoch clock previously yielded
        // `expires_at_unix ≈ 365d`, which is far in the past today, so every
        // device would be Unauthorized on its next request.
        let mut store = make_store();
        let (token, expires_at) = store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        let now_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("test host clock must be past the epoch")
            .as_secs() as i64;
        assert!(
            expires_at > now_unix,
            "token must not be born expired: expires_at={expires_at}, now={now_unix}"
        );
        // Roughly one year out (allow a few seconds of test scheduling slack).
        let one_year = 365 * 24 * 3600;
        assert!((expires_at - now_unix - one_year).abs() < 60);
        // And the freshly-issued token must actually verify.
        assert!(store.verify_token(&device_a_id(), &token).is_ok());
    }

    #[test]
    fn co_registration_mints_new_token_and_both_are_valid() {
        // R1a: re-registering an already-registered device_id no longer
        // conflicts — it co-registers, minting a fresh independent token while
        // keeping the original valid. BOTH tokens must authorize.
        let mut store = make_store();
        let (token1, _) = store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        let (token2, _) = store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        assert_ne!(token1, token2, "co-registration must mint a distinct token");
        assert!(
            store.verify_token(&device_a_id(), &token1).is_ok(),
            "the original token must remain valid after co-registration"
        );
        assert!(
            store.verify_token(&device_a_id(), &token2).is_ok(),
            "the co-registered token must also be valid"
        );
        // Still one device record / one inbox — co-registration shares the inbox.
        assert_eq!(store.devices.len(), 1);
        assert_eq!(store.devices[&device_a_id()].tokens.len(), 2);
    }

    /// CopyPaste-crh3.115: device_tier reports the registered tier (the push
    /// quota now reads this instead of a hardcoded Tier::Free), and falls back to
    /// Free for an unknown device.
    #[test]
    fn device_tier_reports_registered_tier_and_free_fallback() {
        let mut store = make_store();
        store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        assert_eq!(store.device_tier(&device_a_id()), Tier::Free);
        assert_eq!(
            store.device_tier("never-registered"),
            Tier::Free,
            "unknown device must default to Free (never over-grant)"
        );
    }

    /// CopyPaste-crh3.115: a Pro-tier registration is reflected by device_tier, so
    /// the push item-size quota admits an above-free item for that sender.
    #[cfg(feature = "quota-tiers")]
    #[test]
    fn device_tier_reports_pro_for_pro_registration() {
        let mut store = make_store();
        store
            .register_device_with_tier(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
                Tier::Pro,
            )
            .unwrap();
        assert_eq!(store.device_tier(&device_a_id()), Tier::Pro);
    }

    #[test]
    fn token_cap_evicts_oldest() {
        // Issuing more than MAX_TOKENS_PER_DEVICE tokens for one device_id
        // evicts the oldest (FIFO): the first-issued token stops authorizing
        // once the cap is exceeded, while the most recent cap-worth stay valid.
        let mut store = make_store();
        let mut tokens = Vec::new();
        for _ in 0..(MAX_TOKENS_PER_DEVICE + 1) {
            let (t, _) = store
                .register_device(
                    device_a_id(),
                    "Device A".into(),
                    valid_key_b64(),
                    valid_pop_b64(),
                )
                .unwrap();
            tokens.push(t);
        }
        assert_eq!(
            store.devices[&device_a_id()].tokens.len(),
            MAX_TOKENS_PER_DEVICE,
            "token set must be capped at MAX_TOKENS_PER_DEVICE"
        );
        // The very first token was evicted (oldest), the rest still authorize.
        assert!(
            store.verify_token(&device_a_id(), &tokens[0]).is_err(),
            "oldest token must be evicted past the cap"
        );
        for t in &tokens[1..] {
            assert!(
                store.verify_token(&device_a_id(), t).is_ok(),
                "the most recent cap-worth of tokens must remain valid"
            );
        }
    }

    #[test]
    fn expired_token_is_pruned_on_co_registration() {
        // add_token reclaims space from already-expired tokens before applying
        // the oldest-eviction cap: an expired token is dropped on the next
        // co-registration rather than counting toward the cap.
        let mut store = make_store();
        store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        // Forcibly expire the sole token.
        store.devices.get_mut(&device_a_id()).unwrap().tokens[0].expires_at_unix = 1;
        // Co-register: the expired entry is pruned, leaving only the new token.
        let (fresh, _) = store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        let tokens = &store.devices[&device_a_id()].tokens;
        assert_eq!(tokens.len(), 1, "expired token must be pruned on add");
        assert_eq!(tokens[0].token, fresh);
        assert!(store.verify_token(&device_a_id(), &fresh).is_ok());
    }

    #[test]
    fn verify_token_ok() {
        let mut store = make_store();
        let (token, _) = store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        assert!(store.verify_token(&device_a_id(), &token).is_ok());
    }

    #[test]
    fn verify_token_wrong_token_is_unauthorized() {
        let mut store = make_store();
        store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        let err = store
            .verify_token(&device_a_id(), "badtoken00000000000000000000000")
            .unwrap_err();
        assert!(matches!(err, RelayError::Unauthorized));
    }

    #[test]
    fn verify_token_expired_is_unauthorized() {
        // M11: an expired token must return Unauthorized (same variant as
        // a wrong token) so an attacker cannot distinguish the two cases.
        let mut store = make_store();
        store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        // Forcibly expire the device's sole token by rewinding `expires_at_unix`.
        let record = store.devices.get_mut(&device_a_id()).unwrap();
        let token = record.tokens[0].token.clone();
        record.tokens[0].expires_at_unix = 1; // 1970-01-01
        let err = store.verify_token(&device_a_id(), &token).unwrap_err();
        assert!(matches!(err, RelayError::Unauthorized));
    }

    #[test]
    fn get_device_returns_correct_info() {
        let mut store = make_store();
        store
            .register_device(
                device_a_id(),
                "My Mac".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        let record = store.get_device(&device_a_id()).unwrap();
        assert_eq!(record.device_id, device_a_id());
        assert_eq!(record.device_name, "My Mac");
        assert_eq!(record.public_key_b64, valid_key_b64());
        assert!(record.latest_expires_at() > 0);
    }

    #[test]
    fn get_device_missing_returns_not_found() {
        let store = make_store();
        let err = store.get_device("nonexistent-id").unwrap_err();
        assert!(matches!(err, RelayError::DeviceNotFound));
    }

    /// Positive: a device that calls `update_last_seen` after the inactivity
    /// threshold has elapsed must SURVIVE `cleanup_inactive_devices` — last_seen
    /// is reset to "now" so the threshold is no longer exceeded.
    ///
    /// This guards the fix for the prior bug where `update_last_seen` was
    /// defined but never called from the route handlers, so `last_seen` stayed
    /// equal to `registered_at` and active devices were evicted after 30 days.
    #[test]
    fn update_last_seen_prevents_eviction_after_threshold() {
        let mut store = make_store();
        store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();

        // Simulate the device's last_seen being past the threshold by rewinding
        // it to a time far in the past (subtract threshold + 1 second).
        let threshold_secs = 60u64; // use a small finite threshold for the test
        {
            let record = store.devices.get_mut(&device_a_id()).unwrap();
            // Rewind last_seen by (threshold + 1) seconds so cleanup would
            // normally evict this device.
            record.last_seen = Instant::now() - Duration::from_secs(threshold_secs + 1);
        }

        // Verify that WITHOUT update_last_seen the device would be evicted.
        let would_evict = store
            .devices
            .get(&device_a_id())
            .map(|r| r.last_seen.elapsed().as_secs() >= threshold_secs)
            .unwrap_or(false);
        assert!(
            would_evict,
            "precondition: device should be evictable before update_last_seen"
        );

        // Now call update_last_seen (simulating what the route handler does
        // after a successful verify_token).
        store.update_last_seen(&device_a_id());

        // cleanup_inactive_devices with the same finite threshold must NOT
        // evict the device whose last_seen was just refreshed.
        let removed = store.cleanup_inactive_devices(threshold_secs);
        assert_eq!(
            removed, 0,
            "device must survive cleanup after update_last_seen refreshes last_seen"
        );
        assert!(
            store.devices.contains_key(&device_a_id()),
            "device must still be registered"
        );
    }

    /// Negative: a device that does NOT call `update_last_seen` after the
    /// inactivity threshold has elapsed must be EVICTED by
    /// `cleanup_inactive_devices`.
    #[test]
    fn no_update_last_seen_causes_eviction_after_threshold() {
        let mut store = make_store();
        store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();

        let threshold_secs = 60u64;

        // Rewind last_seen past the threshold — no update_last_seen called.
        {
            let record = store.devices.get_mut(&device_a_id()).unwrap();
            record.last_seen = Instant::now() - Duration::from_secs(threshold_secs + 1);
        }

        let removed = store.cleanup_inactive_devices(threshold_secs);
        assert_eq!(
            removed, 1,
            "stale device with no last_seen update must be evicted"
        );
        assert!(
            !store.devices.contains_key(&device_a_id()),
            "device must be gone after eviction"
        );
    }

    /// When `verify_token` encounters a clock error it must fail CLOSED (return
    /// `Unauthorized`), not silently treat `now_unix=0` as "valid" (the old
    /// `unwrap_or_default()` behaviour that made `0 <= expires_at_unix` always
    /// true). We test the internal helper `verify_token_at` directly, injecting
    /// a simulated clock error via `None` for `now_unix`.
    #[test]
    fn verify_token_clock_error_returns_unauthorized() {
        let mut store = make_store();
        let (token, _) = store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        // None = simulated clock error → must fail closed.
        let err = store
            .verify_token_at(&device_a_id(), &token, None)
            .unwrap_err();
        assert!(
            matches!(err, RelayError::Unauthorized),
            "clock error must fail closed: got {err:?}"
        );
    }
}

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
        let record = self
            .devices
            .get(device_id)
            .ok_or(RelayError::Unauthorized)?;
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

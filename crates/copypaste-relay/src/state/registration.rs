//! Device registration: rate-limiting and the `register_device_*` family.
//!
//! All registration paths funnel through [`super::RelayStore::register_device_with_tier_scoped`],
//! which enforces the PoP proof-of-possession check, per-scope device quota, and
//! co-registration (R1a) semantics before issuing a bearer token.

use std::net::IpAddr;
use std::time::Instant;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use rand::rngs::OsRng;
use rand::RngCore;

use crate::error::RelayError;
use crate::quota::{self, QuotaViolation, Tier};

use super::device::{DeviceRecord, TokenEntry};
use super::{REG_LIMIT_MAX_ATTEMPTS, REG_LIMIT_MAX_KEYS, REG_LIMIT_WINDOW};

impl super::RelayStore {
    // -----------------------------------------------------------------------
    // Per-device registration rate limiter (security MEDIUM #13)
    // -----------------------------------------------------------------------

    /// Record a registration attempt for `(client_ip, device_id)` and return
    /// `Err(retry_after_secs)` when the per-(ip, device) rate-limit window
    /// is exhausted (`REG_LIMIT_MAX_ATTEMPTS` attempts within
    /// `REG_LIMIT_WINDOW`).
    ///
    /// This is independent of the per-IP `tower_governor` limiter installed
    /// in `routes/mod.rs`: it blocks an attacker who has obtained a victim's
    /// `device_id` (but not the bearer token) from flooding re-registrations
    /// of that specific id from a single source IP. Keying by the tuple
    /// (HIGH #5) avoids leaking "this device id is known to the limiter"
    /// across source IPs.
    ///
    /// Callers should invoke this **only after** the request payload has
    /// passed full validation (UUID parse, base64 key, key length, device
    /// name) so the limiter never grows from probes that the handler would
    /// have rejected anyway.
    pub fn check_registration_rate_limit(
        &mut self,
        client_ip: Option<IpAddr>,
        device_id: &str,
    ) -> Result<(), u64> {
        let now = Instant::now();

        // Opportunistic global eviction when the map grows too large.
        if self.reg_attempts.len() > REG_LIMIT_MAX_KEYS {
            self.reg_attempts.retain(|_, deque| {
                deque.retain(|t| now.duration_since(*t) < REG_LIMIT_WINDOW);
                !deque.is_empty()
            });
        }

        let deque = self
            .reg_attempts
            .entry((client_ip, device_id.to_string()))
            .or_default();
        // Drop attempts that fell out of the rolling window.
        while let Some(front) = deque.front() {
            if now.duration_since(*front) >= REG_LIMIT_WINDOW {
                deque.pop_front();
            } else {
                break;
            }
        }

        if deque.len() >= REG_LIMIT_MAX_ATTEMPTS {
            let oldest = *deque.front().expect("non-empty by check above");
            let retry_after = REG_LIMIT_WINDOW
                .saturating_sub(now.duration_since(oldest))
                .as_secs()
                .max(1);
            return Err(retry_after);
        }

        deque.push_back(now);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Registration
    // -----------------------------------------------------------------------

    /// Register a new device with an explicit tier (no source IP — the device
    /// is placed in the shared `None` scope). Convenience wrapper over
    /// [`Self::register_device_with_tier_scoped`].
    ///
    /// Returns `(bearer_token, expires_at_unix)` on success. A duplicate
    /// `device_id` co-registers (mints an additional token) rather than
    /// conflicting — see [`Self::register_device_with_tier_scoped`].
    /// Returns `RelayError::DeviceQuotaExceeded` if registering a NEW device
    /// would exceed the device count limit for `tier` *within the device's scope*.
    // Production registration goes through `register_device_scoped` (which
    // supplies the client IP). This unscoped wrapper is used only by tests that
    // exercise tier-aware quotas without a transport. When `quota-tiers` is
    // enabled (e.g. --all-features) it is included but has no non-test caller
    // — allow suppresses dead_code.
    #[cfg(any(test, feature = "quota-tiers"))]
    #[allow(dead_code)] // intentional: test helper, no production caller
    pub fn register_device_with_tier(
        &mut self,
        device_id: String,
        device_name: String,
        public_key_b64: String,
        pop_b64: String,
        tier: Tier,
    ) -> Result<(String, i64), RelayError> {
        self.register_device_with_tier_scoped(
            None,
            device_id,
            device_name,
            public_key_b64,
            pop_b64,
            tier,
        )
    }

    /// Register a device with an explicit tier, scoped to `client_ip`, issuing
    /// a fresh independent bearer token.
    ///
    /// **Shared-account co-registration (R1a).** Unlike the original behaviour
    /// (409 on a duplicate `device_id`), an *already-registered* `device_id` is
    /// accepted: a new independent token is minted, appended to that device's
    /// token set (capped at [`super::device::MAX_TOKENS_PER_DEVICE`]), and returned. This is
    /// the mechanism that makes cross-device delivery possible: clients derive
    /// ONE account-inbox `device_id` (a UUID via HKDF of the shared sync key)
    /// and every device on the account co-registers THAT id, each getting its
    /// own token. All of those tokens then authorize push to / read of / SSE
    /// subscribe to the single shared inbox — so an item pushed by one device
    /// is delivered to every other device on the account. Echo/dupes are
    /// prevented client-side (LWW + item_id dedup).
    ///
    /// SECURITY: the account-inbox `device_id` is a SECRET derived from the
    /// sync key and is never transmitted in cleartext, so co-registration is
    /// effectively gated by knowledge of that secret id — only a device that
    /// already holds the account's sync key can derive the id and co-register.
    /// The relay stores only opaque ciphertext (`content_b64`); a wrong id
    /// simply addresses an inbox that doesn't exist (or isn't the account's)
    /// and yields nothing useful. Tokens remain random (never derived from the
    /// public key), so holding the id alone does not let an attacker forge a
    /// token for an existing inbox without going through registration.
    ///
    /// The per-scope device-count quota (H1) and the conservative public-key
    /// proof-of-possession check apply to a genuinely **new** device record;
    /// co-registration of an existing id reuses the existing record (same
    /// inbox, same name/key) and is therefore NOT re-charged against the device
    /// quota — it only mints an additional token. The per-(ip, device)
    /// registration rate limit in `routes/devices.rs` still bounds co-register
    /// floods from a single source, and because it is keyed by `(client_ip,
    /// device_id)` a legitimate co-registration from a *different* device/IP
    /// for the same account id is not blocked by another device's bucket.
    ///
    /// Returns `(bearer_token, expires_at_unix)` — the freshly-minted token and
    /// its expiry — on success.
    /// Returns `RelayError::DeviceQuotaExceeded` if registering a NEW device
    /// would exceed the device-count limit for `tier` within `client_ip`'s
    /// scope. (No `DeviceConflict` is returned anymore — duplicates co-register.)
    pub fn register_device_with_tier_scoped(
        &mut self,
        client_ip: Option<IpAddr>,
        device_id: String,
        device_name: String,
        public_key_b64: String,
        pop_b64: String,
        tier: Tier,
    ) -> Result<(String, i64), RelayError> {
        let is_co_registration = self.devices.contains_key(&device_id);

        // Validate public_key_b64: non-empty, valid base64, decodes to 32 bytes.
        if public_key_b64.is_empty() {
            return Err(RelayError::BadRequest(
                "public_key_b64 must not be empty".into(),
            ));
        }
        let key_bytes = B64
            .decode(&public_key_b64)
            .map_err(|_| RelayError::BadRequest("invalid base64 for public_key_b64".into()))?;
        if key_bytes.len() != 32 {
            return Err(RelayError::BadRequest(format!(
                "public_key_b64 must decode to exactly 32 bytes, got {}",
                key_bytes.len()
            )));
        }

        // Proof-of-possession (PoP) verification — fixes CopyPaste-n2l.
        //
        // The registrant must prove it holds the sync key that `device_id` was
        // derived from by presenting `HMAC-SHA256(key=sync_key, msg=PREFIX ||
        // device_id)`. The relay cannot recompute the HMAC (it never learns the
        // sync key), so verification works as follows:
        //
        //   - The `pop_b64` field decodes to exactly 32 bytes and is stored on
        //     FIRST registration.
        //   - On CO-REGISTRATION the presented PoP is compared against the stored
        //     one with a constant-time equality check. A mismatch means the new
        //     registrant does NOT hold the same sync key — rejected.
        //
        // This closes the attack where an adversary who has learned the secret
        // `device_id` (e.g. via traffic analysis) co-registers and receives the
        // victim's inbox ciphertext.
        if pop_b64.is_empty() {
            return Err(RelayError::BadRequest(
                "pop_b64 is required for registration".into(),
            ));
        }
        let pop_bytes_vec = B64
            .decode(&pop_b64)
            .map_err(|_| RelayError::BadRequest("invalid base64 for pop_b64".into()))?;
        if pop_bytes_vec.len() != 32 {
            return Err(RelayError::BadRequest(format!(
                "pop_b64 must decode to exactly 32 bytes (HMAC-SHA256 output), got {}",
                pop_bytes_vec.len()
            )));
        }
        let mut pop_bytes = [0u8; 32];
        pop_bytes.copy_from_slice(&pop_bytes_vec);

        // On co-registration: constant-time compare against stored PoP.
        if let Some(existing) = self.devices.get(&device_id) {
            use subtle::ConstantTimeEq;
            if existing.registered_pop.ct_eq(&pop_bytes).unwrap_u8() != 1 {
                return Err(RelayError::BadRequest(
                    "pop_b64 does not match the registered proof-of-possession for this device_id"
                        .into(),
                ));
            }
        }
        // On first registration: the PoP is accepted as-is and stored below.
        // (We cannot independently verify it — the relay never learns the sync
        // key — but this still closes the attack: an attacker who only knows the
        // device_id cannot compute the correct HMAC without the sync key.)

        // Enforce the device-count quota *within this scope* only for a NEW
        // device record. Co-registration reuses an existing record (one inbox),
        // so it must not be charged against the quota — otherwise the account's
        // own subsequent devices would be rejected once the cap is reached even
        // though they all share a single inbox. Count only devices that
        // registered from the same client IP so the cap is per-scope, not a
        // global ceiling (H1).
        if !is_co_registration {
            let scope_count = self
                .devices
                .values()
                .filter(|r| r.registered_from_ip == client_ip)
                .count();
            quota::check_device_quota(tier, scope_count).map_err(|v| match v {
                QuotaViolation::MaxDevicesExceeded { limit } => {
                    RelayError::DeviceQuotaExceeded { limit }
                }
                // `check_device_quota` only ever returns `MaxDevicesExceeded`,
                // but the enum has other variants (ItemTooLarge / HistoryFull)
                // returned by other quota functions. Map them to Internal rather
                // than panicking — a new variant should not bring down the relay.
                QuotaViolation::ItemTooLarge { limit_bytes } => RelayError::Internal(format!(
                    "unexpected ItemTooLarge({limit_bytes}) from check_device_quota"
                )),
                // `HistoryFull` is cfg-gated alongside `check_history_quota`.
                #[cfg(any(test, feature = "quota-tiers"))]
                QuotaViolation::HistoryFull { limit } => RelayError::Internal(format!(
                    "unexpected HistoryFull({limit}) from check_device_quota"
                )),
            })?;
        }

        // Read the wall clock *before* issuing the token. A token whose
        // `expires_at_unix` is computed from a bogus near-epoch clock would be
        // born already-expired, so every device it is issued to would get
        // Unauthorized on the next request — a silent, total outage. Treat a
        // `duration_since(UNIX_EPOCH)` error (clock before the epoch) or an
        // implausibly-near-epoch reading as fatal and refuse to issue a token
        // rather than handing back a dead credential. `MIN_PLAUSIBLE_UNIX` is
        // 2020-01-01; any correctly-set host clock is far past it.
        const MIN_PLAUSIBLE_UNIX: u64 = 1_577_836_800; // 2020-01-01T00:00:00Z
        let now_unix = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(d) if d.as_secs() >= MIN_PLAUSIBLE_UNIX => d.as_secs() as i64,
            other => {
                tracing::error!(
                    ?other,
                    "host clock is before {MIN_PLAUSIBLE_UNIX} (near-epoch or pre-epoch); \
                     refusing to issue an auth token that would be born expired"
                );
                return Err(RelayError::Internal(
                    "server clock is not set correctly; cannot issue auth token".into(),
                ));
            }
        };
        let expires_at_unix = now_unix + 365 * 24 * 3600;

        // Generate bearer token from 16 random bytes (NEVER derive from
        // public key — that would let any client compute the secret).
        // Output: 32 hex characters representing 16 bytes of entropy.
        let mut token_bytes = [0u8; 16];
        OsRng.fill_bytes(&mut token_bytes);
        let bearer_token = super::hex_encode(&token_bytes);

        let now = Instant::now();
        match self.devices.get_mut(&device_id) {
            // Co-registration: keep the existing record (name, key, registered_at,
            // inbox) and just add the new independent token to its token set.
            // Do NOT advance `last_seen` here — that is reserved for actual
            // authenticated push/pull/subscribe traffic via `update_last_seen`.
            Some(record) => {
                record.add_token(bearer_token.clone(), expires_at_unix, now_unix);
                // R1b write-through: the token set may have been pruned/FIFO-
                // evicted by `add_token`, so persist the full current set so the
                // on-disk order + membership stays byte-identical to memory.
                let tokens: Vec<(String, i64)> = record
                    .tokens
                    .iter()
                    .map(|t| (t.token.clone(), t.expires_at_unix))
                    .collect();
                self.db.replace_tokens(&device_id, &tokens)?;
            }
            // First registration of this id: insert a fresh record.
            None => {
                let ip_str = client_ip.map(|ip| ip.to_string());
                self.devices.insert(
                    device_id.clone(),
                    DeviceRecord {
                        device_id: device_id.clone(),
                        device_name: device_name.clone(),
                        public_key_b64: public_key_b64.clone(),
                        registered_pop: pop_bytes,
                        tokens: vec![TokenEntry {
                            token: bearer_token.clone(),
                            expires_at_unix,
                        }],
                        registered_at: now,
                        last_seen: now,
                        tier,
                        registered_from_ip: client_ip,
                    },
                );
                // Pre-create an empty inbox so pull works without a separate
                // device-check.
                self.sync_items.entry(device_id.clone()).or_default();
                // R1b write-through: persist the new device record + its first
                // token. `registered_at`/`last_seen` are stored as Unix seconds
                // (`now_unix`) so they can be rehydrated relative to the
                // monotonic clock on restart.
                self.db.insert_device(
                    &device_id,
                    &device_name,
                    &public_key_b64,
                    super::tier_to_str(tier),
                    ip_str.as_deref(),
                    now_unix,
                    now_unix,
                    &pop_b64,
                )?;
                self.db
                    .replace_tokens(&device_id, &[(bearer_token.clone(), expires_at_unix)])?;
            }
        }

        Ok((bearer_token, expires_at_unix))
    }

    /// Register a new device using the default tier (`Tier::Free`), unscoped
    /// (shared `None` scope). Convenience wrapper used by tests.
    ///
    /// Returns `(bearer_token, expires_at_unix)` on success.
    // Production uses `register_device_scoped`; this unscoped form is used by
    // the test suites that don't drive a real transport. When `quota-tiers` is
    // enabled (e.g. --all-features) it is included but has no non-test caller
    // — allow suppresses dead_code.
    #[cfg(any(test, feature = "quota-tiers"))]
    #[allow(dead_code)] // intentional: test helper, no production caller
    pub fn register_device(
        &mut self,
        device_id: String,
        device_name: String,
        public_key_b64: String,
        pop_b64: String,
    ) -> Result<(String, i64), RelayError> {
        self.register_device_with_tier(device_id, device_name, public_key_b64, pop_b64, Tier::Free)
    }

    /// Register a new device using the default tier (`Tier::Free`), scoped to
    /// `client_ip` for the per-scope device quota (H1). Used by the HTTP
    /// registration handler, which supplies the connecting client's IP.
    ///
    /// Returns `(bearer_token, expires_at_unix)` on success.
    pub fn register_device_scoped(
        &mut self,
        client_ip: Option<IpAddr>,
        device_id: String,
        device_name: String,
        public_key_b64: String,
        pop_b64: String,
    ) -> Result<(String, i64), RelayError> {
        self.register_device_with_tier_scoped(
            client_ip,
            device_id,
            device_name,
            public_key_b64,
            pop_b64,
            Tier::Free,
        )
    }
}

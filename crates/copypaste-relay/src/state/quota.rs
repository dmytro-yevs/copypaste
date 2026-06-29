//! Tier / quota accounting: string serialisation helpers, effective-cap
//! computation, and per-store tier queries.
//!
//! These functions are small but referenced from several submodules
//! (`inbox`, `registration`) and the store constructors, so they live here
//! rather than scattered across those files.

use crate::quota::Tier;

use super::MAX_PUSH_ITEMS_PER_DEVICE;

// ---------------------------------------------------------------------------
// Tier string serialisation (used by rehydrate_from_db + registration)
// ---------------------------------------------------------------------------

/// Map a [`Tier`] to its persisted string form.
pub(crate) fn tier_to_str(tier: Tier) -> &'static str {
    match tier {
        Tier::Free => "free",
        // `Tier::Pro` is cfg-gated; this arm only exists when the variant does.
        #[cfg(any(test, feature = "quota-tiers"))]
        Tier::Pro => "pro",
    }
}

/// Parse a persisted tier string back to a [`Tier`]. Unknown values fall back
/// to `Free` (the conservative default — never grant a wider quota than stored).
pub(crate) fn tier_from_str(s: &str) -> Tier {
    match s {
        // `Tier::Pro` is cfg-gated; fall back to `Free` in production builds
        // that don't enable the `quota-tiers` feature.
        #[cfg(any(test, feature = "quota-tiers"))]
        "pro" => Tier::Pro,
        _ => Tier::Free,
    }
}

// ---------------------------------------------------------------------------
// History-cap helpers (used by inbox push + tests)
// ---------------------------------------------------------------------------

/// Effective per-inbox history cap for a device: the tighter of the absolute
/// hard cap [`MAX_PUSH_ITEMS_PER_DEVICE`] and the device tier's
/// `max_history_items` (`None` = unlimited tier history → only the hard cap
/// applies). Enforced as a silent prune-oldest inside `RelayStore::push_item`
/// — the sender is never told a recipient inbox is full, matching the existing
/// hard-cap eviction behaviour (see the relay v2 quotas plan).
// pub(crate) so inbox.rs and its test blocks can import directly.
pub(crate) fn effective_history_cap(tier: Tier) -> usize {
    history_cap_for_limit(tier.max_history_items())
}

/// Core of [`effective_history_cap`]: clamp a tier's optional `max_history_items`
/// against the absolute hard cap. `None` (unlimited tier history) yields the
/// hard cap; a limit tighter than the hard cap wins. Split out so the
/// clamp can be unit-tested with a genuinely sub-hard-cap limit (no live tier
/// currently defines one — see `effective_history_cap_is_tier_aware`).
// pub(crate) so test blocks can call it directly.
pub(crate) fn history_cap_for_limit(tier_limit: Option<usize>) -> usize {
    MAX_PUSH_ITEMS_PER_DEVICE.min(tier_limit.unwrap_or(usize::MAX))
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::quota::Tier;

    use super::super::MAX_PUSH_ITEMS_PER_DEVICE;
    use super::{effective_history_cap, history_cap_for_limit, tier_from_str, tier_to_str};

    #[test]
    fn tier_to_str_and_back_roundtrip() {
        assert_eq!(tier_from_str(tier_to_str(Tier::Free)), Tier::Free);
        assert_eq!(tier_to_str(Tier::Free), "free");
        assert_eq!(tier_from_str("free"), Tier::Free);
        assert_eq!(
            tier_from_str("unknown"),
            Tier::Free,
            "unknown falls back to Free"
        );
    }

    #[cfg(feature = "quota-tiers")]
    #[test]
    fn tier_pro_roundtrip() {
        assert_eq!(tier_to_str(Tier::Pro), "pro");
        assert_eq!(tier_from_str("pro"), Tier::Pro);
    }

    #[test]
    fn effective_history_cap_clamps_to_hard_cap() {
        // Free: tier limit is 1000, hard cap is 500 → effective is 500.
        assert_eq!(effective_history_cap(Tier::Free), MAX_PUSH_ITEMS_PER_DEVICE);
        // Pro: unlimited tier history → bounded only by the hard cap.
        assert_eq!(effective_history_cap(Tier::Pro), MAX_PUSH_ITEMS_PER_DEVICE);
    }

    #[test]
    fn history_cap_for_limit_tight_limit_wins() {
        let tight = 10usize;
        assert!(tight < MAX_PUSH_ITEMS_PER_DEVICE);
        assert_eq!(history_cap_for_limit(Some(tight)), 10);
    }

    #[test]
    fn history_cap_for_limit_none_gives_hard_cap() {
        assert_eq!(history_cap_for_limit(None), MAX_PUSH_ITEMS_PER_DEVICE);
    }
}

// ---------------------------------------------------------------------------
// RelayStore: quota queries
// ---------------------------------------------------------------------------

impl super::RelayStore {
    /// CopyPaste-crh3.115: the subscription [`Tier`] registered for `device_id`,
    /// so the push path can apply the sender's ACTUAL per-tier item-size quota
    /// instead of conservatively assuming `Tier::Free` for everyone.
    ///
    /// Returns `Tier::Free` for an unknown device — a conservative default that
    /// never over-grants. In production (no `quota-tiers` feature) `Tier::Free`
    /// is the only variant, so this is a no-op there; it future-proofs the push
    /// quota for when paid tiers ship.
    pub fn device_tier(&self, device_id: &str) -> Tier {
        self.devices
            .get(device_id)
            .map(|r| r.tier)
            .unwrap_or(Tier::Free)
    }

    /// Returns `(device_count, total_inbox_items)`.
    // Not called from any current route handler (CopyPaste-j21: counts stripped
    // from unauthenticated endpoints). When `quota-tiers` is enabled (e.g.
    // --all-features) it is included but has no non-test caller — allow
    // suppresses dead_code.
    #[cfg(any(test, feature = "quota-tiers"))]
    #[allow(dead_code)] // intentional: test helper, no production caller today
    pub fn stats(&self) -> (usize, usize) {
        let total = self.sync_items.values().map(|v| v.len()).sum();
        (self.devices.len(), total)
    }
}

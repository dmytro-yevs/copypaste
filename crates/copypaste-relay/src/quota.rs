/// Device tier — determines quotas applied to a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(dead_code)]
pub enum Tier {
    /// Default tier: capped registration, history, and item size limits.
    #[default]
    Free,
    /// Paid tier: elevated limits.
    Pro,
}

impl Tier {
    /// Maximum number of devices that may be registered for this tier.
    /// This limit is applied account-wide (all devices share the same tier).
    pub fn max_devices(self) -> usize {
        match self {
            Tier::Free => 5,
            Tier::Pro => 10,
        }
    }

    /// Maximum number of history items kept per device inbox.
    /// `None` means unlimited.
    #[allow(dead_code)]
    pub fn max_history_items(self) -> Option<usize> {
        match self {
            Tier::Free => Some(1_000),
            Tier::Pro => None,
        }
    }

    /// Maximum decoded ciphertext size in bytes for a single clipboard item.
    #[allow(dead_code)]
    pub fn max_item_bytes(self, content_type: &str) -> usize {
        match (self, content_type) {
            // Images: 10 MiB for both tiers (pro has no additional benefit here).
            (_, "image") => 10 * 1024 * 1024,
            // Text: 1 MiB for both tiers.
            _ => 1024 * 1024,
        }
    }
}

/// Quota violation kind returned when a quota check fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuotaViolation {
    /// The account has reached the maximum number of registered devices.
    MaxDevicesExceeded { limit: usize },
    /// The uploaded item exceeds the size limit for its content type.
    #[allow(dead_code)]
    ItemTooLarge { limit_bytes: usize },
    /// The device inbox has reached its maximum history count.
    #[allow(dead_code)]
    HistoryFull { limit: usize },
}

/// Check whether adding a new device would exceed the account's device quota.
///
/// `current_device_count` is the number of devices currently registered.
/// Returns `Err(QuotaViolation::MaxDevicesExceeded)` if the quota is exceeded.
pub fn check_device_quota(tier: Tier, current_device_count: usize) -> Result<(), QuotaViolation> {
    let limit = tier.max_devices();
    if current_device_count >= limit {
        Err(QuotaViolation::MaxDevicesExceeded { limit })
    } else {
        Ok(())
    }
}

/// Check whether a clipboard item's size is within the allowed limit.
///
/// `payload_bytes` is the number of decoded ciphertext bytes.
/// `content_type` is e.g. `"text"`, `"image"`, or `"file"`.
#[allow(dead_code)]
pub fn check_item_size(
    tier: Tier,
    payload_bytes: usize,
    content_type: &str,
) -> Result<(), QuotaViolation> {
    let limit = tier.max_item_bytes(content_type);
    if payload_bytes > limit {
        Err(QuotaViolation::ItemTooLarge { limit_bytes: limit })
    } else {
        Ok(())
    }
}

/// Check whether the device inbox still has capacity for one more item.
///
/// `current_count` is the number of items already in the inbox.
#[allow(dead_code)]
pub fn check_history_quota(tier: Tier, current_count: usize) -> Result<(), QuotaViolation> {
    if let Some(limit) = tier.max_history_items() {
        if current_count >= limit {
            return Err(QuotaViolation::HistoryFull { limit });
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Tier limits -------------------------------------------------------

    #[test]
    fn free_tier_max_devices_is_5() {
        assert_eq!(Tier::Free.max_devices(), 5);
    }

    #[test]
    fn pro_tier_max_devices_is_10() {
        assert_eq!(Tier::Pro.max_devices(), 10);
    }

    #[test]
    fn free_tier_max_history_is_1000() {
        assert_eq!(Tier::Free.max_history_items(), Some(1_000));
    }

    #[test]
    fn pro_tier_history_is_unlimited() {
        assert_eq!(Tier::Pro.max_history_items(), None);
    }

    #[test]
    fn text_item_limit_is_1mib() {
        assert_eq!(Tier::Free.max_item_bytes("text"), 1024 * 1024);
        assert_eq!(Tier::Pro.max_item_bytes("text"), 1024 * 1024);
    }

    #[test]
    fn image_item_limit_is_10mib() {
        assert_eq!(Tier::Free.max_item_bytes("image"), 10 * 1024 * 1024);
        assert_eq!(Tier::Pro.max_item_bytes("image"), 10 * 1024 * 1024);
    }

    // ---- Device quota ------------------------------------------------------

    #[test]
    fn check_device_quota_ok_when_under_limit() {
        assert!(check_device_quota(Tier::Free, 4).is_ok());
        assert!(check_device_quota(Tier::Pro, 9).is_ok());
    }

    #[test]
    fn check_device_quota_fails_at_limit() {
        let err = check_device_quota(Tier::Free, 5).unwrap_err();
        assert_eq!(err, QuotaViolation::MaxDevicesExceeded { limit: 5 });
    }

    #[test]
    fn sixth_free_device_is_rejected() {
        // Simulates 5 already registered devices — 6th must fail.
        let err = check_device_quota(Tier::Free, 5).unwrap_err();
        match err {
            QuotaViolation::MaxDevicesExceeded { limit } => assert_eq!(limit, 5),
            _ => panic!("expected MaxDevicesExceeded"),
        }
    }

    #[test]
    fn eleventh_pro_device_is_rejected() {
        let err = check_device_quota(Tier::Pro, 10).unwrap_err();
        assert_eq!(err, QuotaViolation::MaxDevicesExceeded { limit: 10 });
    }

    // ---- Item size quota ---------------------------------------------------

    #[test]
    fn check_item_size_ok_for_text_within_limit() {
        assert!(check_item_size(Tier::Free, 512 * 1024, "text").is_ok());
    }

    #[test]
    fn oversized_text_item_is_rejected() {
        let err = check_item_size(Tier::Free, 1024 * 1024 + 1, "text").unwrap_err();
        assert_eq!(
            err,
            QuotaViolation::ItemTooLarge {
                limit_bytes: 1024 * 1024
            }
        );
    }

    #[test]
    fn oversized_image_item_is_rejected() {
        let limit = 10 * 1024 * 1024;
        let err = check_item_size(Tier::Free, limit + 1, "image").unwrap_err();
        assert_eq!(err, QuotaViolation::ItemTooLarge { limit_bytes: limit });
    }

    #[test]
    fn image_within_10mib_is_accepted() {
        assert!(check_item_size(Tier::Free, 10 * 1024 * 1024, "image").is_ok());
    }

    // ---- History quota -----------------------------------------------------

    #[test]
    fn history_quota_ok_when_under_limit() {
        assert!(check_history_quota(Tier::Free, 999).is_ok());
    }

    #[test]
    fn history_quota_fails_at_1000_for_free() {
        let err = check_history_quota(Tier::Free, 1000).unwrap_err();
        assert_eq!(err, QuotaViolation::HistoryFull { limit: 1000 });
    }

    #[test]
    fn pro_history_is_never_full() {
        // Even at a very high count, pro tier should not report HistoryFull.
        assert!(check_history_quota(Tier::Pro, 999_999).is_ok());
    }
}

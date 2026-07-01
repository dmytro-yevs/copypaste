//! Shared "Wi-Fi only" outbound gate.
//!
//! Split out of the former flat `sync_common.rs` (ADR-017, CopyPaste-vp63.7)
//! — moved verbatim, no behavior change.

/// Shared "Wi-Fi only" outbound gate (CopyPaste-7ub).
///
/// Returns `true` when an outbound sync transmission should be SKIPPED because
/// the user enabled `sync_on_wifi_only` and the device is not currently on
/// Wi-Fi. Used by the P2P fanout path so it honours the privacy setting exactly
/// like the relay (`relay/push.rs`) and cloud paths. Fail-open is the caller's
/// responsibility: pass `on_wifi = true` when the platform Wi-Fi probe errors.
pub fn should_skip_on_cellular(sync_on_wifi_only: bool, on_wifi: bool) -> bool {
    sync_on_wifi_only && !on_wifi
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_skip_on_cellular_truth_table() {
        // Only skip when the user opted into Wi-Fi-only AND we are off Wi-Fi.
        assert!(
            should_skip_on_cellular(true, false),
            "wifi-only + cellular → skip"
        );
        assert!(
            !should_skip_on_cellular(true, true),
            "wifi-only + on wifi → send"
        );
        assert!(
            !should_skip_on_cellular(false, false),
            "flag off + cellular → send"
        );
        assert!(
            !should_skip_on_cellular(false, true),
            "flag off + on wifi → send"
        );
    }
}

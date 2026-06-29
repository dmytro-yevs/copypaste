//! mDNS advertisement helpers: TXT record construction, opaque instance labels,
//! and periodic re-announcement.

use mdns_sd::{ServiceDaemon, ServiceInfo};
use tracing::warn;

use super::types::{
    Registration, DNS_LABEL_MAX, PROTOCOL_VERSION, SERVICE_TYPE, TXT_BPORT, TXT_DEVICE_ID,
    TXT_VERSION,
};
use crate::error::DiscoveryError;

/// Build the ordered TXT record property list for our mDNS advertisement.
///
/// The human device name is intentionally **not** included — it is PII and
/// must not be broadcast to passive LAN observers. Paired peers learn the
/// human name through the post-PAKE authenticated exchange instead.
///
/// Returns a `Vec` of `(key, value)` pairs in stable order. The caller is
/// responsible for allocating the `bport` string and extending the slice.
pub(super) fn build_txt_properties(device_id: &str) -> Vec<(&'static str, String)> {
    vec![
        (TXT_VERSION, PROTOCOL_VERSION.to_string()),
        (TXT_DEVICE_ID, device_id.to_string()),
        // TXT_DEVICE_NAME is deliberately absent — see CopyPaste-sh9a.
    ]
}

/// Build the opaque mDNS instance label from a `device_id`.
///
/// Computes SHA-256 over the `device_id` string and uses the first 8 hex
/// characters of the digest prefixed with `"cp-"`. This ensures:
///
/// * The raw `device_id` prefix is NOT present in the label — a passive LAN
///   observer cannot distinguish `cp-5f4dcc3b` (hash of "password") from any
///   other device label without already knowing the target `device_id`.
/// * The label is deterministic for a given `device_id`, so it remains stable
///   across daemon restarts on the same device.
/// * Different `device_id` values produce different labels (no trivial collision
///   in the 8-hex-char output space for realistic device counts).
///
/// The result is guaranteed to be ≤ `DNS_LABEL_MAX` (63) characters.
/// ("cp-" = 3 chars) + (8 hex digits) = 11 chars total.
///
/// CopyPaste-rt50 root cause: the previous implementation used
/// `device_id.chars().take(8)`, which directly exposed the first 8 characters
/// of the stable device fingerprint — allowing a passive LAN observer to durably
/// track a device across network changes by its mDNS instance name.
///
/// TODO(CopyPaste-sh9a): For stronger unlinkability across sessions, derive the
/// label from a daily HKDF epoch (HKDF(static_key, salt='copypaste/label/' +
/// floor(now/86400))) so it rotates once per day without requiring re-pairing.
pub(super) fn opaque_instance_label(device_id: &str) -> String {
    use sha2::Digest as _;
    // "cp-" (3) + 8 hex digits = 11 chars, well within DNS_LABEL_MAX.
    const _: () = assert!(3 + 8 <= DNS_LABEL_MAX);
    let digest = sha2::Sha256::digest(device_id.as_bytes());
    // Take the first 4 bytes (8 hex chars) of the SHA-256 digest.
    // This is not a cryptographic secret — it is a one-way transform to prevent
    // the raw device_id prefix from appearing in unauthenticated mDNS frames.
    let id_short = hex::encode(&digest[..4]);
    format!("cp-{id_short}")
}

/// Build the hostname label (the part before `.local.`) for mDNS advertisement.
///
/// Uses the opaque instance label so the hostname does not embed the human
/// device name. Previously this used the sanitised device name.
pub(super) fn opaque_hostname_label(device_id: &str) -> String {
    opaque_instance_label(device_id)
}

/// Re-announce own mDNS service with fresh interface addresses.
///
/// Unregisters the existing record (ignoring errors — the daemon may have
/// already removed it if the interface disappeared) then re-registers with
/// the addresses returned by [`crate::interfaces::usable_advertise_addrs`]
/// at the moment of the call.
///
/// Called by the periodic re-announce task spawned in [`super::service::DiscoveryService::start`].
/// The service-info building mirrors [`super::service::DiscoveryService::advertise`] — keep
/// both in sync if the advertisement format changes.
pub(super) fn reannounce_once(
    daemon: &ServiceDaemon,
    reg: &Registration,
) -> Result<(), DiscoveryError> {
    // Compute the deterministic fullname for the currently registered record.
    // ServiceInfo::get_fullname() returns "{instance}.{service_type}"; since both
    // components are derived solely from reg.device_id and the fixed SERVICE_TYPE,
    // we can reconstruct it without keeping a copy from the original advertise() call.
    let fullname = format!("{}.{}", opaque_instance_label(&reg.device_id), SERVICE_TYPE);

    // Unregister the stale record. Fire-and-forget the status receiver — we do
    // not wait for the DNS goodbye packet; the follow-up register() takes effect
    // regardless. Errors are ignored: the record may already be absent if the
    // network interface went down.
    let _ = daemon.unregister(&fullname);

    // Re-register with the current interface address list (mirrors advertise()).
    let instance_name = opaque_instance_label(&reg.device_id);
    let hostname = format!("{}.local.", opaque_hostname_label(&reg.device_id));
    let base_props = build_txt_properties(&reg.device_id);
    let bport_str: String;
    let mut properties: Vec<(&str, &str)> =
        base_props.iter().map(|(k, v)| (*k, v.as_str())).collect();
    if let Some(bp) = reg.bport {
        bport_str = bp.to_string();
        properties.push((TXT_BPORT, bport_str.as_str()));
    }
    let usable_addrs = crate::interfaces::usable_advertise_addrs();
    let service_info = if usable_addrs.is_empty() {
        warn!("re-announce: no usable LAN interface; letting mdns-sd auto-detect addresses");
        ServiceInfo::new(
            SERVICE_TYPE,
            &instance_name,
            &hostname,
            (),
            reg.port,
            &properties[..],
        )
    } else {
        ServiceInfo::new(
            SERVICE_TYPE,
            &instance_name,
            &hostname,
            &usable_addrs[..],
            reg.port,
            &properties[..],
        )
    }
    .map_err(|e| DiscoveryError::Register(e.to_string()))?;

    daemon
        .register(service_info)
        .map_err(|e| DiscoveryError::Register(e.to_string()))
}

/// Replace characters that are invalid in mDNS labels with hyphens and
/// trim leading/trailing hyphens.
///
/// If the result is empty (e.g. the device name consists entirely of
/// non-alphanumeric characters such as `"!!!"`) a hardcoded fallback label
/// `"copypaste"` is substituted so `ServiceInfo` is never constructed with an
/// invalid `".{id}"` label that would cause `mdns-sd` to reject registration.
///
/// No longer called in production code (CopyPaste-sh9a: opaque labels replace
/// human-name-based labels) but retained for tests so existing `sanitize_label`
/// test coverage continues to document the invariants.
#[cfg(test)]
pub(super) fn sanitize_label(s: &str) -> String {
    let sanitized: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = sanitized.trim_matches('-');
    if trimmed.is_empty() {
        "copypaste".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── sanitize_label ───────────────────────────────────────────────────────

    #[test]
    fn sanitize_label_keeps_alphanumeric_and_hyphen() {
        assert_eq!(sanitize_label("Alice-Mac"), "Alice-Mac");
    }

    #[test]
    fn sanitize_label_replaces_spaces() {
        assert_eq!(sanitize_label("Alice's Mac"), "Alice-s-Mac");
    }

    #[test]
    fn sanitize_label_trims_leading_trailing_hyphens() {
        assert_eq!(sanitize_label(" hello "), "hello");
    }

    #[test]
    fn sanitize_label_empty_input() {
        // Empty input → empty after trim → fallback label returned.
        assert_eq!(sanitize_label(""), "copypaste");
    }

    #[test]
    fn sanitize_label_pure_special_chars_becomes_fallback() {
        // All specials → hyphens → trimmed → empty → fallback label.
        // ServiceInfo must never be created with an empty label (invalid mDNS).
        assert_eq!(sanitize_label("!!!"), "copypaste");
    }

    // ── opaque_instance_label: raw-prefix privacy (CopyPaste-rt50) ──────────

    /// The label must NOT start with the raw device_id prefix.
    ///
    /// Before CopyPaste-rt50 the label was `"cp-{first8charsOfDeviceId}"`, letting
    /// a passive LAN observer durably track the device. After the fix the label is
    /// `"cp-{first8hexCharsOfSha256(device_id)}"` — the raw prefix never appears.
    #[test]
    fn opaque_instance_label_does_not_expose_raw_device_id_prefix() {
        let device_id = "aabbccdddeadbeef0011223344556677";
        let label = opaque_instance_label(device_id);
        // The raw first-8-chars prefix must NOT appear in the label.
        let raw_prefix = &device_id[..8]; // "aabbccdd"
        assert!(
            !label.contains(raw_prefix),
            "label must not contain the raw device_id prefix '{raw_prefix}', got: {label}"
        );
        // The label still has the 'cp-' prefix and is exactly 11 chars.
        assert!(
            label.starts_with("cp-"),
            "label must start with 'cp-': {label}"
        );
        assert_eq!(label.len(), 11, "label must be exactly 11 chars: {label}");
        // All chars after 'cp-' are lowercase hex digits.
        assert!(
            label[3..].chars().all(|c| c.is_ascii_hexdigit()),
            "label suffix must be lowercase hex: {label}"
        );
    }

    // ── privacy: TXT name redaction (CopyPaste-sh9a) ─────────────────────────

    /// The opaque instance label must NOT contain the human device name.
    ///
    /// Regression guard: the old scheme embedded the human name directly into
    /// the mDNS label (e.g. `"Alice-s-MacBook.aabbccdd"`), leaking PII to any
    /// passive LAN observer.
    #[test]
    fn opaque_instance_label_does_not_contain_human_name() {
        let label = opaque_instance_label("aabbccdddeadbeef");
        assert!(
            label.starts_with("cp-"),
            "opaque label must start with 'cp-' prefix, got: {label}"
        );
        assert!(
            label.len() <= DNS_LABEL_MAX,
            "opaque label exceeds DNS_LABEL_MAX: {label}"
        );
        // Verify no free-form string ended up in the label.
        assert!(!label.contains("Alice"), "name must not appear in label");
        assert!(!label.contains("Mac"), "name must not appear in label");
    }

    /// The opaque label is determined solely by `device_id`.
    #[test]
    fn opaque_instance_label_depends_only_on_device_id() {
        let label_a = opaque_instance_label("aabbccdd00000000");
        let label_b = opaque_instance_label("aabbccdd00000000");
        assert_eq!(label_a, label_b, "same device_id must produce same label");

        let label_c = opaque_instance_label("1122334400000000");
        assert_ne!(
            label_a, label_c,
            "different device_id must produce different label"
        );
    }

    /// `build_txt_properties` must NOT contain `TXT_DEVICE_NAME`.
    ///
    /// This is the primary regression guard for CopyPaste-sh9a: if someone
    /// accidentally adds the human name back to the emitted TXT record, this
    /// test will catch it immediately.
    #[test]
    fn build_txt_properties_does_not_include_device_name_key() {
        use super::super::types::TXT_DEVICE_NAME;
        let device_id = "aabbccdd12345678";
        let props = build_txt_properties(device_id);
        for (k, _v) in &props {
            assert_ne!(
                *k, TXT_DEVICE_NAME,
                "TXT record must not include '{TXT_DEVICE_NAME}' key (PII leak)"
            );
        }
    }

    /// `build_txt_properties` must contain `TXT_DEVICE_ID` and `TXT_VERSION`.
    ///
    /// The `did` key is required for pairing resolution (peers dial by mDNS
    /// `did` when no address hint is available).
    #[test]
    fn build_txt_properties_contains_did_and_version() {
        let device_id = "cafebabe12345678";
        let props = build_txt_properties(device_id);
        let keys: Vec<&str> = props.iter().map(|(k, _)| *k).collect();
        assert!(
            keys.contains(&TXT_DEVICE_ID),
            "TXT must contain '{TXT_DEVICE_ID}' for pairing resolution"
        );
        assert!(
            keys.contains(&TXT_VERSION),
            "TXT must contain '{TXT_VERSION}'"
        );
        // did value must match the device_id passed in.
        let did_val = props
            .iter()
            .find(|(k, _)| *k == TXT_DEVICE_ID)
            .map(|(_, v)| v.as_str());
        assert_eq!(did_val, Some(device_id));
    }

    /// The human device name must not appear in any value of the TXT properties.
    #[test]
    fn build_txt_properties_values_do_not_contain_human_name() {
        let device_id = "aabbccdd12345678";
        let human_name = "Alice's MacBook Pro";
        let props = build_txt_properties(device_id);
        for (_k, v) in &props {
            assert!(
                !v.contains("Alice"),
                "human name must not appear in TXT value '{v}'"
            );
            let _ = human_name;
        }
    }
}

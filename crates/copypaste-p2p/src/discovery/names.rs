//! Turning arbitrary text into something DNS and a log line can hold. On the way
//! *out*, a user-supplied device name has to fit mDNS label rules; on the way
//! *in*, every field came from an unauthenticated source on the LAN, so it is
//! length-bounded and stripped of control characters before it can reach a log
//! line or the UI.

use crate::SERVICE_TYPE;

/// Longest pairing id accepted or advertised, in bytes.
pub(super) const MAX_PAIRING_ID_LEN: usize = 64;
/// Longest device name accepted or advertised, in bytes. Keeps `n=<name>`
/// inside the 255-byte limit RFC 6763 §6.1 puts on one TXT string.
pub(super) const MAX_NAME_LEN: usize = 128;
/// Longest DNS label, per RFC 1035 §2.3.4.
pub(super) const MAX_LABEL_LEN: usize = 63;

pub(super) fn is_valid_pairing_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= MAX_PAIRING_ID_LEN
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Truncate on a char boundary so the result is still valid UTF-8.
fn truncate_bytes(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Strip control characters — they would otherwise reach logs and the UI from
/// an unauthenticated source.
fn strip_controls(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect()
}

pub(super) fn sanitise_display_name(name: &str) -> Option<String> {
    let cleaned = strip_controls(name);
    let cleaned = truncate_bytes(cleaned.trim(), MAX_NAME_LEN).trim();
    (!cleaned.is_empty()).then(|| cleaned.to_string())
}

/// mDNS instance label. `.` is dropped rather than escaped so that splitting a
/// fullname back into instance + service type stays unambiguous.
pub(super) fn sanitise_instance(name: &str) -> Option<String> {
    let cleaned: String = strip_controls(name)
        .chars()
        .map(|c| if c == '.' || c == '\\' { '-' } else { c })
        .collect();
    let cleaned = truncate_bytes(cleaned.trim(), MAX_LABEL_LEN).trim();
    (!cleaned.is_empty()).then(|| cleaned.to_string())
}

/// A single DNS label for the host record: lowercase ASCII alphanumerics and
/// hyphens only.
pub(super) fn sanitise_host_label(name: &str) -> Option<String> {
    let mapped: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = truncate_bytes(&mapped, MAX_LABEL_LEN)
        .trim_matches('-')
        .to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}

/// Recover the instance label from a fullname such as
/// `Laptop (2)._copypaste._tcp.local.`.
pub(super) fn instance_of(fullname: &str) -> Option<String> {
    let instance = fullname
        .strip_suffix(SERVICE_TYPE)?
        .strip_suffix('.')?
        .to_string();
    (!instance.is_empty()).then_some(instance)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_sanitised_for_dns() {
        assert_eq!(
            sanitise_host_label("Dmitriy's Laptop"),
            Some("dmitriy-s-laptop".to_string())
        );
        assert_eq!(sanitise_host_label("***"), None);
        assert_eq!(sanitise_instance("a.b"), Some("a-b".to_string()));
        assert_eq!(sanitise_instance("  "), None);
        assert_eq!(
            sanitise_display_name("  spaced  "),
            Some("spaced".to_string())
        );
        assert!(sanitise_instance(&"x".repeat(200)).unwrap().len() <= MAX_LABEL_LEN);
        assert!(sanitise_display_name(&"é".repeat(200)).unwrap().len() <= MAX_NAME_LEN);
    }
}

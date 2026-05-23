// fingerprint.rs — Device fingerprint formatting utilities
// Converts raw hex fingerprint to colon-separated display format.

/// Format a raw hex fingerprint string as "XX:XX:XX:XX" groups.
///
/// Accepts any hex string (with or without colons) and outputs colon-separated
/// uppercase pairs.  Non-hex characters are silently stripped.
///
/// # Examples
///
/// ```
/// use copypaste_ui::fingerprint::format_fingerprint;
///
/// let raw = "deadbeef01234567";
/// // max_pairs=4: only the first 4 byte-pairs are returned
/// assert_eq!(format_fingerprint(raw, 4), "DE:AD:BE:EF");
/// // max_pairs=0: all pairs
/// assert_eq!(format_fingerprint(raw, 0), "DE:AD:BE:EF:01:23:45:67");
/// ```
pub fn format_fingerprint(hex: &str, max_pairs: usize) -> String {
    // Strip colons / spaces to get raw hex digits
    let clean: String = hex
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .collect::<String>()
        .to_uppercase();

    clean
        .as_bytes()
        .chunks(2)
        .take(if max_pairs == 0 {
            usize::MAX
        } else {
            max_pairs
        })
        .map(|pair| std::str::from_utf8(pair).unwrap_or("??"))
        .collect::<Vec<_>>()
        .join(":")
}

/// Format for SettingsWindow "About" display: first 4 byte-pairs → "AB:CD:EF:01".
pub fn format_fingerprint_short(hex: &str) -> String {
    format_fingerprint(hex, 4)
}

/// Format for PairWindow own-fingerprint display: first 8 byte-pairs → "AB:CD:EF:01:23:45:67:89".
pub fn format_fingerprint_long(hex: &str) -> String {
    format_fingerprint(hex, 8)
}

/// Format for the paired-device list: "AB:CD:…:EF" (first 2 + last 1 pair with ellipsis).
pub fn format_fingerprint_truncated(hex: &str) -> String {
    let clean: String = hex
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .collect::<String>()
        .to_uppercase();

    let pairs: Vec<&str> = clean
        .as_bytes()
        .chunks(2)
        .map(|p| std::str::from_utf8(p).unwrap_or("??"))
        .collect();

    if pairs.len() <= 3 {
        return pairs.join(":");
    }

    format!("{}:…:{}", pairs[..2].join(":"), pairs[pairs.len() - 1])
}

/// Validate that a string looks like a plausible hex fingerprint (at least 8 hex digits).
pub fn is_valid_fingerprint(s: &str) -> bool {
    let hex_digits: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    hex_digits.len() >= 8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_fingerprint_basic() {
        // max_pairs=4 → only 4 byte-pairs (8 hex chars) are returned
        assert_eq!(format_fingerprint("deadbeef01234567", 4), "DE:AD:BE:EF");
    }

    #[test]
    fn format_fingerprint_max_pairs_limits_output() {
        let hex = "aabbccddeeff00112233";
        let result = format_fingerprint(hex, 4);
        assert_eq!(result, "AA:BB:CC:DD");
    }

    #[test]
    fn format_fingerprint_strips_existing_colons() {
        assert_eq!(format_fingerprint("AA:BB:CC:DD", 4), "AA:BB:CC:DD");
    }

    #[test]
    fn format_fingerprint_strips_spaces() {
        assert_eq!(format_fingerprint("aa bb cc dd", 4), "AA:BB:CC:DD");
    }

    #[test]
    fn format_fingerprint_max_pairs_zero_means_unlimited() {
        let hex = "aabbccdd";
        let result = format_fingerprint(hex, 0);
        assert_eq!(result, "AA:BB:CC:DD");
    }

    #[test]
    fn format_fingerprint_short_takes_4_pairs() {
        let hex = "aabbccddeeff0011223344556677889900aabbcc";
        let result = format_fingerprint_short(hex);
        assert_eq!(result, "AA:BB:CC:DD");
    }

    #[test]
    fn format_fingerprint_long_takes_8_pairs() {
        let hex = "aabbccddeeff00112233";
        let result = format_fingerprint_long(hex);
        assert_eq!(result, "AA:BB:CC:DD:EE:FF:00:11");
    }

    #[test]
    fn format_fingerprint_truncated_short_input() {
        let result = format_fingerprint_truncated("aabb");
        assert_eq!(result, "AA:BB");
    }

    #[test]
    fn format_fingerprint_truncated_long_input() {
        let result = format_fingerprint_truncated("aabbccddeeff");
        // pairs: AA BB CC DD EE FF  (6 pairs) → "AA:BB:…:FF"
        assert_eq!(result, "AA:BB:…:FF");
    }

    #[test]
    fn format_fingerprint_truncated_exactly_3_pairs() {
        let result = format_fingerprint_truncated("aabbcc");
        assert_eq!(result, "AA:BB:CC");
    }

    #[test]
    fn format_fingerprint_empty_returns_empty() {
        assert_eq!(format_fingerprint("", 4), "");
    }

    #[test]
    fn format_fingerprint_uppercase() {
        assert_eq!(format_fingerprint("abcd", 2), "AB:CD");
    }

    #[test]
    fn is_valid_fingerprint_accepts_long_hex() {
        assert!(is_valid_fingerprint("deadbeef01234567"));
    }

    #[test]
    fn is_valid_fingerprint_rejects_short_string() {
        assert!(!is_valid_fingerprint("abc"));
        assert!(!is_valid_fingerprint("abcdefg")); // 7 chars but 'g' not hex → 6 hex digits
    }

    #[test]
    fn is_valid_fingerprint_accepts_colon_formatted() {
        assert!(is_valid_fingerprint("AB:CD:EF:01"));
    }

    #[test]
    fn is_valid_fingerprint_empty_is_invalid() {
        assert!(!is_valid_fingerprint(""));
    }
}

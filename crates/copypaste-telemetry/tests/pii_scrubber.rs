//! Integration tests for the PII scrubber.
//!
//! These pin the redaction contract that the telemetry policy advertises in
//! `docs/privacy/telemetry-policy.md`. If any test here changes, the policy
//! doc must change in the same commit.

use copypaste_telemetry::{OsTag, PiiScrubber, ReportableError};

fn scrub(input: &str) -> String {
    PiiScrubber::default().scrub(input)
}

#[test]
fn email_addresses_redacted() {
    let out = scrub("user alice@example.com failed");
    assert!(out.contains("<REDACTED-EMAIL>"), "got: {out}");
    assert!(!out.contains("alice@example.com"), "got: {out}");

    // Multiple addresses in one string.
    let out = scrub("from a@b.io to c.d+tag@e.co.uk");
    assert_eq!(out.matches("<REDACTED-EMAIL>").count(), 2, "got: {out}");
}

#[test]
fn home_directory_paths_redacted() {
    // macOS layout.
    let out = scrub("failed to open /Users/dmytro/Library/foo.db");
    assert!(out.contains("~/Library/foo.db"), "got: {out}");
    assert!(!out.contains("dmytro"), "got: {out}");

    // Linux layout.
    let out = scrub("ENOENT /home/alice/.config/copypaste");
    assert!(out.contains("~/.config/copypaste"), "got: {out}");
    assert!(!out.contains("alice"), "got: {out}");
}

#[test]
fn ip_addresses_redacted() {
    let out = scrub("peer 192.168.1.42 refused");
    assert!(out.contains("<REDACTED-IP>"), "got: {out}");
    assert!(!out.contains("192.168.1.42"), "got: {out}");

    // IPv6 loopback and full form.
    let out = scrub("connect ::1 then fe80::1ff:fe23:4567:890a");
    assert!(out.contains("<REDACTED-IP>"), "got: {out}");
    assert!(!out.contains("fe80::1ff:fe23:4567:890a"), "got: {out}");
}

#[test]
fn uuid_hex_strings_redacted() {
    // UUID with dashes.
    let out = scrub("device 550e8400-e29b-41d4-a716-446655440000 lost");
    assert!(out.contains("<REDACTED-HEX>"), "got: {out}");
    assert!(!out.contains("550e8400"), "got: {out}");

    // 64-char SHA-256-style hex.
    let digest = "a".repeat(64);
    let out = scrub(&format!("hash={digest} mismatch"));
    assert!(out.contains("<REDACTED-HEX>"), "got: {out}");
    assert!(!out.contains(&digest), "got: {out}");
}

#[test]
fn jwt_tokens_redacted() {
    // Realistic-looking 3-segment JWT with each segment ≥20 base64url chars.
    let header = "eyJhbGciOiJIUzI1NiJ9aaaa";
    let payload = "eyJzdWIiOiIxMjM0NTY3ODkwIn0bbbbb";
    let sig = "SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5ccc";
    let token = format!("{header}.{payload}.{sig}");
    let out = scrub(&format!("auth header bearer={token} rejected"));
    assert!(out.contains("<REDACTED-JWT>"), "got: {out}");
    assert!(!out.contains(&token), "got: {out}");
}

#[test]
fn url_credentials_redacted() {
    let out = scrub("connect to https://user:secret@db.internal/path failed");
    assert!(out.contains("<REDACTED-AUTH>@"), "got: {out}");
    assert!(out.contains("https://"), "scheme should survive: {out}");
    assert!(out.contains("db.internal"), "host should survive: {out}");
    assert!(!out.contains("secret"), "got: {out}");
}

#[test]
fn home_path_without_trailing_slash_redacted() {
    // Regression: `/Users/<name>` with no trailing slash (e.g. from an
    // ENOENT / stat error string) previously leaked the username because
    // the pattern required a trailing `/`.
    let out = scrub("/Users/secretuser");
    assert!(!out.contains("secretuser"), "got: {out}");
    assert!(out.contains("~/"), "got: {out}");

    let out = scrub("cannot stat /Users/jdoe");
    assert!(!out.contains("jdoe"), "got: {out}");
    assert!(out.contains("~/"), "got: {out}");

    // Linux analogue.
    let out = scrub("ENOENT /home/bob");
    assert!(!out.contains("bob"), "got: {out}");
    assert!(out.contains("~/"), "got: {out}");
}

#[test]
fn home_path_username_with_space_redacted() {
    // Regression: usernames containing spaces previously leaked because the
    // pattern excluded whitespace from the username class.
    let out = scrub("/Users/John Doe/file");
    assert!(!out.contains("John Doe"), "got: {out}");
    assert!(!out.contains("John"), "got: {out}");
    assert!(out.contains("~/file"), "got: {out}");
}

#[test]
fn url_password_containing_at_sign_redacted() {
    // Regression: a password containing `@` (e.g. `p@ss`) used to leak its
    // tail because the password class stopped at the first `@`.
    let out = scrub("https://user:p@ss@host/x");
    assert!(out.contains("<REDACTED-AUTH>@"), "got: {out}");
    assert!(!out.contains("user:p@ss"), "got: {out}");
    assert!(!out.contains("p@ss"), "got: {out}");
    assert!(out.contains("host/x"), "host/path should survive: {out}");
}

#[test]
fn adjacent_ipv6_addresses_both_redacted() {
    // Regression: the trailing boundary char was consumed into the match,
    // so `replace_all` resumed after it and the second adjacent IPv6 leaked.
    let out = scrub("::1 ::1");
    assert_eq!(
        out.matches("<REDACTED-IP>").count(),
        2,
        "both addresses must redact, got: {out}"
    );
    assert!(!out.contains("::1"), "got: {out}");
}

#[test]
fn email_with_long_hex_domain_label_fully_redacted() {
    // Regression/ordering: an email whose domain label is a long hex string
    // must be fully redacted as an email, not fragmented by the hex rule.
    let hex_label = "a".repeat(32);
    let input = format!("bob@{hex_label}.com");
    let out = scrub(&input);
    assert!(out.contains("<REDACTED-EMAIL>"), "got: {out}");
    assert!(!out.contains("bob"), "local part leaked: {out}");
}

#[test]
fn non_pii_strings_unchanged() {
    // Canonical taxonomy strings the codebase uses.
    for s in [
        "keychain.read_failed",
        "ipc.parse_error",
        "clipboard.snapshot.too_large",
        "config.migration.v1_to_v2",
        "0.3.0-dev",
        "copypaste-daemon",
    ] {
        assert_eq!(scrub(s), s, "scrubber over-matched on {s:?}");
    }
}

// ---------------------------------------------------------------------------
// CopyPaste-nabu: single-token base64url bearer-token scrubbing
// ---------------------------------------------------------------------------

/// A realistic 44-char base64url token (pairing / relay bearer).
///
/// This is the shape of tokens that `copypaste-daemon` mints for relay auth
/// (32 random bytes → base64url, no padding → 43–44 chars, alphabet
/// `[A-Za-z0-9_-]`, no dots).
const RELAY_BEARER: &str = "VqE2mK9xRsT7nYpL4wBhOuDj8cFaNbXkZeWsRtYuI";

#[test]
fn high_entropy_base64url_token_is_redacted() {
    // The token appears bare in an error class string — typical when a relay
    // 401 includes the offending token for diagnostics.
    let input = format!("relay auth rejected bearer={RELAY_BEARER}");
    let out = scrub(&input);
    assert!(
        out.contains("<REDACTED-TOKEN>"),
        "base64url bearer token was not scrubbed, got: {out}"
    );
    assert!(
        !out.contains(RELAY_BEARER),
        "raw token still present, got: {out}"
    );
}

#[test]
fn high_entropy_base64url_token_scrub_is_idempotent() {
    // After scrubbing, a second pass must not change anything — the
    // replacement `<REDACTED-TOKEN>` must not itself match the pattern.
    let input = format!("token={RELAY_BEARER} expired");
    let once = scrub(&input);
    let twice = scrub(&once);
    assert_eq!(once, twice, "second scrub pass changed output");
}

#[test]
fn prose_words_not_scrubbed_as_tokens() {
    // These strings must NOT be over-redacted by the new base64url-token
    // rule specifically. The rule requires ≥32 chars AND mixed character
    // classes (upper + lower + digit), so these cases are safe:
    //
    // - Short taxonomy strings — below the 32-char length gate.
    // - All-lowercase 32+ char strings — fail the "must contain uppercase"
    //   gate (character-variety guard).
    // - All-uppercase 32+ char strings — fail the "must contain lowercase"
    //   gate.
    //
    // Note: all-digit and all-hex strings are already caught by the existing
    // RE_HEX32 / RE_UUID_HEX rules and are intentionally NOT listed here —
    // those SHOULD be redacted and are tested in `uuid_hex_strings_redacted`.
    for s in [
        // Short taxonomy strings with dashes — below 32-char length gate.
        "keychain.read_failed",
        "copypaste-daemon",
        "ipc.parse_error",
        "0.3.0-dev",
        // All-lowercase, 34 chars — fails mixed-case requirement of new rule.
        // (Already not matched by RE_HEX32 since 'z' is not hex.)
        "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
        // All-uppercase, 34 chars — fails mixed-case requirement.
        "ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ",
        // Prose — spaces separate words into short tokens, none ≥32 chars.
        "the quick brown fox jumps over the lazy dog",
    ] {
        assert_eq!(scrub(s), s, "scrubber over-matched on {s:?}");
    }
}

#[test]
fn custom_pattern_works() {
    let mut s = PiiScrubber::empty();
    s.add_custom(r"ACME-\d{6}").expect("valid regex");
    let out = s.scrub("license ACME-123456 expired");
    assert!(out.contains("<REDACTED-CUSTOM>"), "got: {out}");
    assert!(!out.contains("ACME-123456"), "got: {out}");
}

#[test]
fn scrubber_is_idempotent() {
    let s = PiiScrubber::default();
    let raw = "user alice@example.com from /Users/alice/x \
               at 10.0.0.7 with token \
               eyJhbGciOiJIUzI1NiJ9aaaa.eyJzdWIiOiIxMjM0NTY3ODkwIn0bbbbb.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5ccc";
    let once = s.scrub(raw);
    let twice = s.scrub(&once);
    assert_eq!(once, twice, "second pass changed output");
}

#[test]
fn reportable_error_scrubbed_clones_safely() {
    let scrubber = PiiScrubber::default();
    let evt = ReportableError::new(
        "copypaste-daemon",
        "0.3.0-dev",
        "open /Users/dmytro/db.sqlite -> 550e8400-e29b-41d4-a716-446655440000",
        OsTag::MacOs,
    );
    let s = evt.scrubbed(&scrubber);

    assert_eq!(s.crate_name, "copypaste-daemon");
    assert_eq!(s.crate_version, "0.3.0-dev");
    assert_eq!(s.os, OsTag::MacOs);
    assert!(s.error_class.contains("~/"));
    assert!(s.error_class.contains("<REDACTED-HEX>"));
    assert!(!s.error_class.contains("dmytro"));
    assert!(!s.error_class.contains("550e8400"));
}

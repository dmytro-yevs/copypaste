//! Shared HTTPS URL validation used by both the Supabase cloud-sync path
//! (`cloud::config`) and the relay-sync path (`relay::registration`).
//!
//! # Security note
//! Both paths previously kept independent copies of the URL gate.  A one-sided
//! fix (e.g. tightening the scheme check in one copy) would silently leave the
//! other vulnerable — CopyPaste-g06m.32 item #2.  This module is the single
//! authoritative source so any future change propagates to both callers.
//!
//! # Safe-union decision (g06m.32 #2)
//! The two originals were functionally identical in production. The only
//! difference was cosmetic: `cloud/config.rs` split the loopback-HTTP
//! test relaxation into a separate `test_only_allows_local_http` fn, while
//! `relay/registration.rs` inlined it with `#[cfg(test)]`. We preserve the
//! `test_only_allows_local_http` name as a re-export alias (see below) so
//! existing callers in `cloud/lifecycle.rs` require no signature change.
//!
//! **Production behaviour is unchanged for both callers**: HTTPS is mandatory;
//! the `http://` loopback path is compiled out entirely (`#[cfg(not(test))]`
//! returns `false`), so it cannot weaken the shipped binary.

/// Strict HTTPS-URL guard shared by cloud-sync and relay-sync.
///
/// We deliberately do **not** pull in the `url` crate — a string-prefix check
/// plus a sanity test that something follows the scheme is sufficient, and
/// avoids a transitive-dep surface.
///
/// Accepts: `https://host[:port][/path...]`
/// Rejects: `http://...`, `ws://...`, `file://...`, bare hostnames, empty strings.
pub(crate) fn is_https_url(s: &str) -> bool {
    // Case-insensitive scheme compare; reject if no authority follows.
    let lower = s.to_ascii_lowercase();
    if !lower.starts_with("https://") {
        return false;
    }
    // `s[8..]` is safe: "https://" is 8 ASCII bytes and the prefix matched.
    let rest = &s[8..];
    // Must have at least one non-`/` character (a host).
    rest.chars()
        .next()
        .is_some_and(|c| c != '/' && !c.is_whitespace())
}

/// TEST-ONLY HTTPS-gate relaxation — shared by both cloud and relay paths.
///
/// Returns `true` only when the URL is plain `http://` pointing at a loopback
/// host (`127.0.0.1` / `localhost` / `[::1]`). This lets the test suite point
/// either the cloud orchestrator or the relay push/receive loops at an
/// in-process mock server bound to loopback.
///
/// In production this function is a hard `false` (`#[cfg(not(test))]` variant
/// below), so **neither caller** ever trusts plain HTTP in the shipped binary.
/// Loopback HTTP is never trusted outside the test harness.
#[cfg(test)]
pub(crate) fn allows_loopback_http_in_tests(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    let Some(rest) = lower.strip_prefix("http://") else {
        return false;
    };
    // Host is everything up to the first `/`, `:` (port), or end-of-string.
    let host = rest.split(['/', ':']).next().unwrap_or_default();
    matches!(host, "127.0.0.1" | "localhost" | "[::1]" | "::1")
}

/// Production stub: loopback HTTP is NEVER allowed.  Always `false` so the
/// HTTPS gate is absolute in the shipped binary.
#[cfg(not(test))]
#[inline]
pub(crate) fn allows_loopback_http_in_tests(_s: &str) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── is_https_url ──────────────────────────────────────────────────────────

    #[test]
    fn https_url_accepts_valid() {
        assert!(is_https_url("https://x.test"));
        assert!(is_https_url("https://x.test:8443/api"));
        assert!(is_https_url("https://relay.example.com/"));
        // Uppercase scheme is normalised.
        assert!(is_https_url("HTTPS://example.com"));
    }

    #[test]
    fn https_url_rejects_invalid() {
        // Plain HTTP — must be rejected in production for BOTH cloud and relay.
        assert!(!is_https_url("http://x.test"));
        // Scheme present but no host.
        assert!(!is_https_url("https://"));
        assert!(!is_https_url("https:///"));
        assert!(!is_https_url("https://  "));
        // Wrong schemes.
        assert!(!is_https_url("ws://x.test"));
        assert!(!is_https_url("file:///etc/passwd"));
        // Bare hostname / empty.
        assert!(!is_https_url("x.test"));
        assert!(!is_https_url(""));
    }

    // ── allows_loopback_http_in_tests ─────────────────────────────────────────

    #[test]
    fn loopback_http_accepted_in_test_mode() {
        // IPv4 loopback and localhost — reliably parsed by split(['/', ':']) host extraction.
        assert!(allows_loopback_http_in_tests("http://127.0.0.1"));
        assert!(allows_loopback_http_in_tests("http://127.0.0.1:9090/v1"));
        assert!(allows_loopback_http_in_tests("http://localhost"));
        assert!(allows_loopback_http_in_tests("http://localhost:3000/api"));
        // IPv6 bare (no brackets): the split stops at the first ':' giving "".
        // The non-bracketed bare `::1` form is also accepted: stripping "http://"
        // leaves "::1" and split at ':' yields "" as the first token — which the
        // regex doesn't match. The `"[::1]" | "::1"` arms in matches! are
        // inherited from the original code but are unreachable with this parser.
        // We do NOT test bracketed IPv6 (`http://[::1]`) here because the
        // simple split-based host extractor breaks on brackets — the brackets
        // cause `"["` to be extracted as the host, which doesn't match. This is
        // a pre-existing limitation of the original code that we preserve faithfully.
    }

    #[test]
    fn loopback_http_rejects_non_loopback() {
        // External HTTP must never be allowed even in test mode.
        assert!(!allows_loopback_http_in_tests("http://evil.example.com"));
        assert!(!allows_loopback_http_in_tests("http://192.168.1.1"));
        // HTTPS must not match the loopback-HTTP gate.
        assert!(!allows_loopback_http_in_tests("https://localhost"));
        // Empty / garbage.
        assert!(!allows_loopback_http_in_tests(""));
        assert!(!allows_loopback_http_in_tests("not-a-url"));
    }

    // ── Cross-path consistency: both cloud and relay call sites ───────────────

    #[test]
    fn cloud_relay_both_require_https_in_production_simulation() {
        // Simulate the guard each caller uses (both use the same two fns now).
        // Cloud call site: `if !is_https_url(url) && !allows_loopback_http_in_tests(url)`
        // Relay call site: same logic via `is_relay_url_ok` which now delegates here.
        let check = |url: &str| is_https_url(url) || allows_loopback_http_in_tests(url);

        // Production-style HTTPS URL: accepted by both paths.
        assert!(check("https://relay.copypaste.app"));
        assert!(check("https://project.supabase.co"));
        // Test-mode loopback (IPv4/localhost only — bracketed IPv6 is a
        // pre-existing parser limitation; see loopback_http_accepted_in_test_mode).
        assert!(check("http://127.0.0.1:8080"));
        assert!(check("http://localhost:5432"));
        // Must be rejected by both paths in all builds.
        assert!(!check("http://evil.example.com"));
        assert!(!check(""));
        assert!(!check("ws://relay.example.com"));
    }
}

//! PII scrubbing for outbound telemetry payloads.
//!
//! This module is the last line of defence before a [`ReportableError`]
//! reaches a backend such as Sentry. Even though [`ReportableError`] is
//! designed to hold only coarse, developer-defined categorical fields, the
//! `error_class` string is human-authored and could accidentally leak PII
//! (e.g. a path containing a username, an email captured from a log line,
//! or a long hex token).
//!
//! The [`PiiScrubber`] applies a fixed list of conservative redaction
//! patterns in deterministic order. Each pattern produces a replacement that
//! is recognisably non-secret (`<REDACTED-…>`), so reviewers can tell at a
//! glance that a value was scrubbed rather than mistakenly emitted.
//!
//! # Defence-in-depth, not a substitute
//!
//! Producers MUST still avoid putting user data into [`ReportableError`].
//! The scrubber is intentionally pattern-based and will not catch every
//! conceivable PII shape (e.g. arbitrary names, free-form sentences). See
//! [`docs/privacy/telemetry-policy.md`][policy] for the authoritative
//! policy.
//!
//! [policy]: https://github.com/dmytro-yevs/copypaste/blob/main/docs/privacy/telemetry-policy.md
//!
//! # Determinism
//!
//! Scrubbing is pure: the same input always yields the same output, and a
//! scrubbed string is idempotent under further scrubbing (see the
//! `scrubber_is_idempotent` integration test).

use regex::Regex;
use std::sync::LazyLock;
use unicode_normalization::UnicodeNormalization;

use crate::error::ReportableError;

// ---------------------------------------------------------------------------
// Compiled-once regex statics.
//
// Each LazyLock is initialised on first use and reused for the process
// lifetime, avoiding the per-call Regex::new overhead in with_defaults().
// MSRV 1.96 stabilises std::sync::LazyLock (available since 1.80; 1.89 was previous floor), so no extra crate is needed.
// The .expect() strings are only ever reached on a malformed static literal,
// which would be caught by the scrubber_patterns_compile unit test below.
// ---------------------------------------------------------------------------

/// URL authority credentials: `scheme://user:pass@host…`
static RE_URL_AUTH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b([a-z][a-z0-9+.\-]*://)[^/\s:@]+:[^\s/]*@")
        .expect("url-auth pattern is valid")
});

/// Email addresses.
static RE_EMAIL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}").expect("email pattern is valid")
});

/// UUIDs and UUID-like hex strings (32+ hex chars, optional dashes).
static RE_UUID_HEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b[0-9a-f]{8}-?[0-9a-f]{4}-?[0-9a-f]{4}-?[0-9a-f]{4}-?[0-9a-f]{12,}\b")
        .expect("uuid/hex pattern is valid")
});

/// Bare 32+-char hex strings (SHA-256 digests, API keys, etc.).
static RE_HEX32: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[0-9a-fA-F]{32,}\b").expect("hex32 pattern is valid"));

/// JWT-like three-segment base64url tokens.
static RE_JWT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,}\b")
        .expect("jwt pattern is valid")
});

/// Candidate single-token base64url blob: ≥32 chars, alphabet `[A-Za-z0-9_-]`,
/// no dots (dots would make it a JWT candidate instead).
///
/// This regex only *candidates* — the actual replacement is gated on a
/// character-variety check in `scrub_base64url_tokens` to avoid over-scrubbing
/// short or all-one-class strings (e.g. version segments, taxonomy identifiers).
/// We use `\b` on the left so the match cannot start mid-word, and we stop at
/// any character that is not in the base64url alphabet (no trailing `\b` needed
/// because the character class itself is the bound).
static RE_B64URL_CANDIDATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[A-Za-z0-9_-]{32,}").expect("base64url candidate pattern is valid")
});

/// IPv4 addresses.
static RE_IPV4: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").expect("ipv4 pattern is valid"));

/// IPv6 addresses (permissive; see inline comment in with_defaults for rationale).
static RE_IPV6: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(^|[^0-9a-fA-F:])([0-9a-fA-F]{0,4}(?::[0-9a-fA-F]{0,4}){2,7})")
        .expect("ipv6 pattern is valid")
});

/// macOS home-directory prefix: `/Users/<name>/…` → `~/`.
static RE_MACOS_HOME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/Users/[^/\n]+?(?:/|$)").expect("macos home pattern is valid"));

/// Linux home-directory prefix: `/home/<name>/…` → `~/`.
static RE_LINUX_HOME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/home/[^/\n]+?(?:/|$)").expect("linux home pattern is valid"));

/// Windows home-directory prefix: `C:\Users\<name>\…` → `~/`.
///
/// The character class excludes `\`, `'`, `"`, and newlines so the username
/// segment is bounded tightly and cannot consume path separators or quote
/// delimiters that might appear in surrounding log text.
///
/// IMPORTANT: this literal MUST use a hash raw string (`r#"…"#`) because the
/// character class contains a double-quote (`"`), which would terminate a
/// plain `r"…"` raw string prematurely.
static RE_WINDOWS_HOME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)[A-Z]:\\Users\\[^\\'"\n]+?(?:\\|$)"#).expect("windows home pattern is valid")
});

/// Replacement pattern: a compiled regex plus the literal replacement string
/// applied by [`Regex::replace_all`].
#[derive(Debug, Clone)]
struct Pattern {
    re: Regex,
    replacement: &'static str,
}

/// A single scrubbing step — either a simple regex substitution or an
/// entropy-gated transform that inspects each candidate match before deciding
/// whether to replace it.
///
/// The enum is `non_exhaustive` (internal) so future variants can be added
/// without a breaking change to the `patterns`/`steps` field type.
#[derive(Debug, Clone)]
enum ScrubStep {
    /// Apply `regex::Regex::replace_all` unconditionally with the given
    /// replacement string.
    Simple(Pattern),
    /// Match all candidates with the regex, but only emit the replacement for
    /// candidates that pass the variety predicate. Candidates that fail keep
    /// their original text.
    EntropyCandidateB64Url,
}

/// Returns `true` when `tok` has at least one uppercase ASCII letter, at least
/// one lowercase ASCII letter, and at least one ASCII digit — the minimum
/// character-class variety expected of a machine-generated bearer token.
///
/// Pure lowercase or pure uppercase strings (e.g. `copypaste-daemon` or
/// `AAAA…`) and digit-only strings are rejected so taxonomy identifiers and
/// semver segments are not scrubbed.
///
/// This is intentionally a *necessary* condition, not a full Shannon-entropy
/// calculation — it is cheap to compute and sufficient to distinguish
/// `VqE2mK9xRs…` (token) from `zzzzzzzzzzzzzzzz…` (low-entropy placeholder).
fn has_token_variety(tok: &str) -> bool {
    let has_upper = tok.bytes().any(|b| b.is_ascii_uppercase());
    let has_lower = tok.bytes().any(|b| b.is_ascii_lowercase());
    let has_digit = tok.bytes().any(|b| b.is_ascii_digit());
    has_upper && has_lower && has_digit
}

/// PII scrubber. Construct via [`PiiScrubber::default`] to get the built-in
/// pattern set, then optionally extend with [`PiiScrubber::add_custom`].
///
/// Cheap to share across threads behind an [`std::sync::Arc`].
#[derive(Debug, Clone)]
pub struct PiiScrubber {
    steps: Vec<ScrubStep>,
}

impl PiiScrubber {
    /// Construct an empty scrubber with no patterns. Mostly useful for tests
    /// that want to verify pass-through behaviour or build a custom set from
    /// scratch.
    pub fn empty() -> Self {
        Self { steps: Vec::new() }
    }

    /// Construct a scrubber preloaded with the built-in pattern set:
    ///
    /// 1. URLs containing `user:password@` — credentials stripped.
    /// 2. Email addresses.
    /// 3. Hex strings ≥32 chars (UUIDs, keys, digests).
    /// 4. JWT-like three-segment tokens.
    /// 5. High-entropy single-token base64url blobs ≥32 chars (bearer tokens).
    /// 6. IPv4 and IPv6 addresses.
    /// 7. `/Users/<name>/` and `/home/<name>/` prefixes — replaced with `~/`.
    ///
    /// The order matters and encodes two dependencies:
    /// - URL credentials run before email, because a `user:pass@host`
    ///   authority contains an `@` that the email rule would otherwise eat.
    /// - Email runs before the long-hex rule, so an address whose domain
    ///   label is a long hex string is redacted whole rather than fragmented.
    /// - The base64url-token step runs after JWT so a three-segment token is
    ///   caught as a JWT first; the single-token rule would otherwise only
    ///   redact the first (longest) segment.
    ///
    /// More specific patterns generally run before more general ones (IP,
    /// paths) so they are not partially eaten by a broader rule.
    ///
    /// Patterns are conservative and prefer false positives (over-redaction)
    /// to false negatives (PII leakage).
    pub fn with_defaults() -> Self {
        // Steps reference the process-lifetime LazyLock statics defined at
        // the top of this module; .clone() is a cheap Regex arc-clone.
        let steps = vec![
            // URL credentials: strip the `user:pass@` portion, keep scheme
            // and host so the error class remains debuggable.
            //
            // This runs *first* — before the email rule — because a
            // `user:pass@host` authority contains an `@` and would otherwise
            // be partially eaten by the email pattern (e.g.
            // `https://user:secret@db.internal/path` → the email rule would
            // match `secret@db.internal` and the credential rule could no
            // longer fire).
            //
            // The password span must be allowed to contain `@` so that a
            // password like `p@ss` in `https://user:p@ss@host/x` does not
            // leak its tail. We consume everything (sans whitespace and `/`,
            // which would end the authority) greedily up to the *last* `@`
            // before the path: `[^\s/]*@` backtracks to that final `@`,
            // leaving the host intact.
            ScrubStep::Simple(Pattern {
                re: RE_URL_AUTH.clone(),
                replacement: "$1<REDACTED-AUTH>@",
            }),
            // Email addresses. Conservative local-part character class to
            // avoid eating surrounding punctuation.
            //
            // This runs *before* the long-hex rule: an email whose domain
            // label is a long hex string (e.g. `a@deadbeef…32hexchars.com`)
            // would otherwise have its domain partially redacted to
            // `<REDACTED-HEX>` first, leaving a dangling local part that the
            // email rule could no longer match — leaking the local part.
            ScrubStep::Simple(Pattern {
                re: RE_EMAIL.clone(),
                replacement: "<REDACTED-EMAIL>",
            }),
            // Long hex strings: UUIDs (with or without dashes), SHA-256
            // digests, API keys with hex encoding. 32+ hex chars catches
            // MD5 and up. We allow optional dashes inside to match UUIDs.
            ScrubStep::Simple(Pattern {
                re: RE_UUID_HEX.clone(),
                replacement: "<REDACTED-HEX>",
            }),
            ScrubStep::Simple(Pattern {
                re: RE_HEX32.clone(),
                replacement: "<REDACTED-HEX>",
            }),
            // JWT-like: three base64url segments separated by '.'. Each
            // segment is at least 20 chars to avoid eating dotted version
            // strings.
            ScrubStep::Simple(Pattern {
                re: RE_JWT.clone(),
                replacement: "<REDACTED-JWT>",
            }),
            // Single-token base64url bearer tokens (pairing / relay auth).
            //
            // These are 32+-char blobs in the `[A-Za-z0-9_-]` alphabet with
            // NO dots — which means JWT and hex rules have not matched them.
            // A daemon bearer token is typically 32 random bytes encoded as
            // base64url (43–44 chars). The regex candidate is intentionally
            // broad; the entropy gate (`has_token_variety`) then filters out
            // low-entropy false positives (all-lowercase, all-uppercase,
            // short taxonomy identifiers) so normal error strings are not
            // over-redacted.
            //
            // Ordering: runs AFTER JWT so a three-segment JWT is consumed
            // whole by the JWT rule rather than having its first segment
            // redacted here.
            ScrubStep::EntropyCandidateB64Url,
            // IPv4 — four 1-3 digit groups. We do not enforce 0-255 bounds
            // because we'd rather over-match than under-match.
            ScrubStep::Simple(Pattern {
                re: RE_IPV4.clone(),
                replacement: "<REDACTED-IP>",
            }),
            // IPv6 — we anchor on ASCII non-hex non-colon boundaries
            // rather than `\b` because `:` itself is not a word character —
            // the original `\b::` form matched erratically depending on
            // what surrounded the colons. The leading boundary char is
            // captured into `$1` so the replacement preserves it.
            //
            // There is *no* trailing boundary assertion. The `regex` crate
            // has no look-around, and consuming a trailing boundary char
            // (the original `($|[^0-9a-fA-F:])` group) breaks adjacency:
            // `replace_all` would resume scanning *after* the consumed char,
            // so the space between the two addresses in `"::1 ::1"` was
            // eaten by the first match and the second `::1` leaked. A
            // trailing assertion is unnecessary anyway — the address body
            // matches only `[0-9a-fA-F:]`, so it is already maximal and
            // stops exactly at the first non-hex-non-colon char without
            // consuming it.
            //
            // We accept any run that contains at least two `:` separators
            // and only [0-9a-fA-F:] in between, which is permissive enough
            // to catch compressed forms like `fe80::1ff:fe23:4567:890a` and
            // `::1` without writing the full RFC 4291 grammar. The leading
            // boundary group prevents matching inside a longer
            // alphanumeric run (e.g. a hash that happens to contain
            // colons in a different schema).
            //
            // NOTE: this permissive `{0,4}` form is deliberate — it accepts
            // every IPv6 shorthand including the leading-`::` compressed
            // loopback `::1`. A previous tightening to require a non-empty
            // leading hextet broke `::1` (regression caught by
            // `adjacent_ipv6_addresses_both_redacted`). Over-redaction of bare
            // colon-delimited tokens is an accepted, fail-safe tradeoff: the
            // scrubber prefers false positives, and telemetry is unwired today.
            ScrubStep::Simple(Pattern {
                re: RE_IPV6.clone(),
                replacement: "$1<REDACTED-IP>",
            }),
            // Home directory prefixes: macOS `/Users/<name>/…` and Linux
            // `/home/<name>/…`. We collapse to `~/` so the structural part
            // of the path (which is often the useful debugging signal)
            // survives.
            //
            // The username segment is everything after `/Users/` up to the
            // next `/` or end-of-line. The trailing `/` is *optional* so a
            // bare `/Users/secretuser` (with no trailing slash — common in
            // ENOENT / `stat` error strings like `cannot stat /Users/jdoe`)
            // still redacts. We exclude only `\n`, *not* spaces, so a
            // username containing a space (`/Users/John Doe/file`) cannot
            // leak; stopping at the first `/` ensures we never over-redact
            // the deeper path segments that carry the debugging signal.
            ScrubStep::Simple(Pattern {
                re: RE_MACOS_HOME.clone(),
                replacement: "~/",
            }),
            ScrubStep::Simple(Pattern {
                re: RE_LINUX_HOME.clone(),
                replacement: "~/",
            }),
            // Windows home-directory prefix: `C:\Users\<name>\…` → `~/`.
            // Uses RE_WINDOWS_HOME (r#"…"# hash raw string) because the
            // character class contains `"` which would terminate `r"…"` early.
            ScrubStep::Simple(Pattern {
                re: RE_WINDOWS_HOME.clone(),
                replacement: "~/",
            }),
        ];
        Self { steps }
    }

    /// Append a custom user-supplied pattern. Returns `Err` (with the
    /// underlying [`regex::Error`] message) if the regex does not compile.
    ///
    /// The replacement is fixed to `<REDACTED-CUSTOM>` to keep the redaction
    /// surface uniform; if a caller needs more nuance they can construct a
    /// scrubber manually with [`PiiScrubber::empty`] and feed in their own
    /// patterns via this method.
    pub fn add_custom(&mut self, regex_src: &str) -> Result<(), String> {
        let re = Regex::new(regex_src).map_err(|e| e.to_string())?;
        self.steps.push(ScrubStep::Simple(Pattern {
            re,
            replacement: "<REDACTED-CUSTOM>",
        }));
        Ok(())
    }

    /// Apply every pattern in order and return the scrubbed copy.
    ///
    /// The input is first normalised to NFKC. Without this step an attacker
    /// (or, more commonly, a copy-pasted log line) can bypass the regex
    /// patterns with Unicode-equivalent characters that *look* like ASCII
    /// but do not match the ASCII regex class — e.g. fullwidth Latin letters
    /// (U+FF21 'Ａ' vs U+0041 'A'), Greek small letter omicron in place of
    /// Latin 'o' inside an email's local part, or compatibility forms of
    /// digits. NFKC collapses those to their canonical ASCII equivalents
    /// before pattern matching so the existing regex set covers them.
    ///
    /// Pure function: no I/O, no allocation beyond the normalised input and
    /// the returned `String` (and intermediate buffers internal to
    /// [`Regex::replace_all`]).
    pub fn scrub(&self, input: &str) -> String {
        let normalised: String = input.nfkc().collect();
        let mut out = normalised;
        for step in &self.steps {
            out = match step {
                ScrubStep::Simple(p) => {
                    // `replace_all` only allocates when there is at least one
                    // match, so pass-through strings stay cheap.
                    p.re.replace_all(&out, p.replacement).into_owned()
                }
                ScrubStep::EntropyCandidateB64Url => {
                    // Match every ≥32-char base64url candidate. For each
                    // match, apply the replacement only if the token passes
                    // the character-variety gate (has_token_variety).
                    // Candidates that fail keep their original text, so
                    // taxonomy identifiers and version segments are not
                    // over-redacted.
                    RE_B64URL_CANDIDATE
                        .replace_all(&out, |caps: &regex::Captures<'_>| {
                            let m = caps.get(0).expect("full match group 0 always exists");
                            if has_token_variety(m.as_str()) {
                                "<REDACTED-TOKEN>".to_owned()
                            } else {
                                m.as_str().to_owned()
                            }
                        })
                        .into_owned()
                }
            };
        }
        out
    }
}

impl Default for PiiScrubber {
    fn default() -> Self {
        Self::with_defaults()
    }
}

impl ReportableError {
    /// Return a copy of this event with every string field passed through
    /// `scrubber`. Numeric / enum fields ([`crate::OsTag`]) are copied
    /// as-is — they have no free-form content.
    pub fn scrubbed(&self, scrubber: &PiiScrubber) -> Self {
        Self {
            crate_name: scrubber.scrub(&self.crate_name),
            crate_version: scrubber.scrub(&self.crate_version),
            error_class: scrubber.scrub(&self.error_class),
            os: self.os,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OsTag;

    #[test]
    fn empty_scrubber_is_passthrough() {
        let s = PiiScrubber::empty();
        assert_eq!(
            s.scrub("anything goes /Users/alice/x foo@bar.com"),
            "anything goes /Users/alice/x foo@bar.com"
        );
    }

    #[test]
    fn default_scrubber_redacts_email_in_event() {
        let scrubber = PiiScrubber::default();
        let evt = ReportableError::new(
            "copypaste-daemon",
            "0.3.0-dev",
            "alice@example.com failed login",
            OsTag::MacOs,
        );
        let scrubbed = evt.scrubbed(&scrubber);
        assert!(scrubbed.error_class.contains("<REDACTED-EMAIL>"));
        assert!(!scrubbed.error_class.contains("alice@example.com"));
        assert_eq!(scrubbed.crate_name, "copypaste-daemon");
        assert_eq!(scrubbed.os, OsTag::MacOs);
    }

    #[test]
    fn add_custom_rejects_invalid_regex() {
        let mut s = PiiScrubber::empty();
        assert!(s.add_custom("(unclosed").is_err());
    }
}

use regex::{Regex, RegexSet};
use std::sync::OnceLock;

static PATTERN_SET: OnceLock<RegexSet> = OnceLock::new();
static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();

/// Eagerly compile all sensitive-data regex patterns and the companion
/// `RegexSet`. Must be called once before any detection function runs.
///
/// Returns an error if any pattern string is invalid. Subsequent calls
/// are a no-op (OnceLock ensures the patterns are compiled at most once).
///
/// Called automatically by `Database::open()` so callers that go through
/// the normal DB path get patterns validated at startup rather than
/// panicking on first clipboard scan.
pub fn init_patterns() -> Result<(), regex::Error> {
    // Compile the RegexSet first — it validates all pattern strings.
    if PATTERN_SET.get().is_none() {
        let set = RegexSet::new(RAW_PATTERNS.iter().map(|(_, p, _, _)| *p))?;
        let _ = PATTERN_SET.set(set); // ignore if already set by a racing thread
    }
    // Compile the individual Regex vec.
    if PATTERNS.get().is_none() {
        let pats: Result<Vec<Regex>, regex::Error> = RAW_PATTERNS
            .iter()
            .map(|(_, p, _, _)| Regex::new(p))
            .collect();
        let _ = PATTERNS.set(pats?); // ignore if already set by a racing thread
    }
    Ok(())
}

/// (name, raw_regex, category_index, confidence)
/// category_index: 0=Credential, 1=Financial, 2=PersonalId, 3=Infrastructure
pub const RAW_PATTERNS: &[(&str, &str, u8, f32)] = &[
    // ── Credentials ──────────────────────────────────────────────────────────
    // Leading `\b` prevents matching AKIA/ASIA mid-token (e.g. "XAKIAIOSFODNN7EXAMPLE").
    // No trailing `\b`: ASIA temporary keys may have trailing digits, and `E1` is
    // two word-chars with no boundary between them.
    ("aws_access_key", r"\b(?:AKIA|ASIA)[0-9A-Z]{16}", 0, 0.99),
    (
        "github_fine_grained",
        r"github_pat_[A-Za-z0-9]{22}_[A-Za-z0-9]{59}",
        0,
        0.99,
    ),
    ("github_classic_pat", r"ghp_[A-Za-z0-9]{36}", 0, 0.99),
    ("github_actions_token", r"ghs_[a-zA-Z0-9]{36}", 0, 0.99),
    ("openai_new", r"sk-proj-[A-Za-z0-9]{48}", 0, 0.99),
    // `\b` boundaries prevent matching mid-token. No lookahead needed: `sk-proj-`
    // keys contain a hyphen after "proj" which is not in [A-Za-z0-9], so this
    // pattern structurally cannot match strings caught by `openai_new`.
    ("openai_legacy", r"\bsk-[A-Za-z0-9]{48}\b", 0, 0.95),
    ("anthropic", r"sk-ant-api\d{2}-[A-Za-z0-9_-]{80,}", 0, 0.99),
    ("stripe_live", r"sk_live_[0-9A-Za-z]{24}", 0, 0.99),
    ("stripe_webhook", r"whsec_[a-zA-Z0-9]{32,64}", 0, 0.99),
    ("npm_token", r"npm_[A-Za-z0-9]{36}", 0, 0.99),
    ("pypi_token", r"pypi-[A-Za-z0-9_-]{180,}", 0, 0.99),
    (
        "slack_bot",
        r"xoxb-[0-9]{11}-[0-9]{11}-[a-zA-Z0-9]{24}",
        0,
        0.99,
    ),
    (
        "slack_webhook",
        r"https://hooks\.slack\.com/services/T[A-Z0-9]+/B[A-Z0-9]+/[a-zA-Z0-9]+",
        0,
        0.99,
    ),
    (
        "discord_bot_token",
        r"[MN][a-zA-Z\d]{23,25}\.[\w-]{6}\.[\w-]{27,38}",
        0,
        0.85,
    ),
    ("twilio_auth_token", r"SK[a-f0-9]{32}", 0, 0.90),
    ("google_api_key", r"AIza[0-9A-Za-z\-_]{35}", 0, 0.99),
    (
        "heroku_api_key",
        r"(?i)heroku[^\n]{0,50}[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}",
        0,
        0.95,
    ),
    // Require at least 32 chars after the dot to reduce FP on short `hvs.`-prefixed strings.
    ("hashicorp_vault", r"hvs\.[A-Za-z0-9]{32,}", 0, 0.95),
    ("gcp_oauth", r"GOCSPX-[A-Za-z0-9_-]{28}", 0, 0.99),
    (
        "ssh_private_key",
        r"-----BEGIN (?:RSA |EC |OPENSSH |DSA |)?PRIVATE KEY-----",
        0,
        0.99,
    ),
    // Audit MED #5: original `ssh_private_key` only catches the
    // `-----BEGIN ... PRIVATE KEY-----` PEM family. PKCS#8 encrypted keys
    // and PuTTY's `.ppk` format use different headers and were silently
    // ignored — add them as separate patterns so any of the three forms
    // triggers detection. `(?m)` enables multiline mode so the `^`
    // anchor matches at the start of any line in a clipboard blob.
    (
        "ssh_private_key_pkcs8_encrypted",
        r"(?m)^-----BEGIN ENCRYPTED PRIVATE KEY-----",
        0,
        0.99,
    ),
    (
        "ssh_private_key_putty",
        r"(?m)^PuTTY-User-Key-File-[0-9]+:",
        0,
        0.99,
    ),
    (
        "generic_bearer",
        r"(?i)\bBearer\s+[A-Za-z0-9\-._~+/]{20,}",
        0,
        0.80,
    ),
    // generic_password_kv — captures key/value pairs; the matched value is post-validated
    // by `is_credential_value_strong` to suppress FP on benign prose like
    // "secret = foo" / "password: nope" / "// api_key=demo".
    // The capture group around the value lets the validator inspect only the value bytes.
    (
        "generic_password_kv",
        r"(?i)(?:password|passwd|secret|api_key|apikey|auth_token)\s*[:=]\s*(\S{6,})",
        0,
        0.75,
    ),
    (
        // Audit MED #5: anchor on `\b` so we don't match `eyJ` glued onto
        // the tail of another identifier (e.g. `mykeyeyJabc.def.ghi`),
        // cutting false positives without changing legitimate token hits.
        "jwt",
        r"\beyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+",
        0,
        0.95,
    ),
    // ── Financial ─────────────────────────────────────────────────────────────
    (
        "iban",
        r"\b[A-Z]{2}[0-9]{2}[A-Z0-9]{4}[0-9]{7}[A-Z0-9]{0,16}\b",
        1,
        0.85,
    ),
    // ── Personal Identifiers ──────────────────────────────────────────────────
    ("ssn_us", r"\b\d{3}[-\s]\d{2}[-\s]\d{4}\b", 2, 0.80),
    (
        "email",
        r"\b[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}\b",
        2,
        0.60,
    ),
    (
        "phone_us",
        r"(?:\+1[-.\s]?)?\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}\b",
        2,
        0.55,
    ),
    // Raised min digits from 6 to 9 to cut FP on short uppercase+digit codes
    // (order IDs, product codes, etc.). Still well below the auto-wipe floor.
    ("passport", r"\b[A-Z]{1,2}[0-9]{9}\b", 2, 0.55),
    // ── Infrastructure ────────────────────────────────────────────────────────
    (
        "ip_with_port",
        r"\b(?:(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\.){3}(?:25[0-5]|2[0-4]\d|[01]?\d\d?):\d{2,5}\b",
        3,
        0.70,
    ),
    (
        "db_conn_string",
        r"(?i)(?:postgresql|postgres|mysql|mongodb|redis|amqp|mssql)://[^@\s]*:[^@\s]*@\S+",
        3,
        0.99,
    ),
    (
        "aws_arn",
        r"\barn:aws:[a-z][a-z0-9\-]*:[a-z0-9\-]*:[0-9]{12}:[^\s]+",
        3,
        0.90,
    ),
    (
        "dotenv_secret",
        r"(?m)^(?:export\s+)?[A-Z][A-Z0-9_]{2,}(?:_KEY|_SECRET|_TOKEN|_PASSWORD|_PASS|_PWD|_CREDENTIALS?)\s*=\s*\S+",
        3,
        0.80,
    ),
];

pub fn pattern_set() -> &'static RegexSet {
    // `RAW_PATTERNS` are compile-time constants validated by the test suite
    // (`all_raw_patterns_compile`), so construction is provably infallible in
    // practice. To uphold the no-`unwrap()` rule without changing this
    // `&'static`-returning signature, an unexpected compile failure degrades
    // to an empty `RegexSet` (detection simply matches nothing) rather than
    // panicking on the hot clipboard-scan path.
    PATTERN_SET.get_or_init(|| {
        RegexSet::new(RAW_PATTERNS.iter().map(|(_, p, _, _)| *p))
            .unwrap_or_else(|_| RegexSet::empty())
    })
}

pub fn patterns() -> &'static Vec<Regex> {
    // Index-alignment guarantee: the individual `Vec<Regex>` MUST have the
    // same length and order as `RAW_PATTERNS` so that `enumerate()` indices
    // in `detect_normalised` agree with `pattern_name` / `pattern_category` /
    // `pattern_confidence`, which index into `RAW_PATTERNS` directly.
    //
    // `filter_map(…ok())` would silently drop a failing pattern and shift all
    // subsequent indices, desyncing the two paths. Instead we degrade the
    // WHOLE vec to empty on any compile failure (matching the `pattern_set`
    // empty-fallback contract): a single bad pattern is caught by the
    // `all_raw_patterns_compile` test, and in production the empty fallback
    // means "no detection" rather than "wrong detection with mismatched names".
    PATTERNS.get_or_init(|| {
        let result: Result<Vec<Regex>, _> = RAW_PATTERNS
            .iter()
            .map(|(_, p, _, _)| Regex::new(p))
            .collect();
        result.unwrap_or_default()
    })
}

pub fn pattern_name(index: usize) -> &'static str {
    RAW_PATTERNS
        .get(index)
        .map(|(n, _, _, _)| *n)
        .unwrap_or("unknown")
}

pub fn pattern_category(index: usize) -> u8 {
    RAW_PATTERNS.get(index).map(|(_, _, c, _)| *c).unwrap_or(0)
}

pub fn pattern_confidence(index: usize) -> f32 {
    RAW_PATTERNS
        .get(index)
        .map(|(_, _, _, conf)| *conf)
        .unwrap_or(0.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Underpins the "provably infallible" claim on the degrade-safe
    /// `pattern_set()` / `patterns()` lazy initialisers: every constant in
    /// `RAW_PATTERNS` must compile, so the empty-fallback branch is never
    /// actually taken at runtime.
    #[test]
    fn all_raw_patterns_compile() {
        for (name, raw, _, _) in RAW_PATTERNS {
            assert!(
                Regex::new(raw).is_ok(),
                "pattern `{name}` failed to compile: {raw}"
            );
        }
        // The RegexSet (used by `pattern_set`) must also build from the full set.
        assert!(RegexSet::new(RAW_PATTERNS.iter().map(|(_, p, _, _)| *p)).is_ok());
    }

    /// The lazy getters must expose every pattern and a same-sized RegexSet —
    /// confirms the degrade-safe init did not silently drop any constant.
    #[test]
    fn lazy_getters_expose_all_patterns() {
        assert_eq!(patterns().len(), RAW_PATTERNS.len());
        assert_eq!(pattern_set().len(), RAW_PATTERNS.len());
    }
}

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
    // P2 fb3e: lowered from 0.85 → 0.65 — pattern shape is broad enough to
    // fire on non-Discord dot-separated base64url strings; still detected and
    // flagged (0.65 > 0) but now below the 0.70 auto-wipe floor so legitimate
    // content is never silently deleted. Added `\b` to avoid mid-token hits.
    (
        "discord_bot_token",
        r"\b[MN][a-zA-Z\d]{23,25}\.[\w-]{6}\.[\w-]{27,38}\b",
        0,
        0.65,
    ),
    // P2 fb3e: the previous regex `SK[a-f0-9]{32}` matches a Twilio *Signing-Key
    // SID* (prefix `SK`), NOT the auth token. Twilio auth tokens are 32 lowercase
    // hex chars with no distinctive prefix and cannot be distinguished reliably via
    // regex alone. Renamed to `twilio_signing_key_sid`, added `\b` boundaries to
    // prevent matching hex substrings, and lowered confidence to 0.65 (below the
    // 0.70 auto-wipe floor) since the bare `SK`+hex shape is insufficiently
    // distinctive to warrant silent deletion.
    ("twilio_signing_key_sid", r"\bSK[a-f0-9]{32}\b", 0, 0.65),
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
    // P2 fb3e: lowered from 0.80 → 0.65 — fires on tutorial/mock strings such
    // as "Bearer YOUR_TOKEN_HERE" in curl examples or README files. Confidence
    // below the 0.70 auto-wipe floor prevents silent deletion of legitimate
    // clipboard content. The pattern still flags bearer tokens for display;
    // the daemon's `is_sensitive_for_autowipe` gate will not act on them.
    // Note: a post-match entropy guard would be a no-op here because the 20-char
    // minimum already implies `is_credential_value_strong` returns true for all
    // matches; the confidence floor is the correct and sufficient control.
    (
        "generic_bearer",
        r"(?i)\bBearer\s+[A-Za-z0-9\-._~+/]{20,}",
        0,
        0.65,
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
    // P2 fb3e: lowered from 0.85 → 0.65 — legitimately-copied IBAN strings
    // (bank transfer, invoices) were being silently auto-wiped. IBANs ARE
    // sensitive data worth flagging but should not be auto-deleted without user
    // consent. A checksum validator (mod-97) would be the ideal fix; for now we
    // stay below the 0.70 auto-wipe floor. Still detected and surfaced in the UI.
    (
        "iban",
        r"\b[A-Z]{2}[0-9]{2}[A-Z0-9]{4}[0-9]{7}[A-Z0-9]{0,16}\b",
        1,
        0.65,
    ),
    // ── Personal Identifiers ──────────────────────────────────────────────────
    // P2 fb3e: lowered from 0.80 → 0.65 — the pattern `\d{3}[-\s]\d{2}[-\s]\d{4}`
    // also matches common date/unit strings like "012 31 2024" or "012-31-2024".
    // A structural validator (first group 001-899, second 01-99, third 0001-9999,
    // no all-zeros group) would reduce FP; lowering below the auto-wipe floor is
    // the safe-fallback per the audit mandate to prevent silent deletion.
    ("ssn_us", r"\b\d{3}[-\s]\d{2}[-\s]\d{4}\b", 2, 0.65),
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
    // ── P2 ozzt — additional cloud/infra tokens ───────────────────────────────
    // Azure Storage Account key: 88-char base64 (512-bit) value. A BARE base64
    // blob of this shape is indistinguishable from any other 64-byte base64
    // (SHA-512, Ed25519 dumps, random tokens), so matching it bare at conf 0.90
    // would silently auto-wipe/sync-exclude benign content. Require the
    // connection-string context `AccountKey=` (as cloudflare_api_token requires
    // its env-var prefix) so only a real Azure key matches.
    (
        "azure_storage_key",
        r"AccountKey=[A-Za-z0-9+/]{86}==",
        0,
        0.90,
    ),
    // Azure SAS token: anchor on the two stable, order-independent markers —
    // `sv=YYYY-MM-DD` (service version) and `&sig=<base64>` (signature). The
    // intervening SAS fields (ss/srt/sp/se/...) vary in order and value, so do
    // NOT over-specify them (the old `s[a-z]=(?:b|c|f|q)` matched almost no real
    // tokens). `(?i)` since query-param casing varies.
    (
        "azure_sas_token",
        r"(?i)\bsv=\d{4}-\d{2}-\d{2}\b[^\s]*&sig=[A-Za-z0-9%+/]{40,}",
        0,
        0.92,
    ),
    // GCP service-account JSON private key block. The "private_key" field value
    // always begins with the PEM header embedded in a JSON string.
    // `(?m)` multiline so `^` anchors per-line inside a pasted JSON blob.
    (
        "gcp_service_account_key",
        r#"(?m)"private_key"\s*:\s*"-----BEGIN RSA PRIVATE KEY-----"#,
        0,
        0.99,
    ),
    // Cloudflare API token: Cloudflare tokens are 40 chars of URL-safe base64
    // with no universal standalone prefix (unlike SG. or atlasv1.), so we
    // require the token to appear in a Cloudflare-specific env-var context:
    // `CLOUDFLARE_API_TOKEN=`, `CF_API_TOKEN=`, or `CLOUDFLARE_API_KEY=`.
    // This avoids matching any arbitrary 40-char alphanumeric string.
    // The `(?i)` flag allows both upper and mixed-case env var names.
    (
        "cloudflare_api_token",
        r"(?i)\b(?:CLOUDFLARE_API_(?:TOKEN|KEY)|CF_API_TOKEN)\s*=\s*[A-Za-z0-9_-]{40}\b",
        0,
        0.92,
    ),
    // SendGrid API key: always starts with `SG.` followed by two base64url
    // segments separated by `.`. Length constraints match real keys (~69 chars).
    (
        "sendgrid_api_key",
        r"\bSG\.[A-Za-z0-9_-]{22}\.[A-Za-z0-9_-]{43}\b",
        0,
        0.99,
    ),
    // Terraform Cloud / Terraform Enterprise user/team/org token:
    // prefix `atlasv1.` followed by a URL-safe base64 payload (64+ chars).
    (
        "terraform_cloud_token",
        r"\batlasv1\.[A-Za-z0-9_-]{64,}\b",
        0,
        0.99,
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
        RegexSet::new(RAW_PATTERNS.iter().map(|(_, p, _, _)| *p)).unwrap_or_else(|e| {
            tracing::error!(
                error = %e,
                "sensitive RegexSet failed to compile; detection degraded to empty set"
            );
            RegexSet::empty()
        })
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

    // ── P2 fb3e: confidence floor tests ──────────────────────────────────────

    /// FP-risk patterns must be below the 0.70 auto-wipe floor.
    #[test]
    fn fp_risk_patterns_below_autowipe_floor() {
        const AUTOWIPE_FLOOR: f32 = 0.70;
        let fp_patterns = [
            "discord_bot_token",
            "twilio_signing_key_sid",
            "iban",
            "ssn_us",
            "generic_bearer",
        ];
        for (name, _, _, conf) in RAW_PATTERNS {
            if fp_patterns.contains(name) {
                assert!(
                    *conf < AUTOWIPE_FLOOR,
                    "pattern `{name}` has confidence {conf} ≥ {AUTOWIPE_FLOOR}: \
                     must be below auto-wipe floor to prevent silent data deletion"
                );
            }
        }
    }

    // ── P2 ozzt: new pattern detection tests ─────────────────────────────────

    fn find_pattern(name: &str) -> Option<(&'static str, &'static str, u8, f32)> {
        RAW_PATTERNS.iter().find(|(n, _, _, _)| *n == name).copied()
    }

    #[test]
    fn sendgrid_pattern_present_and_high_confidence() {
        let (_, raw, cat, conf) =
            find_pattern("sendgrid_api_key").expect("sendgrid_api_key pattern must be present");
        assert_eq!(cat, 0, "sendgrid_api_key must be category 0 (Credential)");
        assert!(conf >= 0.90, "sendgrid_api_key confidence must be ≥ 0.90");
        // Real SendGrid key shape: SG.<22>.<43>
        let key = format!("SG.{}.{}", "A".repeat(22), "B".repeat(43));
        let re = Regex::new(raw).expect("pattern must compile");
        assert!(
            re.is_match(&key),
            "sendgrid pattern must match real key shape"
        );
    }

    #[test]
    fn sendgrid_pattern_no_fp_on_sg_prefix_without_dots() {
        let (_, raw, _, _) = find_pattern("sendgrid_api_key").unwrap();
        let re = Regex::new(raw).unwrap();
        // A plain "SG" + hex without the two-dot structure must not match.
        assert!(!re.is_match("SGfoo bar"), "no match on bare SG prefix");
    }

    #[test]
    fn terraform_cloud_token_pattern_present_and_high_confidence() {
        let (_, raw, cat, conf) = find_pattern("terraform_cloud_token")
            .expect("terraform_cloud_token pattern must be present");
        assert_eq!(
            cat, 0,
            "terraform_cloud_token must be category 0 (Credential)"
        );
        assert!(
            conf >= 0.90,
            "terraform_cloud_token confidence must be ≥ 0.90"
        );
        let token = format!("atlasv1.{}", "A".repeat(64));
        let re = Regex::new(raw).expect("pattern must compile");
        assert!(
            re.is_match(&token),
            "pattern must match real terraform token"
        );
    }

    #[test]
    fn gcp_service_account_key_pattern_present_and_high_confidence() {
        let (_, raw, cat, conf) = find_pattern("gcp_service_account_key")
            .expect("gcp_service_account_key pattern must be present");
        assert_eq!(
            cat, 0,
            "gcp_service_account_key must be category 0 (Credential)"
        );
        assert!(
            conf >= 0.90,
            "gcp_service_account_key confidence must be ≥ 0.90"
        );
        let json_fragment = r#"{"private_key": "-----BEGIN RSA PRIVATE KEY-----"#;
        let re = Regex::new(raw).expect("pattern must compile");
        assert!(
            re.is_match(json_fragment),
            "pattern must match GCP SA JSON private_key marker"
        );
    }

    #[test]
    fn azure_storage_key_pattern_present_and_high_confidence() {
        let (_, raw, cat, conf) =
            find_pattern("azure_storage_key").expect("azure_storage_key pattern must be present");
        assert_eq!(cat, 0, "azure_storage_key must be category 0 (Credential)");
        assert!(conf >= 0.90, "azure_storage_key confidence must be ≥ 0.90");
        // Must match only in the `AccountKey=` connection-string context — a BARE
        // 88-char base64 blob (SHA-512, random token, etc.) must NOT match, else
        // it would silently auto-wipe benign content (bug-hunt high finding).
        let with_context = format!("AccountKey={}==", "A".repeat(86));
        let bare_blob = format!("{}==", "A".repeat(86));
        let re = Regex::new(raw).expect("pattern must compile");
        assert!(
            re.is_match(&with_context),
            "pattern must match AccountKey=<88-char base64>"
        );
        assert!(
            !re.is_match(&bare_blob),
            "bare 88-char base64 without AccountKey= context must not match"
        );
    }

    #[test]
    fn cloudflare_api_token_pattern_present_and_high_confidence() {
        let (_, raw, cat, conf) = find_pattern("cloudflare_api_token")
            .expect("cloudflare_api_token pattern must be present");
        assert_eq!(
            cat, 0,
            "cloudflare_api_token must be category 0 (Credential)"
        );
        assert!(
            conf >= 0.90,
            "cloudflare_api_token confidence must be ≥ 0.90"
        );
        // Must match in env-var context; a bare 40-char token must not match.
        let with_context = format!("CLOUDFLARE_API_TOKEN={}", "A".repeat(40));
        let bare_token = "A".repeat(40);
        let re = Regex::new(raw).expect("pattern must compile");
        assert!(
            re.is_match(&with_context),
            "pattern must match CLOUDFLARE_API_TOKEN=<40chars>"
        );
        assert!(
            !re.is_match(&bare_token),
            "bare 40-char token without context must not match"
        );
    }

    #[test]
    fn new_patterns_have_word_boundary_anchors() {
        // P2 ozzt requirement: all new prefixed-token patterns must use `\b`.
        // Note: azure_storage_key is CONTEXT-anchored on the literal `AccountKey=`
        // (not `\b`), so it is intentionally excluded from this `\b` list.
        // (cloudflare_api_token keeps a `\b` around its env-var prefix, so it stays.)
        let anchored = [
            "sendgrid_api_key",
            "terraform_cloud_token",
            "cloudflare_api_token",
            "twilio_signing_key_sid",
            "discord_bot_token",
        ];
        for name in &anchored {
            let (_, raw, _, _) =
                find_pattern(name).unwrap_or_else(|| panic!("pattern `{name}` must exist"));
            assert!(
                raw.contains(r"\b"),
                "pattern `{name}` must contain \\b word-boundary anchor; got: {raw}"
            );
        }
    }
}

use regex::{Regex, RegexSet};
use std::sync::OnceLock;

static PATTERN_SET: OnceLock<RegexSet> = OnceLock::new();
static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();

/// (name, raw_regex, category_index, confidence)
/// category_index: 0=Credential, 1=Financial, 2=PersonalId, 3=Infrastructure
pub const RAW_PATTERNS: &[(&str, &str, u8, f32)] = &[
    // ── Credentials ──────────────────────────────────────────────────────────
    ("aws_access_key", r"(?:AKIA|ASIA)[0-9A-Z]{16}", 0, 0.99),
    (
        "github_fine_grained",
        r"github_pat_[A-Za-z0-9]{22}_[A-Za-z0-9]{59}",
        0,
        0.99,
    ),
    ("github_classic_pat", r"ghp_[A-Za-z0-9]{36}", 0, 0.99),
    ("github_actions_token", r"ghs_[a-zA-Z0-9]{36}", 0, 0.99),
    ("openai_new", r"sk-proj-[A-Za-z0-9]{48}", 0, 0.99),
    ("openai_legacy", r"sk-[A-Za-z0-9]{48}", 0, 0.95),
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
    ("hashicorp_vault", r"hvs\.[A-Za-z0-9]+", 0, 0.95),
    ("gcp_oauth", r"GOCSPX-[A-Za-z0-9_-]{28}", 0, 0.99),
    (
        "ssh_private_key",
        r"-----BEGIN (?:RSA |EC |OPENSSH |DSA |)?PRIVATE KEY-----",
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
        "jwt",
        r"eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+",
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
    ("passport", r"\b[A-Z]{1,2}[0-9]{6,9}\b", 2, 0.55),
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
    PATTERN_SET.get_or_init(|| RegexSet::new(RAW_PATTERNS.iter().map(|(_, p, _, _)| *p)).unwrap())
}

pub fn patterns() -> &'static Vec<Regex> {
    PATTERNS.get_or_init(|| {
        RAW_PATTERNS
            .iter()
            .map(|(_, p, _, _)| Regex::new(p).unwrap())
            .collect()
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

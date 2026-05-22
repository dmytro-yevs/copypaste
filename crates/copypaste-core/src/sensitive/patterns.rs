use regex::{Regex, RegexSet};
use std::sync::OnceLock;

static PATTERN_SET: OnceLock<RegexSet> = OnceLock::new();
#[allow(dead_code)]
static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();

const RAW_PATTERNS: &[(&str, &str)] = &[
    ("aws_access_key",      r"(?:AKIA|ASIA)[0-9A-Z]{16}"),
    ("github_fine_grained", r"github_pat_[A-Za-z0-9]{22}_[A-Za-z0-9]{59}"),
    ("github_classic_pat",  r"ghp_[A-Za-z0-9]{36}"),
    ("openai_new",          r"sk-proj-[A-Za-z0-9]{48}"),
    ("openai_legacy",       r"sk-[A-Za-z0-9]{48}"),
    ("anthropic",           r"sk-ant-api\d{2}-[A-Za-z0-9_-]{80,}"),
    ("stripe_live",         r"sk_live_[0-9A-Za-z]{24}"),
    ("npm_token",           r"npm_[A-Za-z0-9]{36}"),
    ("pypi_token",          r"pypi-[A-Za-z0-9_-]{180,}"),
    ("slack_bot",           r"xoxb-[0-9]{11}-[0-9]{11}-[a-zA-Z0-9]{24}"),
    ("slack_webhook",       r"https://hooks\.slack\.com/services/T[A-Z0-9]+/B[A-Z0-9]+/[a-zA-Z0-9]+"),
    ("discord_bot_token",   r"[MN][a-zA-Z\d]{23,25}\.[\w-]{6}\.[\w-]{27,38}"),
    ("stripe_webhook",      r"whsec_[a-zA-Z0-9]{32,64}"),
    ("twilio_auth_token",   r"SK[a-f0-9]{32}"),
    ("github_actions_token",r"ghs_[a-zA-Z0-9]{36}"),
    ("google_api_key",      r"AIza[0-9A-Za-z\-_]{35}"),
    ("heroku_api_key",      r"(?i)heroku[^\n]{0,50}[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}"),
    ("hashicorp_vault",     r"hvs\.[A-Za-z0-9]+"),
    ("gcp_oauth",           r"GOCSPX-[A-Za-z0-9_-]{28}"),
    ("ssh_private_key",     r"-----BEGIN (?:RSA |EC |OPENSSH |)?PRIVATE KEY-----"),
    ("jwt",                 r"eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+"),
];

pub fn pattern_set() -> &'static RegexSet {
    PATTERN_SET.get_or_init(|| RegexSet::new(RAW_PATTERNS.iter().map(|(_, p)| *p)).unwrap())
}

#[allow(dead_code)]
pub fn patterns() -> &'static Vec<Regex> {
    PATTERNS.get_or_init(|| RAW_PATTERNS.iter().map(|(_, p)| Regex::new(p).unwrap()).collect())
}

pub fn pattern_name(index: usize) -> &'static str {
    RAW_PATTERNS.get(index).map(|(n, _)| *n).unwrap_or("unknown")
}

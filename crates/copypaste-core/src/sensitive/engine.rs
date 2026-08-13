//! One compiled ruleset feeds span detection, compatibility labelling,
//! redaction, and the high-confidence whole-item gate.

use std::collections::HashMap;

use regex::{Captures, Regex, RegexSet, RegexSetBuilder};

use super::finding::{Finding, Severity, SpannedFinding};
use super::normalise::normalise;
use super::redact::redact_findings;
use super::rules::{GLOBAL_ALLOWLISTS, RULES};
use super::spec::{AllowlistCondition, AllowlistSpec, AllowlistTarget, RuleSpec, Validator};
use super::validators::{
    card_number_valid, iban_valid, phone_is_formatted, ssn_structure_plausible, value_is_strong,
};

/// Regex compilation failures. No variant carries a path or any input text
/// (AGENTS.md rule 4).
#[derive(Debug, thiserror::Error)]
pub enum DetectorError {
    /// A single rule's regex failed to compile.
    #[error("sensitive-content rule `{rule}` failed to compile")]
    Rule {
        rule: &'static str,
        #[source]
        source: regex::Error,
    },
    /// The combined prefilter failed to compile.
    #[error("the sensitive-content rule set failed to compile")]
    RuleSet(#[source] regex::Error),
    /// A selected upstream allowlist failed to compile.
    #[error("sensitive-content allowlist for `{rule}` failed to compile")]
    Allowlist {
        rule: &'static str,
        #[source]
        source: regex::Error,
    },
}

/// The vendored Gitleaks patterns overflow `regex`'s 2 MiB lazy-DFA cache.
/// At 8 MiB the prefilter benchmark took 13 seconds against 1 second for
/// serial searches; 64 MiB restores the prefilter advantage. The cache is
/// allocated on demand, so this is a ceiling rather than a fixed cost.
const PREFILTER_DFA_SIZE_LIMIT: usize = 64 * 1024 * 1024;
const PREFILTER_COMPILED_SIZE_LIMIT: usize = 32 * 1024 * 1024;

/// The compiled ruleset. Construct **once** and share it: `new()` compiles the
/// generated regexes plus the prefilter, and this runs on the clipboard hot path
/// (`CopyPaste-mnte`: v1 built the detector once per history page).
pub struct Detector {
    /// Prefilter. One pass says which rules can possibly match; only those
    /// individual regexes are then run.
    set: RegexSet,
    /// Index-aligned with `set`, but each entry carries its own name,
    /// category, confidence and validator, so a partial compile can never
    /// desync names from matches (manifest I8 — v1 needed a whole
    /// degrade-to-empty mechanism to hold this invariant; here it is
    /// structural, §7.6).
    rules: Vec<Rule>,
    /// Index-aligned with `rules`: what a match from this rule permits. Derived
    /// from the rule table at construction and never listed by hand, or a new
    /// rule would silently drop out of the gate between detection and
    /// destruction.
    severities: Vec<Severity>,
    global_allowlists: Vec<CompiledAllowlist>,
}

/// The two shapes [`Detector::scan_normalised`] answers in. One implementation,
/// because §7.4 records that v1's four overlapping "is it sensitive?" entry
/// points were the defect — a second predicate is what must not exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScanMode {
    /// Every validated match, ranked.
    AllSpans,
    /// The first validated match this severe or worse. Rules that cannot reach
    /// it cannot change the answer, so they are not run at all.
    FirstAtLeast(Severity),
}

impl Detector {
    /// Compile the ruleset. Fails only on a typo in a pattern literal, which
    /// `ruleset_compiles` pins.
    pub fn new() -> Result<Self, DetectorError> {
        let mut rules = Vec::with_capacity(RULES.len());
        for spec in RULES {
            let regex = Regex::new(spec.pattern).map_err(|source| DetectorError::Rule {
                rule: spec.name,
                source,
            })?;
            rules.push(Rule {
                spec,
                regex,
                keywords: spec
                    .keywords
                    .iter()
                    .map(|keyword| keyword.to_lowercase())
                    .collect(),
                allowlists: compile_allowlists(spec.name, spec.allowlists)?,
            });
        }
        let set = RegexSetBuilder::new(RULES.iter().map(|r| r.pattern))
            .size_limit(PREFILTER_COMPILED_SIZE_LIMIT)
            .dfa_size_limit(PREFILTER_DFA_SIZE_LIMIT)
            .build()
            .map_err(DetectorError::RuleSet)?;
        let global_allowlists = compile_allowlists("global", GLOBAL_ALLOWLISTS)?;
        let severities = rules
            .iter()
            .map(|rule| rule.spec.finding().severity)
            .collect();
        Ok(Self {
            set,
            rules,
            severities,
            global_allowlists,
        })
    }

    /// All validated matches in NFKC-normalised UTF-8 byte order.
    ///
    /// Offsets in every [`SpannedFinding`] index the normalised string, not
    /// necessarily `text`. At the same start offset, longer matches sort first,
    /// followed by confidence and stable rule id.
    pub fn scan_all(&self, text: &str) -> Vec<SpannedFinding> {
        let normalised = normalise(text);
        self.scan_all_normalised(&normalised)
    }

    pub(super) fn scan_all_normalised(&self, text: &str) -> Vec<SpannedFinding> {
        self.scan_normalised(text, ScanMode::AllSpans)
    }

    /// Highest-confidence match, or None. Ties prefer the longer match, then
    /// the stable rule id and earlier byte position.
    pub fn scan(&self, text: &str) -> Option<Finding> {
        let normalised = normalise(text);
        self.scan_normalised(&normalised, ScanMode::AllSpans)
            .into_iter()
            .max_by(compare_rank)
            .map(SpannedFinding::without_span)
    }

    fn scan_normalised(&self, text: &str, mode: ScanMode) -> Vec<SpannedFinding> {
        let first_only = matches!(mode, ScanMode::FirstAtLeast(_));
        let mut findings = Vec::new();
        let lowered = text.to_lowercase();
        for idx in self.set.matches(text) {
            // A rule that cannot reach the caller's severity can never change
            // its answer, and these are the rules ordinary content fires: any
            // email address, any `host:port` in a config paste. Running their
            // `captures_iter` and their checksum validators to build findings
            // the caller throws away is the whole of F-CORE-4.
            if let ScanMode::FirstAtLeast(least) = mode {
                if self.severities[idx] < least {
                    continue;
                }
            }
            let rule = &self.rules[idx];
            if !rule.keyword_matches(&lowered) {
                continue;
            }
            for (start, end) in rule.spans(text, &self.global_allowlists, first_only) {
                findings.push(SpannedFinding::new(rule.spec.finding(), start, end));
                if first_only {
                    return findings;
                }
            }
        }
        findings.sort_by(|a, b| {
            a.start
                .cmp(&b.start)
                .then_with(|| b.end.cmp(&a.end))
                .then_with(|| b.confidence.total_cmp(&a.confidence))
                .then_with(|| a.rule.cmp(b.rule))
        });
        findings
    }

    /// True when this text may be **deleted** automatically.
    ///
    /// The one gate between detection and destruction. v1 collapsed it with
    /// [`Detector::is_sensitive`] three separate times (`AB-6a`, `PG-23`,
    /// `PG-3`) and destroyed unrecoverable user data each time; manifest I2
    /// keeps them apart, and [`Severity::Restricted`] is the band where they
    /// disagree.
    pub fn may_auto_wipe(&self, text: &str) -> bool {
        self.reaches(text, Severity::HighConfidence)
    }

    /// True when this text must be **withheld**: kept out of the search index,
    /// out of sync and out of previews.
    ///
    /// Weaker than [`Detector::may_auto_wipe`] by exactly the restricted band,
    /// and the one every storage and sync guard wants. Withholding costs the
    /// user searchability; deleting costs them the data (`AGENTS.md` rule 4).
    pub fn is_sensitive(&self, text: &str) -> bool {
        self.reaches(text, Severity::Restricted)
    }

    fn reaches(&self, text: &str, least: Severity) -> bool {
        let normalised = normalise(text);
        !self
            .scan_normalised(&normalised, ScanMode::FirstAtLeast(least))
            .is_empty()
    }

    /// Redact every validated match. When matches exist, surrounding text is
    /// NFKC-normalised because the match offsets belong to that representation.
    ///
    /// The only span consumer, and it has no caller anywhere in the tree.
    pub fn redact(&self, text: &str) -> String {
        let normalised = normalise(text);
        let findings = self.scan_normalised(&normalised, ScanMode::AllSpans);
        if findings.is_empty() {
            text.to_owned()
        } else {
            redact_findings(&normalised, &findings)
        }
    }
}

pub(super) fn compare_rank(a: &SpannedFinding, b: &SpannedFinding) -> std::cmp::Ordering {
    a.confidence
        .total_cmp(&b.confidence)
        .then_with(|| (a.end - a.start).cmp(&(b.end - b.start)))
        .then_with(|| b.rule.cmp(a.rule))
        .then_with(|| b.start.cmp(&a.start))
}

/// One table entry, compiled.
struct Rule {
    spec: &'static RuleSpec,
    regex: Regex,
    keywords: Vec<String>,
    allowlists: Vec<CompiledAllowlist>,
}

impl Rule {
    /// Every match that passes this rule's validator, or just the first when
    /// the caller only needs to know whether one exists. No separate fast path:
    /// v1's `RegexSet` shortcut skipped the value-strength validator and needed
    /// a bespoke `generic_password_kv` case to compensate (§5.3). `first_only`
    /// stops the iteration, it does not weaken a single check.
    fn spans(
        &self,
        text: &str,
        global_allowlists: &[CompiledAllowlist],
        first_only: bool,
    ) -> Vec<(usize, usize)> {
        let mut spans = Vec::new();
        for caps in self.regex.captures_iter(text) {
            let Some(whole) = caps.get(0) else { continue };
            // A rule that names a secret group and does not get one cannot be
            // judged: its entropy threshold and its stopwords were both written
            // against the value, and scoring them against the whole match reads
            // the keyword and the context anchor as if they were the credential.
            // That direction ends in a deletion, so the candidate is dropped.
            let Some(secret) = secret(&caps, self.spec.secret_group, whole.as_str()) else {
                continue;
            };
            // Both value validators judge the rule's declared secret group, not
            // group 1: `secret_group` is what the entropy threshold and the
            // allowlists already use, and a rule whose value lives elsewhere
            // would otherwise be gated on whatever group 1 happened to be.
            let ok = match self.spec.validator {
                Validator::None => true,
                Validator::ValueStrength => value_is_strong(secret),
                Validator::CardNumber => card_number_valid(whole.as_str()),
                Validator::Iban => iban_valid(whole.as_str()),
                Validator::SsnStructure => ssn_structure_plausible(whole.as_str()),
                Validator::PhoneShape => phone_is_formatted(whole.as_str()),
            };
            if !ok {
                continue;
            }
            if self
                .spec
                .entropy
                .is_some_and(|minimum| shannon_entropy(secret) <= minimum)
            {
                continue;
            }
            let line = line_containing(text, whole.start(), whole.end());
            if !self.spec.upstream_ids.is_empty() && line.contains("gitleaks:allow") {
                continue;
            }
            let globally_allowed = !self.spec.upstream_ids.is_empty()
                && is_allowed(global_allowlists, secret, whole.as_str(), line);
            if !globally_allowed && !is_allowed(&self.allowlists, secret, whole.as_str(), line) {
                spans.push((whole.start(), whole.end()));
                if first_only {
                    break;
                }
            }
        }
        spans
    }

    fn keyword_matches(&self, lowered_text: &str) -> bool {
        self.keywords.is_empty()
            || self
                .keywords
                .iter()
                .any(|keyword| lowered_text.contains(keyword))
    }
}

struct CompiledAllowlist {
    condition: AllowlistCondition,
    target: AllowlistTarget,
    regexes: Vec<Regex>,
    stopwords: Vec<String>,
}

fn compile_allowlists(
    rule: &'static str,
    specs: &'static [AllowlistSpec],
) -> Result<Vec<CompiledAllowlist>, DetectorError> {
    specs
        .iter()
        .map(|spec| {
            let regexes = spec
                .regexes
                .iter()
                .map(|pattern| {
                    Regex::new(pattern).map_err(|source| DetectorError::Allowlist { rule, source })
                })
                .collect::<Result<_, _>>()?;
            Ok(CompiledAllowlist {
                condition: spec.condition,
                target: spec.target,
                regexes,
                stopwords: spec
                    .stopwords
                    .iter()
                    .map(|word| word.to_lowercase())
                    .collect(),
            })
        })
        .collect()
}

fn secret<'a>(caps: &'a Captures<'a>, group: usize, whole: &'a str) -> Option<&'a str> {
    if group == 0 {
        return Some(whole);
    }
    caps.get(group).map(|matched| matched.as_str())
}

fn shannon_entropy(value: &str) -> f64 {
    let mut counts = HashMap::new();
    let mut length = 0_u32;
    for character in value.chars() {
        *counts.entry(character).or_insert(0_u32) += 1;
        length += 1;
    }
    if length == 0 {
        return 0.0;
    }
    counts.values().fold(0.0, |entropy, count| {
        let frequency = f64::from(*count) / f64::from(length);
        entropy - frequency * frequency.log2()
    })
}

fn line_containing(text: &str, start: usize, end: usize) -> &str {
    let line_start = text[..start].rfind('\n').map_or(0, |index| index + 1);
    let line_end = text[end..]
        .find('\n')
        .map_or(text.len(), |index| end + index);
    &text[line_start..line_end]
}

fn is_allowed(allowlists: &[CompiledAllowlist], secret: &str, matched: &str, line: &str) -> bool {
    allowlists.iter().any(|allowlist| {
        let target = match allowlist.target {
            AllowlistTarget::Secret => secret,
            AllowlistTarget::Match => matched,
            AllowlistTarget::Line => line,
        };
        let regex_match = (!allowlist.regexes.is_empty())
            .then(|| allowlist.regexes.iter().any(|regex| regex.is_match(target)));
        let stopword_match = (!allowlist.stopwords.is_empty()).then(|| {
            let lowered_secret = secret.to_lowercase();
            allowlist
                .stopwords
                .iter()
                .any(|word| lowered_secret.contains(word))
        });
        match allowlist.condition {
            AllowlistCondition::Any => {
                regex_match.unwrap_or(false) || stopword_match.unwrap_or(false)
            }
            // `all` over nothing is true, and an allowlist that suppresses
            // everything it is attached to is the one direction this module may
            // never fail in: the rule stops firing and the item is neither
            // flagged nor masked. An allowlist carrying neither a regex nor a
            // stopword allows nothing.
            AllowlistCondition::All => {
                (regex_match.is_some() || stopword_match.is_some())
                    && regex_match.unwrap_or(true)
                    && stopword_match.unwrap_or(true)
            }
        }
    })
}

/// Fixtures and helpers shared by every test module under `sensitive`. Here
/// because [`all_rules`] needs `Detector`'s private fields.
#[cfg(test)]
pub(super) mod test_support {
    use super::Detector;

    pub(in crate::sensitive) fn detector() -> Detector {
        Detector::new().expect("ruleset compiles")
    }

    /// All validated matches, ranked. Test-only: production has exactly one
    /// verdict function and one label function (§7.4 — v1 shipped four
    /// overlapping "is it sensitive?" entry points, three of them dead).
    pub(in crate::sensitive) fn all_rules(det: &Detector, text: &str) -> Vec<&'static str> {
        det.scan_all(text)
            .iter()
            .filter_map(|finding| {
                det.rules
                    .iter()
                    .find(|rule| rule.spec.name == finding.rule)
                    .map(|rule| rule.spec.name)
            })
            .collect()
    }

    pub(in crate::sensitive) fn fired(det: &Detector, text: &str, rule: &str) -> bool {
        all_rules(det, text).contains(&rule)
    }

    pub(in crate::sensitive) fn rep(c: char, n: usize) -> String {
        std::iter::repeat_n(c, n).collect()
    }

    pub(in crate::sensitive) const ALNUM: &str =
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    pub(in crate::sensitive) const BASE64: &str =
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    pub(in crate::sensitive) const HEX: &str = "0123456789abcdef";

    /// A deterministic pseudo-random fixture of `n` characters over `alphabet`.
    ///
    /// Positive fixtures have to be *shaped* like the credential they stand for,
    /// and randomness is part of that shape: every context-anchored rule gates on
    /// its value's entropy, because the anchor proves which field matched and
    /// says nothing about the value (§5.6). A repeated-character fixture measures
    /// near zero and would force the threshold off — which is exactly what the
    /// ten `entropy_override = 0.0` entries used to do, and what left
    /// `AccountKey=<86 identical chars>` indistinguishable from a README example.
    ///
    /// Deterministic so a failure reproduces: xorshift64 from a fixed seed, no
    /// dependency and no `rand` in the test tree.
    pub(in crate::sensitive) fn noise(n: usize, alphabet: &str) -> String {
        let chars: Vec<char> = alphabet.chars().collect();
        let mut state = 0x243f_6a88_85a3_08d3_u64;
        (0..n)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                chars[(state % chars.len() as u64) as usize]
            })
            .collect()
    }

    /// The benign corpus. §7.7: v1's `max(len * 5 / 100, 2)` budget tolerated
    /// two unnamed FPs — `const password = prompt(...)` was one of them. v2
    /// asserts **zero**; any accepted FP must be named here with a reason.
    ///
    /// Shared with [`crate::sensitive::normalise`], which asserts NFKC is the
    /// identity on every entry.
    pub(in crate::sensitive) const BENIGN_CORPUS: &[&str] = &[
        "the password is great, you should try it",
        "my secret is to drink coffee every morning",
        "I forgot my password again, time to reset it",
        "password protected zip files are common",
        "the auth token expired, please log in again",
        "// example: set api_key=demo to enable test mode",
        "# password: <set in your env file>",
        "/* secret = TBD, fill in before deploy */",
        "Note: passwd:enabled means SSH password auth is on",
        "Set apikey: yourkey in the config (do not commit)",
        "// auth_token: see README for setup",
        "# api_key=demo for examples only",
        "fn check_password(pw: &str) -> bool { pw.len() > 8 }",
        "const SECRET_NAME = \"prod-key\";",
        "let api_key = getEnv(); // value loaded later",
        "const password = prompt('enter password:');",
        "AWS region us-east-1 is recommended",
        "see arn naming conventions in the AWS docs",
        "the GitHub repo URL is https://github.com/example/repo",
        "https://example.com/login?next=/dashboard",
        "open the file at C:\\Users\\Public\\Documents",
        "the order number is 1234567890",
        "tracking ID 0010 0020 0030",
        "ticket #4815 has been assigned to you",
        "version 1.2.3 was released yesterday",
        "The api_key returns 401, please investigate",
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit.",
        "fn main() { println!(\"Hello, world!\"); }",
        // the keyword spellings the delimiter-tolerant rule 23 added
        "rotate the api-key before the next release",
        "the client secret is stored in the password manager",
        "run with --api-key=<token> to authenticate",
        "see docs for db-password rotation steps",
    ];
}

#[cfg(test)]
mod tests {
    use super::test_support::{
        all_rules, detector, fired, noise, ALNUM, BASE64, BENIGN_CORPUS, HEX,
    };
    use super::*;
    use crate::sensitive::finding::{Severity, AUTOWIPE_CONFIDENCE_FLOOR};

    // -- §9.1 true positives ------------------------------------------------

    /// Every entry here must be detected. The `Some(rule)` column, where
    /// present, pins *which* rule wins the confidence ranking (§7.2).
    #[test]
    fn manifest_true_positives_are_detected() {
        let det = detector();
        let ghp = format!("ghp_{}", noise(36, ALNUM));
        let fine = format!("github_pat_{}_{}", noise(22, ALNUM), noise(59, ALNUM));
        let openai_new = format!("sk-proj-{}", noise(48, ALNUM));
        let openai_legacy = format!("sk-{}", noise(48, ALNUM));
        let anthropic = format!("sk-ant-api03-{}", noise(80, ALNUM));
        let stripe = format!("sk_live_{}", noise(24, ALNUM));
        let npm = format!("npm_{}", noise(36, ALNUM));
        let slack_hook = format!(
            "https://hooks.slack.com/services/T00000000/B00000000/{}",
            noise(24, ALNUM)
        );
        let vault = format!("hvs.{}", noise(32, ALNUM));
        let sendgrid = format!("SG.{}.{}", noise(22, ALNUM), noise(43, ALNUM));
        let terraform = format!("atlasv1.{}", noise(64, ALNUM));
        let azure = format!("AccountKey={}==", noise(86, BASE64));

        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.\
                   SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        let jwt_rs256 = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCIsImtpZCI6ImFiYzEyMyJ9.\
                         eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4ifQ.\
                         QWJjRGVmR2hpSmtsTW5vUHFyU3R1VnimVwxYeVowMTIzNDU2Nzg5";

        let cases: &[(&str, Option<&str>)] = &[
            ("AKIAIOSFODNN7EXAMPLE", Some("aws_access_key")),
            (
                "AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE",
                Some("aws_access_key"),
            ),
            // trailing digits must not break the ASIA form (no trailing \b)
            ("ASIAIOSFODNN7EXAMPLE1234", Some("aws_access_key")),
            (&ghp, Some("github_classic_pat")),
            (&fine, Some("github_fine_grained")),
            (
                "ghs_16C7e42F292c6912E7710c838347Ae178B4a",
                Some("github_app_token"),
            ),
            (&openai_new, Some("openai_new")),
            (&openai_legacy, Some("openai_legacy")),
            (&anthropic, Some("anthropic")),
            (&stripe, Some("stripe_live")),
            (
                "whsec_aAbBcCdDeEfFgGhHiIjJkKlLmMnNoOpPqQrRsStT",
                Some("stripe_webhook"),
            ),
            (&npm, Some("npm_token")),
            (
                "xoxb-17653285717-17653285718-AbCdEfGhIjKlMnOpQrStUvWx",
                Some("slack_token"),
            ),
            (&slack_hook, Some("slack_webhook")),
            (
                "AIzaSyD-9tSrke72EmVt4TenJheB96ABCDE12345",
                Some("google_api_key"),
            ),
            (&vault, Some("hashicorp_vault")),
            (
                "-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEA",
                Some("private_key"),
            ),
            ("-----BEGIN OPENSSH PRIVATE KEY-----", Some("private_key")),
            ("-----BEGIN EC PRIVATE KEY-----", Some("private_key")),
            ("-----BEGIN PRIVATE KEY-----", Some("private_key")),
            // Audit MED #5 — the ENCRYPTED header was a real miss in v1
            (
                "garbage prefix\n-----BEGIN ENCRYPTED PRIVATE KEY-----\nMIIFDjBABgkq",
                Some("private_key"),
            ),
            // Audit MED #5 — PuTTY .ppk was a real miss in v1
            (
                "PuTTY-User-Key-File-2: ssh-rsa\nEncryption: none\nComment: rsa-key",
                Some("putty_private_key"),
            ),
            // header mid-blob, not line 1
            (
                "# SSH key below\n-----BEGIN RSA PRIVATE KEY-----\nMIIE",
                Some("private_key"),
            ),
            (jwt, Some("jwt")),
            (jwt_rs256, Some("jwt")),
            (&format!("Bearer {jwt}"), Some("jwt")),
            (
                "postgresql://alice:S3cr3tP@ss@db.example.com:5432/mydb",
                Some("db_conn_string"),
            ),
            (
                "mysql://root:hunter2@127.0.0.1:3306/prod",
                Some("db_conn_string"),
            ),
            (
                "mongodb://admin:P@ssw0rd!@mongo.internal:27017/mydb?authSource=admin",
                Some("db_conn_string"),
            ),
            // empty username
            (
                "redis://:my_redis_secret_password@redis.example.com:6379/0",
                Some("db_conn_string"),
            ),
            // CopyPaste-2eet
            (
                "access_token=abc123XYZlongvalue99",
                Some("generic_password_kv"),
            ),
            (
                "access_token: gh_access_abc123XYZ",
                Some("generic_password_kv"),
            ),
            (
                "export access_token=abc123XYZlongvalue99",
                Some("generic_password_kv"),
            ),
            (
                "client_secret=Sup3rS3cr3tV@lue!",
                Some("generic_password_kv"),
            ),
            (
                "refresh_token=rt_abc123XYZlong_value",
                Some("generic_password_kv"),
            ),
            (
                "refresh_token = rt_PROD_abc123XYZlongval",
                Some("generic_password_kv"),
            ),
            ("db_password=S3cur3Pass!word", Some("generic_password_kv")),
            ("password=hunter2", Some("generic_password_kv")),
            ("secret = !abcdef", Some("generic_password_kv")),
            ("password: abcdefghij", Some("generic_password_kv")),
            (&sendgrid, Some("sendgrid_api_key")),
            (&terraform, Some("terraform_cloud_token")),
            (
                "{\"private_key\": \"-----BEGIN RSA PRIVATE KEY-----\\nMIIEo\"}",
                Some("gcp_service_account_key"),
            ),
            (&azure, Some("azure_storage_key")),
            ("4111111111111111", Some("credit_card")),
            // Audit MED #6 — a card embedded in text was a silent miss in v1
            (
                "Customer card: 4111 1111 1111 1111 — expires 12/26",
                Some("credit_card"),
            ),
            (
                "please charge 4111-1111-1111-1111 today",
                Some("credit_card"),
            ),
        ];

        for (input, expected) in cases {
            let finding = det.scan(input);
            assert!(finding.is_some(), "not detected: {input:?}");
            if let Some(want) = expected {
                assert!(
                    fired(&det, input, want),
                    "rule {want} did not fire on {input:?}; fired: {:?}",
                    all_rules(&det, input)
                );
            }
            assert!(det.is_sensitive(input), "is_sensitive false for {input:?}");
        }
    }

    /// §9.1: everything in this list is distinctive enough to delete.
    #[test]
    fn high_confidence_true_positives_are_above_the_floor() {
        let det = detector();
        let inputs = [
            "AKIAIOSFODNN7EXAMPLE".to_string(),
            format!("sk-{}", noise(48, ALNUM)),
            format!("hvs.{}", noise(32, ALNUM)),
            format!("SG.{}.{}", noise(22, ALNUM), noise(43, ALNUM)),
            format!("atlasv1.{}", noise(64, ALNUM)),
            format!("AccountKey={}==", noise(86, BASE64)),
            "-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEA".to_string(),
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c"
                .to_string(),
            "{\"private_key\": \"-----BEGIN RSA PRIVATE KEY-----\\nMIIEo\"}".to_string(),
        ];
        for input in &inputs {
            let f = det.scan(input).unwrap_or_else(|| panic!("{input:?}"));
            assert_eq!(f.severity, Severity::HighConfidence, "{input:?} -> {f:?}");
            assert!(f.confidence >= AUTOWIPE_CONFIDENCE_FLOOR);
        }
        // The card fixture used to be in that list. It is still above the floor
        // and still classified; §3.3 takes away only the deletion.
        let card = det.scan("4111111111111111").unwrap();
        assert!(card.confidence >= AUTOWIPE_CONFIDENCE_FLOOR);
        assert_eq!(card.severity, Severity::Restricted);
    }

    /// The `sk-proj-` exclusion is structural, not a lookahead (P2 `r6cw`).
    #[test]
    fn openai_legacy_does_not_double_fire_on_proj_keys() {
        let det = detector();
        let key = format!("sk-proj-{}", noise(48, ALNUM));
        assert!(fired(&det, &key, "openai_new"));
        assert!(!fired(&det, &key, "openai_legacy"));
    }

    // -- §9.2 false positives ------------------------------------------------

    #[test]
    fn benign_corpus_has_zero_false_positives() {
        let det = detector();
        let offenders: Vec<_> = BENIGN_CORPUS
            .iter()
            .filter(|text| !det.scan_all(text).is_empty())
            .map(|t| (t, all_rules(&det, t)))
            .collect();
        assert!(offenders.is_empty(), "false positives: {offenders:#?}");
    }

    #[test]
    fn manifest_hard_negatives_produce_no_detection() {
        let det = detector();
        let cases: Vec<String> = vec![
            "Lorem ipsum dolor sit amet, consectetur adipiscing elit.".into(),
            "fn main() { println!(\"Hello, world!\"); }".into(),
            // value too short / no variety
            "password: foo".into(),
            "secret = nope".into(),
            // CopyPaste-2eet guard: the keywords were added, the gate stayed
            "access_token=short".into(),
            "refresh_token=abc".into(),
            // below the {32,} vault minimum
            "hvs.abc123".into(),
            // context anchors missing
            noise(40, ALNUM),
            format!("{}==", noise(86, BASE64)),
            // SG without the two-dot structure
            "SGfoo bar".into(),
            // \b anchor: must not classify as a JWT
            "configsomethingeyJabc.def.ghi notajwt".into(),
            // Luhn-invalid 13-digit run
            "ref=4242424242421 EOT".into(),
            // a bare UUID is not a Heroku key without the context word
            "550e8400-e29b-41d4-a716-446655440000".into(),
            // a git SHA is not a secret
            "9f2c1d4e8a7b6c5d4e3f2a1b0c9d8e7f6a5b4c3d".into(),
            // long base64 that is merely long
            "TG9yZW0gaXBzdW0gZG9sb3Igc2l0IGFtZXQsIGNvbnNlY3RldHVyIGFkaXBpc2Npbmc=".into(),
        ];
        for input in &cases {
            assert!(
                det.scan_all(input).is_empty(),
                "false positive on {input:?}: {:?}",
                all_rules(&det, input)
            );
            assert!(det.scan(input).is_none());
        }
    }

    /// A UUID, a git SHA and a long base64 blob are the three shapes users
    /// paste constantly. None of them may reach any rule.
    #[test]
    fn identifiers_are_not_secrets() {
        let det = detector();
        for input in [
            "550e8400-e29b-41d4-a716-446655440000",
            "urn:uuid:123e4567-e89b-12d3-a456-426614174000",
            "commit 9f2c1d4e8a7b6c5d4e3f2a1b0c9d8e7f6a5b4c3d",
            "d41d8cd98f00b204e9800998ecf8427e",
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ] {
            assert!(
                det.scan_all(input).is_empty(),
                "{input:?} -> {:?}",
                all_rules(&det, input)
            );
        }
    }

    /// §9.2's inert band: detected and maskable, but searchable and never
    /// classified for deletion. This is manifest I1, the prime directive.
    #[test]
    fn inert_band_is_detected_but_never_auto_wipes() {
        let det = detector();
        let twilio = format!("SK{}", noise(32, HEX));
        let cases: &[(&str, &str)] = &[
            ("Call me at (555) 867-5309", "phone_us"),
            ("Send to alice@example.com", "email"),
            ("Order AB123456789 is ready", "passport"),
            // the user's own bank details — P2 fb3e, real data loss
            ("DE89370400440532013000", "iban"),
            // a date, not an SSN
            ("012 31 2024", "ssn_us"),
            (
                "MNabcdefghijklmnopqrstuvwx.ABCDEF.ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456",
                "discord_bot_token",
            ),
            (&twilio, "twilio_signing_key_sid"),
            (
                "Authorization: Bearer eyJhbGci0iJSUzI1NiIsInR5cCI6IkpXVCJ9",
                "generic_bearer",
            ),
            // CopyPaste-8ys1 — RFC1918 addresses in configs were auto-wiped
            ("db_host=10.0.0.1:5432", "ip_with_port"),
            ("172.16.0.5:6379", "ip_with_port"),
            ("192.168.1.100:8080", "ip_with_port"),
            ("192.168.1.1:5432", "ip_with_port"),
            // §7.1 — an ARN is a resource identifier, not credential material
            ("arn:aws:iam::123456789012:role/ReadOnly", "aws_arn"),
        ];
        for (input, expected_rule) in cases {
            assert!(
                fired(&det, input, expected_rule),
                "{expected_rule} did not fire on {input:?}; fired: {:?}",
                all_rules(&det, input)
            );
            assert!(!det.is_sensitive(input), "{input:?}");
            assert!(!det.may_auto_wipe(input), "{input:?}");
            let f = det.scan(input).unwrap();
            assert_eq!(f.severity, Severity::Flag, "{input:?} -> {f:?}");
            assert!(f.confidence < AUTOWIPE_CONFIDENCE_FLOOR);
        }
    }

    // -- §9.3 structural / meta tests ---------------------------------------

    #[test]
    fn ruleset_compiles() {
        assert!(Detector::new().is_ok());
    }

    /// The predicate skips every rule below the floor and stops at the first
    /// validated match, so it must still answer exactly what a full ranked scan
    /// answers — this is the gate between detection and destruction.
    #[test]
    fn the_predicate_answers_what_the_full_scan_answers() {
        let det = detector();
        let ghp = format!("ghp_{}", noise(36, ALNUM));
        let twilio = format!("SK{}", noise(32, HEX));
        let mut corpus: Vec<String> = BENIGN_CORPUS.iter().map(|t| (*t).to_string()).collect();
        corpus.extend(
            [
                "AKIAIOSFODNN7EXAMPLE",
                "please charge 4111 1111 1111 1111 today",
                "mail alice@example.com token AKIAIOSFODNN7EXAMPLE",
                "Send to alice@example.com from 192.168.1.100:8080",
                "db_host=10.0.0.1:5432",
                "DE89370400440532013000",
                "012 31 2024",
                "Call me at (555) 867-5309",
                "Order AB123456789 is ready",
                "arn:aws:iam::123456789012:role/ReadOnly",
                "postgresql://alice:S3cr3tP@ss@db.example.com:5432/mydb",
                "password=hunter2",
                "password: foo",
                "",
                "   \n\t",
                "\u{FF21}\u{FF2B}\u{FF29}\u{FF21}IOSFODNN7EXAMPLE",
            ]
            .map(str::to_string),
        );
        corpus.push(ghp);
        corpus.push(twilio);
        corpus.push(format!("notes\n{}\nmore notes", "AKIAIOSFODNN7EXAMPLE"));

        for text in &corpus {
            let reaches = |least| det.scan_all(text).iter().any(|f| f.severity >= least);
            assert_eq!(
                det.may_auto_wipe(text),
                reaches(Severity::HighConfidence),
                "{text:?}"
            );
            assert_eq!(
                det.is_sensitive(text),
                reaches(Severity::Restricted),
                "{text:?}"
            );
        }
        // The corpus carries the case the two predicates answer differently, or
        // the loop above would pass without ever exercising the split.
        let card = "please charge 4111 1111 1111 1111 today";
        assert!(det.is_sensitive(card));
        assert!(!det.may_auto_wipe(card));
    }

    #[test]
    fn the_predicate_is_cheaper_than_the_ranked_scan_it_replaced() {
        const UNIT: &str = "DB_HOST=10.0.0.1:5432\nSMTP_USER=alice@example.com\n\
                            Authorization: Bearer eyJhbGci0iJSUzI1NiIsInR5cCI6IkpXVCJ9\n\
                            contact: bob@example.org, 172.16.0.5:6379\n";
        const RUNS: u32 = 5;

        let det = detector();
        let text = UNIT.repeat(500);
        let ranked = |text: &str| {
            det.scan_all(text)
                .iter()
                .any(|finding| finding.severity == Severity::HighConfidence)
        };
        assert!(!ranked(&text));
        assert!(!det.may_auto_wipe(&text));

        let started = std::time::Instant::now();
        for _ in 0..RUNS {
            assert!(!det.may_auto_wipe(&text));
        }
        let predicate_cost = started.elapsed();

        let started = std::time::Instant::now();
        for _ in 0..RUNS {
            assert!(!ranked(&text));
        }
        let ranked_cost = started.elapsed();

        eprintln!("predicate {predicate_cost:?} vs ranked scan {ranked_cost:?} over {RUNS} runs");
        assert!(
            predicate_cost <= ranked_cost,
            "the predicate costs more than the ranked scan it replaced \
             ({predicate_cost:?} vs {ranked_cost:?})"
        );
    }

    /// The severity index is derived, never listed: a rule added above the
    /// floor must join the predicate without anyone remembering to add it, and
    /// a rule that refuses deletion must leave it the same way.
    #[test]
    fn the_predicate_index_is_derived_from_the_rule_table() {
        let det = detector();
        assert_eq!(det.severities.len(), det.rules.len());
        for (rule, severity) in det.rules.iter().zip(&det.severities) {
            let expected = match (
                rule.spec.confidence >= AUTOWIPE_CONFIDENCE_FLOOR,
                rule.spec.never_auto_delete,
            ) {
                (true, false) => Severity::HighConfidence,
                (true, true) => Severity::Restricted,
                (false, _) => Severity::Flag,
            };
            assert_eq!(*severity, expected, "{}", rule.spec.name);
            assert_eq!(
                *severity,
                rule.spec.finding().severity,
                "{}",
                rule.spec.name
            );
        }
    }

    /// The restricted band is the one place the two predicates disagree, and the
    /// disagreement is the whole point: a validated card is withheld from the
    /// index and never deleted. Derived from the table so the pin cannot go
    /// stale against a rule that changes bands.
    #[test]
    fn withholding_and_deleting_are_the_same_gate_except_for_the_restricted_band() {
        let det = detector();
        let restricted: Vec<_> = det
            .rules
            .iter()
            .zip(&det.severities)
            .filter(|(_, severity)| **severity == Severity::Restricted)
            .map(|(rule, _)| rule.spec.name)
            .collect();
        assert_eq!(restricted, ["credit_card"]);
    }

    /// I8: no silent drops. Name, category and confidence travel *with* the
    /// compiled rule, so a partial compile cannot shift indices.
    #[test]
    fn rule_count_parity() {
        let det = detector();
        assert_eq!(det.rules.len(), RULES.len());
        assert_eq!(det.set.len(), RULES.len());
        for (i, r) in det.rules.iter().enumerate() {
            assert_eq!(r.spec.name, RULES[i].name);
        }
    }

    /// A rule that declares a secret group and does not get one is unjudgeable:
    /// its entropy threshold and its stopwords were written against the value,
    /// and the whole match carries the keyword and the context anchor as well.
    /// Scoring those as if they were the credential is how a placeholder clears
    /// an entropy floor, so the fallback is `None` and the candidate is dropped.
    #[test]
    fn an_absent_secret_group_never_falls_back_to_the_whole_match() {
        let regex = Regex::new("key=(?:(alpha)|beta)").unwrap();

        let taken = regex.captures("key=alpha").unwrap();
        assert_eq!(secret(&taken, 1, "key=alpha"), Some("alpha"));

        let skipped = regex.captures("key=beta").unwrap();
        assert_eq!(secret(&skipped, 1, "key=beta"), None);
        assert_eq!(secret(&skipped, 0, "key=beta"), Some("key=beta"));
    }

    /// `all` over nothing is true, so an `All` allowlist carrying neither a
    /// regex nor a stopword would suppress every match of the rule it is
    /// attached to — the rule would stop flagging and stop masking with nothing
    /// to say so. Allowlists fail closed.
    #[test]
    fn an_allowlist_with_nothing_to_check_allows_nothing() {
        for condition in [AllowlistCondition::Any, AllowlistCondition::All] {
            let empty = [CompiledAllowlist {
                condition,
                target: AllowlistTarget::Secret,
                regexes: Vec::new(),
                stopwords: Vec::new(),
            }];
            assert!(!is_allowed(&empty, "s3cret", "s3cret", "s3cret"));
        }

        let both = [CompiledAllowlist {
            condition: AllowlistCondition::All,
            target: AllowlistTarget::Secret,
            regexes: vec![Regex::new("^ex").unwrap()],
            stopwords: vec!["ample".to_string()],
        }];
        assert!(is_allowed(&both, "example", "example", "example"));
        assert!(!is_allowed(&both, "exabcdef", "exabcdef", "exabcdef"));
        assert!(!is_allowed(&both, "sample", "sample", "sample"));
    }

    #[test]
    fn word_anchors_reject_glued_tokens() {
        let det = detector();
        let sg = format!("SG.{}.{}", noise(22, ALNUM), noise(43, ALNUM));
        assert!(fired(&det, &sg, "sendgrid_api_key"));
        assert!(!fired(&det, &format!("XX{sg}"), "sendgrid_api_key"));
        assert!(!fired(&det, "mykeyeyJabc.eyJdef.ghi", "jwt"));
    }

    /// §5.2 / §8.1.2: the context anchors are the reason these rules may
    /// auto-delete at all. Without the anchor, nothing.
    #[test]
    fn context_anchored_rules_do_not_match_without_their_anchor() {
        let det = detector();
        let blob = noise(86, BASE64);
        assert!(!det.is_sensitive(&format!("{blob}==")));
        assert!(det.is_sensitive(&format!("AccountKey={blob}==")));

        let token = noise(40, ALNUM);
        assert!(!det.is_sensitive(&token));
        assert!(det.is_sensitive(&format!("CLOUDFLARE_API_TOKEN={token}")));

        let uuid = "550e8400-e29b-41d4-a716-446655440000";
        assert!(!det.is_sensitive(uuid));
        assert!(det.is_sensitive(&format!("heroku config: {uuid}")));
    }

    #[test]
    fn upstream_entropy_and_content_allowlists_filter_candidates() {
        let det = detector();
        assert!(!fired(&det, "credential = aaaaaaaaaaaa", "generic_api_key"));
        assert!(fired(&det, "credential = aB3dE5gH7jK9", "generic_api_key"));
        assert!(!fired(
            &det,
            "credential = about7X9kLmPq",
            "generic_api_key"
        ));
        assert!(!fired(&det, "credential = AbCdEfGhIjKl", "generic_api_key"));
        assert!(!fired(
            &det,
            "credential=abc123XYZ987 --mount=type=secret,",
            "generic_api_key"
        ));
        assert!(!fired(
            &det,
            "AIzaSyabcdefghijklmnopqrstuvwxyz1234567",
            "google_api_key"
        ));
    }

    #[test]
    fn inline_suppression_applies_only_to_upstream_backed_rules() {
        let det = detector();
        assert!(!fired(
            &det,
            "credential=abc123XYZ987 # gitleaks:allow",
            "generic_api_key"
        ));
        assert!(fired(&det, "alice@example.com # gitleaks:allow", "email"));
    }

    /// §7.2: rank by confidence, not by declaration index. v1 returned the
    /// lowest index, so this exact input was labelled "email".
    #[test]
    fn scan_ranks_by_confidence_not_declaration_order() {
        let det = detector();
        let text = format!(
            "mail alice@example.com the token atlasv1.{}",
            noise(64, ALNUM)
        );
        let names = all_rules(&det, &text);
        assert!(names.contains(&"email"));
        assert!(names.contains(&"terraform_cloud_token"));
        assert_eq!(det.scan(&text).unwrap().rule, "terraform_cloud_token");
    }

    #[test]
    fn scan_all_returns_every_validated_match_in_text_order() {
        let det = detector();
        let text = "first@example.com then AKIAIOSFODNN7EXAMPLE then second@example.com";
        let findings = det.scan_all(text);

        assert_eq!(
            findings
                .iter()
                .map(|finding| finding.rule)
                .collect::<Vec<_>>(),
            ["email", "aws_access_key", "email"]
        );
        assert_eq!(
            findings
                .iter()
                .map(|finding| &text[finding.byte_range()])
                .collect::<Vec<_>>(),
            [
                "first@example.com",
                "AKIAIOSFODNN7EXAMPLE",
                "second@example.com"
            ]
        );
        assert_eq!(findings[0].severity, Severity::Flag);
        assert_eq!(findings[1].severity, Severity::HighConfidence);
    }

    #[test]
    fn scan_all_keeps_low_findings_when_a_high_match_is_embedded() {
        let det = detector();
        let text = "mail alice@example.com token AKIAIOSFODNN7EXAMPLE";
        let findings = det.scan_all(text);

        assert_eq!(findings.len(), 2);
        assert!(findings.iter().any(|finding| finding.rule == "email"));
        assert!(findings
            .iter()
            .any(|finding| finding.rule == "aws_access_key"));
        assert!(det.may_auto_wipe(text));
        assert!(!det.may_auto_wipe("mail alice@example.com"));
        assert!(det.is_sensitive(text));
        assert!(!det.is_sensitive("mail alice@example.com"));
    }

    #[test]
    fn scan_all_iterates_past_invalid_candidates_for_the_same_rule() {
        let det = detector();
        let text = "password=short password=hunter2 password=abcdefghij";
        let findings: Vec<_> = det
            .scan_all(text)
            .into_iter()
            .filter(|finding| finding.rule == "generic_password_kv")
            .collect();

        assert_eq!(findings.len(), 2);
        assert_eq!(&text[findings[0].byte_range()], "password=hunter2");
        assert_eq!(&text[findings[1].byte_range()], "password=abcdefghij");
    }

    #[test]
    fn scan_all_offsets_index_the_nfkc_normalised_string() {
        let det = detector();
        let input = "\u{FF21}\u{FF2B}\u{FF29}\u{FF21}IOSFODNN7EXAMPLE";
        let normalised = normalise(input);
        let findings = det.scan_all(input);

        assert_eq!(normalised, "AKIAIOSFODNN7EXAMPLE");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].byte_range(), 0..normalised.len());
        assert_eq!(&normalised[findings[0].byte_range()], normalised);
    }

    /// §3.3's known gap in v1 was that the card check produced no span, so a
    /// card embedded in benign text could never be masked. It is a first-class
    /// rule here, and the restricted band is what lets the span exist without
    /// the item becoming deletable.
    #[test]
    fn luhn_valid_card_is_a_spanned_restricted_finding() {
        let det = detector();
        let text = "please charge 4111-1111-1111-1111 today";
        let finding = det
            .scan_all(text)
            .into_iter()
            .find(|finding| finding.rule == "credit_card")
            .unwrap();

        assert_eq!(&text[finding.byte_range()], "4111-1111-1111-1111");
        assert_eq!(finding.category, "financial");
        assert_eq!(finding.confidence, 0.99);
        assert_eq!(finding.severity, Severity::Restricted);
        assert!(det.is_sensitive(text));
        assert!(!det.may_auto_wipe(text));
        assert!(det.redact(text).contains("***REDACTED***"));
    }

    #[test]
    fn findings_carry_their_category() {
        let det = detector();
        assert_eq!(
            det.scan("AKIAIOSFODNN7EXAMPLE").unwrap().category,
            "credential"
        );
        assert_eq!(det.scan("4111111111111111").unwrap().category, "financial");
        assert_eq!(
            det.scan("Send to alice@example.com").unwrap().category,
            "personal_id"
        );
        assert_eq!(
            det.scan("172.16.0.5:6379").unwrap().category,
            "infrastructure"
        );
    }

    #[test]
    fn empty_and_whitespace_input_is_not_sensitive() {
        let det = detector();
        for input in ["", " ", "\n\n\t", "…"] {
            assert!(!det.is_sensitive(input));
            assert!(det.scan(input).is_none());
        }
    }

    /// §9.3 perf pin: no rule may be catastrophically backtracking.
    ///
    /// Asserted as a **ratio against a small-input baseline measured on the same
    /// machine**, not as a wall-clock budget. `regex` is linear-time by
    /// construction, so a sane ruleset scans 100× the text in about 100× the
    /// time, while a backtracking rule blows that up by orders of magnitude —
    /// which is what the 20× slack leaves room to catch. The previous form was
    /// an absolute 5-second bound with roughly 30 % headroom in a debug build,
    /// so it went red whenever the machine was busy, and an intermittently red
    /// test is one people learn to ignore. The manifest's real 10 MB / 500 ms
    /// budget is a release-build benchmark and does not belong in a unit test.
    #[test]
    fn ruleset_scan_cost_stays_linear_in_input_size() {
        const UNIT: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ";
        const SMALL_REPS: usize = 40;
        const FACTOR: u32 = 100;
        const RUNS: u32 = 10;
        const SLACK: u32 = 20;

        let det = detector();
        let small = UNIT.repeat(SMALL_REPS);
        let large = UNIT.repeat(SMALL_REPS * FACTOR as usize);

        // Warm up first, so neither measurement pays for building the lazy DFA.
        assert!(!det.is_sensitive(&small));

        let started = std::time::Instant::now();
        for _ in 0..RUNS {
            assert!(!det.is_sensitive(&small));
        }
        let per_small = started.elapsed() / RUNS;

        let started = std::time::Instant::now();
        assert!(!det.is_sensitive(&large));
        let elapsed = started.elapsed();

        let linear = per_small * FACTOR;
        assert!(
            elapsed < linear * SLACK,
            "scan cost is superlinear: {elapsed:?} for {FACTOR}x the text that scanned \
             in {per_small:?} (linear would be about {linear:?})"
        );
    }

    /// The prefilter must be faster than the work it replaces.
    ///
    /// [`PREFILTER_DFA_SIZE_LIMIT`] is the only thing making that true: at
    /// `regex`'s 2 MiB default the combined automaton does not fit its lazy DFA
    /// cache, every search degrades to the NFA simulation, and one `RegexSet`
    /// pass costs ~55× all 45 individual searches together. Nothing else in the
    /// suite notices — every verdict stays correct — so this is the test that
    /// fails if the limit is "simplified" away.
    ///
    /// A ratio against a same-machine baseline rather than a wall-clock budget,
    /// for the reason [`ruleset_scan_cost_stays_linear_in_input_size`] gives.
    /// The real ratio is ~7× in the prefilter's favour; asserting merely
    /// *no worse than* the serial baseline leaves an order of magnitude of slack
    /// and still catches a 55× regression.
    #[test]
    fn the_prefilter_is_faster_than_the_searches_it_replaces() {
        const UNIT: &str = "ordinary paragraph of notes, about sixty characters long.\n";
        const RUNS: u32 = 5;

        let det = detector();
        let serial: Vec<_> = RULES
            .iter()
            .map(|spec| Regex::new(spec.pattern).unwrap())
            .collect();
        let text = UNIT.repeat(2_000);

        // Warm both lazy DFAs before either is timed.
        assert!(!det.is_sensitive(&text));
        assert!(!serial.iter().any(|re| re.is_match(&text)));

        let started = std::time::Instant::now();
        for _ in 0..RUNS {
            assert!(!det.is_sensitive(&text));
        }
        let prefiltered = started.elapsed();

        let started = std::time::Instant::now();
        for _ in 0..RUNS {
            assert!(!serial.iter().any(|re| re.is_match(&text)));
        }
        let one_at_a_time = started.elapsed();

        assert!(
            prefiltered <= one_at_a_time,
            "the prefilter costs more than running every rule separately \
             ({prefiltered:?} vs {one_at_a_time:?}) — check PREFILTER_DFA_SIZE_LIMIT"
        );
    }

    #[test]
    fn secret_embedded_in_a_large_benign_document_is_still_found() {
        let det = detector();
        let mut haystack = "ordinary notes about the release\n".repeat(2_000);
        haystack.push_str("AKIAIOSFODNN7EXAMPLE\n");
        haystack.push_str(&"more ordinary notes\n".repeat(2_000));
        assert_eq!(det.scan(&haystack).unwrap().rule, "aws_access_key");
    }
}

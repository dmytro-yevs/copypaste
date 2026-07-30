//! The compiled ruleset and the two verdicts callers may ask for:
//! [`super::rules::RULES`] compiled behind a `RegexSet` prefilter, each hit put
//! through its [`super::validators`] gate, survivors ranked into a
//! [`Finding`](super::finding::Finding).

use regex::{Regex, RegexSet, RegexSetBuilder};

use super::finding::{Finding, Severity};
use super::normalise::normalise;
use super::rules::RULES;
use super::spec::{RuleSpec, Validator};
use super::validators::{luhn_valid, phone_is_formatted, ssn_structure_plausible, value_is_strong};

/// Regex compilation failures. No variant carries a path or any input text
/// (CLAUDE.md rule 4).
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
}

/// Lazy-DFA cache ceiling for the prefilter, and the difference between the
/// prefilter being an optimisation and being a 55× pessimisation.
///
/// `regex` defaults to 2 MiB. The combined automaton for these ~45 patterns does
/// not fit in it: the cache thrashes, every search falls back to the NFA
/// simulation, and the one pass that is supposed to save 45 individual searches
/// costs more than all 45 together. Measured on 116 KB of benign ASCII —
/// 2 MiB: 1.0 MB/s · 4 MiB: 400 MB/s · flat to 32 MiB; running the 45 regexes
/// serially instead: 59 MB/s. 8 MiB is past the knee with room for the ruleset
/// to grow. The cache is allocated on demand, so this is a ceiling and not a
/// cost.
const PREFILTER_DFA_SIZE_LIMIT: usize = 8 * 1024 * 1024;

/// The compiled ruleset. Construct **once** and share it: `new()` compiles ~42
/// regexes plus the prefilter, and this runs on the clipboard hot path
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
            rules.push(Rule { spec, regex });
        }
        let set = RegexSetBuilder::new(RULES.iter().map(|r| r.pattern))
            .dfa_size_limit(PREFILTER_DFA_SIZE_LIMIT)
            .build()
            .map_err(DetectorError::RuleSet)?;
        Ok(Self { set, rules })
    }

    /// Highest-confidence match, or None.
    pub fn scan(&self, text: &str) -> Option<Finding> {
        let normalised = normalise(text);
        let mut best: Option<(&RuleSpec, usize)> = None;
        for idx in self.set.matches(&normalised) {
            let rule = &self.rules[idx];
            let Some(len) = rule.hit(&normalised) else {
                continue;
            };
            let better = match best {
                None => true,
                Some((spec, best_len)) => (rule.spec.confidence, len) > (spec.confidence, best_len),
            };
            if better {
                best = Some((rule.spec, len));
            }
        }
        best.map(|(spec, _)| spec.finding())
    }

    /// True when the text must be kept out of the search index: any validated
    /// match at any confidence, including the inert band — an email address is
    /// not worth deleting but is not worth writing to a plaintext FTS table
    /// either (manifest I4).
    ///
    /// **Not** the auto-wipe gate; deletion needs
    /// `scan(..).severity == Severity::HighConfidence`.
    /// True when this text may be deleted automatically: the highest-confidence
    /// match sits above the auto-wipe floor.
    ///
    /// The one gate between detection and destruction, and the only caller of
    /// [`Severity::HighConfidence`] that deletes anything. v1 collapsed this
    /// with [`Detector::is_sensitive`] three separate times (`AB-6a`, `PG-23`,
    /// `PG-3`) and destroyed unrecoverable user data each time.
    pub fn may_auto_wipe(&self, text: &str) -> bool {
        self.scan(text)
            .is_some_and(|finding| finding.severity == Severity::HighConfidence)
    }

    pub fn is_sensitive(&self, text: &str) -> bool {
        let normalised = normalise(text);
        self.set
            .matches(&normalised)
            .into_iter()
            .any(|idx| self.rules[idx].hit(&normalised).is_some())
    }
}

/// One table entry, compiled.
struct Rule {
    spec: &'static RuleSpec,
    regex: Regex,
}

impl Rule {
    /// Length of the first match that passes this rule's validator. No separate
    /// fast path: v1's `RegexSet` shortcut skipped the value-strength validator
    /// and needed a bespoke `generic_password_kv` case to compensate (§5.3).
    fn hit(&self, text: &str) -> Option<usize> {
        for caps in self.regex.captures_iter(text) {
            let Some(whole) = caps.get(0) else { continue };
            let ok = match self.spec.validator {
                Validator::None => true,
                Validator::ValueStrength => {
                    caps.get(1).is_some_and(|v| value_is_strong(v.as_str()))
                }
                Validator::Luhn => luhn_valid(whole.as_str()),
                Validator::SsnStructure => ssn_structure_plausible(whole.as_str()),
                Validator::PhoneShape => phone_is_formatted(whole.as_str()),
            };
            if ok {
                return Some(whole.len());
            }
        }
        None
    }
}

/// Fixtures and helpers shared by every test module under `sensitive`. Here
/// because [`all_rules`] needs `Detector`'s private fields.
#[cfg(test)]
pub(super) mod test_support {
    use super::super::normalise::normalise;
    use super::Detector;

    pub(in crate::sensitive) fn detector() -> Detector {
        Detector::new().expect("ruleset compiles")
    }

    /// All validated matches, ranked. Test-only: production has exactly one
    /// verdict function and one label function (§7.4 — v1 shipped four
    /// overlapping "is it sensitive?" entry points, three of them dead).
    pub(in crate::sensitive) fn all_rules(det: &Detector, text: &str) -> Vec<&'static str> {
        let normalised = normalise(text);
        det.set
            .matches(&normalised)
            .into_iter()
            .filter(|&i| det.rules[i].hit(&normalised).is_some())
            .map(|i| det.rules[i].spec.name)
            .collect()
    }

    pub(in crate::sensitive) fn fired(det: &Detector, text: &str, rule: &str) -> bool {
        all_rules(det, text).contains(&rule)
    }

    pub(in crate::sensitive) fn rep(c: char, n: usize) -> String {
        std::iter::repeat_n(c, n).collect()
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
    ];
}

#[cfg(test)]
mod tests {
    use super::test_support::{all_rules, detector, fired, rep, BENIGN_CORPUS};
    use super::*;
    use crate::sensitive::finding::{Severity, AUTOWIPE_CONFIDENCE_FLOOR};

    // -- §9.1 true positives ------------------------------------------------

    /// Every entry here must be detected. The `Some(rule)` column, where
    /// present, pins *which* rule wins the confidence ranking (§7.2).
    #[test]
    fn manifest_true_positives_are_detected() {
        let det = detector();
        let ghp = format!("ghp_{}", rep('A', 36));
        let fine = format!("github_pat_{}_{}", rep('A', 22), rep('B', 59));
        let openai_new = format!("sk-proj-{}", rep('A', 48));
        let openai_legacy = format!("sk-{}", rep('A', 48));
        let anthropic = format!("sk-ant-api03-{}", rep('A', 80));
        let stripe = format!("sk_live_{}", rep('A', 24));
        let npm = format!("npm_{}", rep('A', 36));
        let slack_hook = format!(
            "https://hooks.slack.com/services/T00000000/B00000000/{}",
            rep('X', 24)
        );
        let vault = format!("hvs.{}", rep('A', 32));
        let sendgrid = format!("SG.{}.{}", rep('A', 22), rep('B', 43));
        let terraform = format!("atlasv1.{}", rep('A', 64));
        let azure = format!("AccountKey={}==", rep('A', 86));

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
            format!("sk-{}", rep('A', 48)),
            format!("hvs.{}", rep('A', 32)),
            format!("SG.{}.{}", rep('A', 22), rep('B', 43)),
            format!("atlasv1.{}", rep('A', 64)),
            format!("AccountKey={}==", rep('A', 86)),
            "-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEA".to_string(),
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c"
                .to_string(),
            "4111111111111111".to_string(),
            "{\"private_key\": \"-----BEGIN RSA PRIVATE KEY-----\\nMIIEo\"}".to_string(),
        ];
        for input in &inputs {
            let f = det.scan(input).unwrap_or_else(|| panic!("{input:?}"));
            assert_eq!(f.severity, Severity::HighConfidence, "{input:?} -> {f:?}");
            assert!(f.confidence >= AUTOWIPE_CONFIDENCE_FLOOR);
        }
    }

    /// The `sk-proj-` exclusion is structural, not a lookahead (P2 `r6cw`).
    #[test]
    fn openai_legacy_does_not_double_fire_on_proj_keys() {
        let det = detector();
        let key = format!("sk-proj-{}", rep('A', 48));
        assert!(fired(&det, &key, "openai_new"));
        assert!(!fired(&det, &key, "openai_legacy"));
    }

    // -- §9.2 false positives ------------------------------------------------

    #[test]
    fn benign_corpus_has_zero_false_positives() {
        let det = detector();
        let offenders: Vec<_> = BENIGN_CORPUS
            .iter()
            .filter(|t| det.is_sensitive(t))
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
            rep('A', 40),
            format!("{}==", rep('A', 86)),
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
                !det.is_sensitive(input),
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
                !det.is_sensitive(input),
                "{input:?} -> {:?}",
                all_rules(&det, input)
            );
        }
    }

    /// §9.2's inert band: detected, labelled, kept out of the index — never
    /// deleted. This is manifest I1, the prime directive.
    #[test]
    fn inert_band_is_detected_but_never_auto_wipes() {
        let det = detector();
        let twilio = format!("SK{}", rep('a', 32));
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
            // detected and kept out of the index …
            assert!(det.is_sensitive(input), "{input:?}");
            // … but inert for deletion
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

    #[test]
    fn word_anchors_reject_glued_tokens() {
        let det = detector();
        let sg = format!("SG.{}.{}", rep('A', 22), rep('B', 43));
        assert!(fired(&det, &sg, "sendgrid_api_key"));
        assert!(!fired(&det, &format!("XX{sg}"), "sendgrid_api_key"));
        assert!(!fired(&det, "mykeyeyJabc.eyJdef.ghi", "jwt"));
    }

    /// §5.2 / §8.1.2: the context anchors are the reason these rules may
    /// auto-delete at all. Without the anchor, nothing.
    #[test]
    fn context_anchored_rules_do_not_match_without_their_anchor() {
        let det = detector();
        let blob = rep('A', 86);
        assert!(!det.is_sensitive(&format!("{blob}==")));
        assert!(det.is_sensitive(&format!("AccountKey={blob}==")));

        let token = rep('b', 40);
        assert!(!det.is_sensitive(&token));
        assert!(det.is_sensitive(&format!("CLOUDFLARE_API_TOKEN={token}")));

        let uuid = "550e8400-e29b-41d4-a716-446655440000";
        assert!(!det.is_sensitive(uuid));
        assert!(det.is_sensitive(&format!("heroku config: {uuid}")));
    }

    /// §7.2: rank by confidence, not by declaration index. v1 returned the
    /// lowest index, so this exact input was labelled "email".
    #[test]
    fn scan_ranks_by_confidence_not_declaration_order() {
        let det = detector();
        let text = format!("mail alice@example.com the token atlasv1.{}", rep('A', 64));
        let names = all_rules(&det, &text);
        assert!(names.contains(&"email"));
        assert!(names.contains(&"terraform_cloud_token"));
        assert_eq!(det.scan(&text).unwrap().rule, "terraform_cloud_token");
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

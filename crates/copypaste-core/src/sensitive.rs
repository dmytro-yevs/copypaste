//! Sensitive-content (secret) detection.
//!
//! Ported from `docs/rewrite/port-manifest/07-secret-detection.md`, which is the
//! binding specification for this module. Section references (`§3.2`, `§5.3`,
//! `§7.1`, …) in the comments below point into that document; every
//! `CopyPaste-*` / `P2 *` / `Audit MED #*` note records a real production miss
//! or a real deletion of user data (§8.1.5 — carry the *reasons* forward even
//! when the regex is replaced).
//!
//! # The two decisions, kept separate (manifest I2)
//!
//! v1 collapsed "is this a secret?" and "may we delete it?" into one predicate
//! three separate times (bugs `AB-6a`, `PG-23`, `PG-3`). Here they are carried
//! by two different things:
//!
//! * [`Detector::is_sensitive`] — *any* validated rule matched. Gates the
//!   search index (manifest I4 / ADR-015: `is_sensitive = 1` ⇒ never written to
//!   FTS, never returned by search). Deliberately inclusive: keeping a phone
//!   number out of a plaintext index costs nothing.
//! * [`Severity`] on the [`Finding`] — only `HighConfidence` (confidence ≥ the
//!   0.70 auto-wipe floor) may drive automatic deletion. Everything else is
//!   `Flag`: detected, labelled, maskable, **inert** for deletion.
//!
//! CLAUDE.md rule 4 and manifest I1 both say the same thing: a false positive
//! silently destroys unrecoverable user data 30 seconds after it was copied.
//! When a shape is not distinctive enough to *prove* a secret, it stays below
//! the floor.
//!
//! # Ranking
//!
//! [`Detector::scan`] returns the **highest-confidence** match (manifest §7.2:
//! v1 returned the lowest *declaration index*, so text containing both an email
//! and a Terraform token was labelled "email"; declaration order must not be
//! semantic). Ties break on longer match, then on table order — all three are
//! deterministic, none is meaningful.
//!
//! # Deliberately not in this file
//!
//! * Spans / redaction (manifest §1.1, I9) — no consumer yet; adding an unused
//!   span API would recreate §7.4's three-dead-entry-points problem.
//! * The password-manager bundle-ID list (§5.8) — it is a *capture-time* signal
//!   about the source app, not about content, and §5.8 says v2 should carry it
//!   as configuration rather than code.
//! * The telemetry scrubber's variety gate (§5.5) — §7.5 is explicit that v2
//!   must not maintain a second regex engine. The variety heuristic that does
//!   belong to content detection lives in [`value_is_strong`].

use std::borrow::Cow;

use regex::{Regex, RegexSet};
use unicode_normalization::UnicodeNormalization;

/// Manifest §4.1. Nothing may sit *exactly* on the floor: `ip_with_port` did,
/// and RFC1918 addresses in docker-compose snippets were silently auto-wiped
/// (`CopyPaste-8ys1`). Pinned by `no_rule_sits_exactly_on_the_floor`.
const AUTOWIPE_CONFIDENCE_FLOOR: f32 = 0.70;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// What a match means for automatic deletion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Flag and keep out of the index, but never auto-delete.
    Flag,
    /// Above the auto-wipe floor.
    HighConfidence,
}

/// The highest-confidence rule that matched.
#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    /// Stable rule id, e.g. `aws_access_key`. The label belongs to the rule
    /// definition — v1 derived it by string-prefix dispatch on the name, so
    /// renaming a rule silently changed its label (manifest §7.8).
    pub rule: String,
    /// One of `credential`, `financial`, `personal_id`, `infrastructure`.
    pub category: String,
    pub confidence: f32,
    pub severity: Severity,
}

/// Regex compilation failures.
///
/// No variant carries a path or any input text: errors reach users, and the
/// daemon socket path discloses the local username (CLAUDE.md rule 4).
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

/// The compiled ruleset.
///
/// Construct **once** and share it: `new()` compiles ~42 regexes plus the
/// prefilter, and this runs on the clipboard hot path (`CopyPaste-mnte`: v1
/// built the detector once per history page, not once per item).
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
    /// Compile the ruleset. Fallible only in the sense that a typo in a
    /// literal would be caught here; `ruleset_compiles` pins that it does not
    /// happen in a shipped build.
    pub fn new() -> Result<Self, DetectorError> {
        let mut rules = Vec::with_capacity(RULES.len());
        for spec in RULES {
            let regex = Regex::new(spec.pattern).map_err(|source| DetectorError::Rule {
                rule: spec.name,
                source,
            })?;
            rules.push(Rule { spec, regex });
        }
        let set = RegexSet::new(RULES.iter().map(|r| r.pattern)).map_err(DetectorError::RuleSet)?;
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

    /// True when the text must be kept out of the search index.
    ///
    /// Any validated match at any confidence, including the inert band — an
    /// email address is not worth deleting but is still not worth writing to a
    /// plaintext FTS table (manifest I4).
    ///
    /// This is **not** the auto-wipe gate. Callers deciding whether to delete
    /// must use `scan(..).severity == Severity::HighConfidence`.
    pub fn is_sensitive(&self, text: &str) -> bool {
        let normalised = normalise(text);
        self.set
            .matches(&normalised)
            .into_iter()
            .any(|idx| self.rules[idx].hit(&normalised).is_some())
    }
}

// ---------------------------------------------------------------------------
// Normalisation (manifest I3 / §5.1)
// ---------------------------------------------------------------------------

/// NFKC-normalise before matching.
///
/// Without this, `Ａ` (U+FF21 FULLWIDTH LATIN CAPITAL A) and every other
/// compatibility form renders as ASCII but bypasses every ASCII character
/// class. All offsets, if this module ever returns any, belong to the
/// normalised string.
///
/// NFKC is the identity on ASCII (§5.1), so ASCII input — the overwhelming
/// majority of clipboard traffic — skips the pass and the allocation entirely.
fn normalise(text: &str) -> Cow<'_, str> {
    if text.is_ascii() {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(text.nfkc().collect())
    }
}

// ---------------------------------------------------------------------------
// Rule table
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Category {
    Credential,
    Financial,
    PersonalId,
    Infrastructure,
}

impl Category {
    fn as_str(self) -> &'static str {
        match self {
            Category::Credential => "credential",
            Category::Financial => "financial",
            Category::PersonalId => "personal_id",
            Category::Infrastructure => "infrastructure",
        }
    }
}

/// Post-match validators. Manifest §5.3/§5.4 plus the two v2 additions called
/// for in §7.1 and §7.7.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Validator {
    /// The regex is the whole decision.
    None,
    /// Capture group 1 must survive [`value_is_strong`].
    ValueStrength,
    /// Luhn checksum over the digit run (§5.4).
    Luhn,
    /// SSN group-structure check (§4.2 — "the correct fix is the structural
    /// validator").
    SsnStructure,
    /// The match must actually be *formatted* like a phone number (§7.7 — the
    /// benign-corpus entry `the order number is 1234567890`).
    PhoneShape,
}

struct RuleSpec {
    name: &'static str,
    category: Category,
    confidence: f32,
    pattern: &'static str,
    validator: Validator,
}

impl RuleSpec {
    fn finding(&self) -> Finding {
        Finding {
            rule: self.name.to_string(),
            category: self.category.as_str().to_string(),
            confidence: self.confidence,
            severity: if self.confidence >= AUTOWIPE_CONFIDENCE_FLOOR {
                Severity::HighConfidence
            } else {
                Severity::Flag
            },
        }
    }
}

struct Rule {
    spec: &'static RuleSpec,
    regex: Regex,
}

impl Rule {
    /// Length of the first match that passes this rule's validator.
    ///
    /// There is no separate fast/slow path: v1's `RegexSet` shortcut skipped
    /// the value-strength validator and needed a bespoke `generic_password_kv`
    /// special case to compensate (§5.3, `engine.rs:107-117`). Validating on
    /// the one and only path removes the class of bug.
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

/// The ruleset (manifest §3.2, §3.3 and the gitleaks mapping in §8).
///
/// Declaration order is **not** semantic (§7.2). It is grouped by vendor purely
/// so a human can read it.
///
/// `⚠ INERT` in a comment marks a rule deliberately tuned below the 0.70
/// auto-wipe floor. Two different reasons live in that band (§4.2): data the
/// user legitimately owns and meant to copy (IBAN, SSN, email, phone,
/// passport), and shapes too weak to prove a secret (`discord_bot_token`,
/// `twilio_signing_key_sid`, `generic_bearer`, `ip_with_port`).
///
/// Where §8 names a gitleaks rule that is compatible with the manifest's
/// acceptance tests, the gitleaks form is used and its rule id is cited.
/// Where adopting it would fail an acceptance test in §9.1, the v1 form is
/// kept and the conflict is written down — §8 is explicit that its table is
/// "a starting point, not a contract", while §9 is a requirements list.
static RULES: &[RuleSpec] = &[
    // -- AWS ---------------------------------------------------------------
    RuleSpec {
        // gitleaks `aws-access-token`. Broader than v1's `AKIA|ASIA`: a recall
        // win (§8 row 0). Leading `\b` stops mid-token hits (`XAKIA…`);
        // **no trailing `\b` on purpose** — ASIA temp keys carry trailing
        // digits and `E1` has no boundary between two word chars (§5.2,
        // "deliberate omissions … must not be fixed blindly").
        name: "aws_access_key",
        category: Category::Credential,
        confidence: 0.99,
        pattern: r"\b(?:A3T[0-9A-Z]|AKIA|ASIA|ABIA|ACCA)[0-9A-Z]{16}",
        validator: Validator::None,
    },
    RuleSpec {
        // §7.1: an ARN is a *resource identifier*, not credential material —
        // the same reasoning that got `ip_with_port` demoted under
        // `CopyPaste-8ys1`. v1 had it at 0.90, so pasting an ARN into a ticket
        // deleted it 30 s later. Demoted to the inert band. ⚠ INERT
        // gitleaks correctly ships no ARN rule (§8 row 32).
        name: "aws_arn",
        category: Category::Infrastructure,
        confidence: 0.50,
        pattern: r"\barn:aws:[a-z][a-z0-9\-]*:[a-z0-9\-]*:[0-9]{12}:[^\s]+",
        validator: Validator::None,
    },
    // -- GitHub ------------------------------------------------------------
    RuleSpec {
        // gitleaks `github-fine-grained-pat`.
        name: "github_fine_grained",
        category: Category::Credential,
        confidence: 0.99,
        pattern: r"github_pat_[0-9a-zA-Z]{22}_[0-9a-zA-Z]{59}",
        validator: Validator::None,
    },
    RuleSpec {
        // gitleaks `github-pat`.
        name: "github_classic_pat",
        category: Category::Credential,
        confidence: 0.99,
        pattern: r"\bghp_[0-9a-zA-Z]{36}\b",
        validator: Validator::None,
    },
    RuleSpec {
        // gitleaks `github-app-token` covers `ghu_` as well as v1's `ghs_`
        // (§8 row 3 — `ghu_` was a v1 gap).
        name: "github_app_token",
        category: Category::Credential,
        confidence: 0.99,
        pattern: r"\b(?:ghu|ghs)_[0-9a-zA-Z]{36}\b",
        validator: Validator::None,
    },
    RuleSpec {
        // gitleaks `github-oauth` — a v1 gap (§8 row 3).
        name: "github_oauth",
        category: Category::Credential,
        confidence: 0.99,
        pattern: r"\bgho_[0-9a-zA-Z]{36}\b",
        validator: Validator::None,
    },
    RuleSpec {
        // gitleaks `github-refresh-token` — a v1 gap (§8 row 3).
        name: "github_refresh_token",
        category: Category::Credential,
        confidence: 0.99,
        pattern: r"\bghr_[0-9a-zA-Z]{36}\b",
        validator: Validator::None,
    },
    // -- AI vendors --------------------------------------------------------
    RuleSpec {
        // gitleaks `openai-api-key` anchors on the `T3BlbkFJ` infix, which is
        // far more precise (§8 row 4/5) — but §9.1 requires `sk-proj-` + 48×A
        // to be detected, and that fixture has no infix. v1 form kept; revisit
        // together with the fixture.
        name: "openai_new",
        category: Category::Credential,
        confidence: 0.99,
        pattern: r"sk-proj-[A-Za-z0-9]{48}",
        validator: Validator::None,
    },
    RuleSpec {
        // Must NOT double-fire on `sk-proj-` keys. The exclusion is
        // **structural, not lookahead** (the `regex` crate has none): the
        // hyphen after `proj` breaks the contiguous 48-char alnum run. The old
        // "(?!proj-) lookahead" comment was wrong and was corrected in P2 `r6cw`.
        name: "openai_legacy",
        category: Category::Credential,
        confidence: 0.95,
        pattern: r"\bsk-[A-Za-z0-9]{48}\b",
        validator: Validator::None,
    },
    RuleSpec {
        // gitleaks `anthropic-api-key` / `anthropic-admin-api-key` require the
        // full 93-char body + `AA` suffix; §9.1's fixture is `sk-ant-api03-` +
        // 80×A, so the v1 length form is kept. `admin` added to close the
        // admin-key gap named in §8 row 6.
        name: "anthropic",
        category: Category::Credential,
        confidence: 0.99,
        pattern: r"sk-ant-(?:api|admin)\d{2}-[A-Za-z0-9_-]{80,}",
        validator: Validator::None,
    },
    // -- Payments / SaaS ---------------------------------------------------
    RuleSpec {
        // gitleaks `stripe-access-token` also covers `sk_test_`; v1 excluded
        // test keys deliberately and that stays — test keys live in public
        // docs and deleting them is pure cost. `rk_` and `prod` adopted.
        name: "stripe_live",
        category: Category::Credential,
        confidence: 0.99,
        pattern: r"\b(?:sk|rk)_(?:live|prod)_[0-9A-Za-z]{10,99}\b",
        validator: Validator::None,
    },
    RuleSpec {
        // No gitleaks equivalent (§8 row 8) — custom rule.
        name: "stripe_webhook",
        category: Category::Credential,
        confidence: 0.99,
        pattern: r"whsec_[a-zA-Z0-9]{32,64}",
        validator: Validator::None,
    },
    RuleSpec {
        // gitleaks `npm-access-token`.
        name: "npm_token",
        category: Category::Credential,
        confidence: 0.99,
        pattern: r"\bnpm_[A-Za-z0-9]{36}\b",
        validator: Validator::None,
    },
    RuleSpec {
        // gitleaks `pypi-upload-token` anchors the `AgEIcHlwaS5vcmc` payload.
        // Adopted over v1's `{180,}` length-only gate, which §7.8 calls fragile.
        name: "pypi_token",
        category: Category::Credential,
        confidence: 0.99,
        pattern: r"\bpypi-AgEIcHlwaS5vcmc[A-Za-z0-9_-]{50,}",
        validator: Validator::None,
    },
    RuleSpec {
        // gitleaks `slack-bot-token` and friends. v1 matched only `xoxb` and
        // fixed both numeric segments at exactly 11 digits; `xoxa/xoxp/xoxr/
        // xoxs` were named gaps (§3.2 row 11, §8 row 11).
        name: "slack_token",
        category: Category::Credential,
        confidence: 0.99,
        pattern: r"\bxox[baprs]-\d{10,13}-\d{10,20}(?:-\d{10,20})?-[a-zA-Z0-9_-]{24,34}\b",
        validator: Validator::None,
    },
    RuleSpec {
        // gitleaks `slack-webhook-url`.
        name: "slack_webhook",
        category: Category::Credential,
        confidence: 0.99,
        pattern: r"https://hooks\.slack\.com/services/T[A-Z0-9]+/B[A-Z0-9]+/[a-zA-Z0-9]+",
        validator: Validator::None,
    },
    RuleSpec {
        // ⚠ INERT. **P2 fb3e**: lowered 0.85 → 0.65 and `\b`-anchored. The
        // shape fires on any dot-separated base64url triple, not just Discord.
        // gitleaks `discord-api-token` is keyword-anchored on "discord" and is
        // strictly better — but §9.2 requires the bare shape to still be
        // *detected* (inert), which a keyword anchor would not do. Kept
        // sub-floor; adopting the keyword anchor is what would justify raising
        // it (§8 row 13).
        name: "discord_bot_token",
        category: Category::Credential,
        confidence: 0.65,
        pattern: r"\b[MN][a-zA-Z\d]{23,25}\.[\w-]{6}\.[\w-]{27,38}\b",
        validator: Validator::None,
    },
    RuleSpec {
        // ⚠ INERT. **P2 fb3e**: was misnamed `twilio_auth_token`; this shape is
        // a Twilio **Signing-Key SID**, not the auth token. Real auth tokens
        // are bare 32-hex with no prefix and are not regex-distinguishable.
        // gitleaks `twilio-api-key` keyword-anchors; without that anchor this
        // stays below the floor (§8 row 14).
        name: "twilio_signing_key_sid",
        category: Category::Credential,
        confidence: 0.65,
        pattern: r"\bSK[a-f0-9]{32}\b",
        validator: Validator::None,
    },
    RuleSpec {
        // gitleaks `sendgrid-api-token`.
        name: "sendgrid_api_key",
        category: Category::Credential,
        confidence: 0.99,
        pattern: r"\bSG\.[A-Za-z0-9_-]{22}\.[A-Za-z0-9_-]{43}\b",
        validator: Validator::None,
    },
    RuleSpec {
        // gitleaks `terraform-api-token` requires a 14-char org prefix before
        // `.atlasv1.`; §9.1's fixture is a bare `atlasv1.` + 64×A, so the v1
        // form is kept (§8 row 39).
        name: "terraform_cloud_token",
        category: Category::Credential,
        confidence: 0.99,
        pattern: r"\batlasv1\.[A-Za-z0-9_-]{64,}\b",
        validator: Validator::None,
    },
    // -- Cloud providers ---------------------------------------------------
    RuleSpec {
        // gitleaks `gcp-api-key`.
        name: "google_api_key",
        category: Category::Credential,
        confidence: 0.99,
        pattern: r"\bAIza[0-9A-Za-z\-_]{35}",
        validator: Validator::None,
    },
    RuleSpec {
        // gitleaks `gcp-oauth-client-secret`.
        name: "gcp_oauth",
        category: Category::Credential,
        confidence: 0.99,
        pattern: r"\bGOCSPX-[A-Za-z0-9_-]{28}",
        validator: Validator::None,
    },
    RuleSpec {
        // **P2 ozzt.** Only the RSA header inside the JSON field. A service
        // account JSON with a PKCS#8 body trips `private_key` instead — that
        // coverage is incidental, not designed (§3.2 row 36).
        name: "gcp_service_account_key",
        category: Category::Credential,
        confidence: 0.99,
        pattern: r#"(?m)"private_key"\s*:\s*"-----BEGIN RSA PRIVATE KEY-----"#,
        validator: Validator::None,
    },
    RuleSpec {
        // gitleaks `heroku-api-key`. Context-anchored on the word `heroku`
        // within 50 chars of a UUID — the anchor is what makes 0.95 safe,
        // since a bare UUID is not a secret.
        name: "heroku_api_key",
        category: Category::Credential,
        confidence: 0.95,
        pattern: r"(?i)heroku[^\n]{0,50}[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}",
        validator: Validator::None,
    },
    RuleSpec {
        // gitleaks `vault-service-token` / `vault-batch-token`. gitleaks
        // requires 90–100 chars; §9.1 requires `hvs.` + 32×A to be detected
        // *and auto-wipe*, so v1's `{32,}` minimum stays (it was itself added
        // to kill FPs on short `hvs.`-prefixed strings). `hvb.` closes the gap
        // named in §8 row 17.
        name: "hashicorp_vault",
        category: Category::Credential,
        confidence: 0.95,
        pattern: r"\bhv[sb]\.[A-Za-z0-9_-]{32,}",
        validator: Validator::None,
    },
    RuleSpec {
        // **P2 ozzt + bug-hunt HIGH finding.** A bare 88-char base64 blob is
        // indistinguishable from a SHA-512 dump, an Ed25519 key or a random
        // token; matching it bare at 0.90 would silently auto-wipe benign
        // content. The `AccountKey=` context anchor is **mandatory** and is the
        // only reason this rule is allowed above the floor (§5.2, §8.1.2).
        name: "azure_storage_key",
        category: Category::Credential,
        confidence: 0.90,
        pattern: r"AccountKey=[A-Za-z0-9+/]{86}==",
        validator: Validator::None,
    },
    RuleSpec {
        // **P2 ozzt.** Anchors only on the two stable, order-independent
        // markers (`sv=` version and `&sig=`). The previous over-specified form
        // matched almost no real tokens — a recall bug from over-specification.
        name: "azure_sas_token",
        category: Category::Credential,
        confidence: 0.92,
        pattern: r"(?i)\bsv=\d{4}-\d{2}-\d{2}\b[^\s]*&sig=[A-Za-z0-9%+/]{40,}",
        validator: Validator::None,
    },
    RuleSpec {
        // **P2 ozzt.** Cloudflare tokens have no standalone prefix, so the
        // env-var context is mandatory — otherwise any 40-char alnum string
        // would auto-wipe (§5.2, §8.1.2).
        name: "cloudflare_api_token",
        category: Category::Credential,
        confidence: 0.92,
        pattern: r"(?i)\b(?:CLOUDFLARE_API_(?:TOKEN|KEY)|CF_API_TOKEN)\s*=\s*[A-Za-z0-9_-]{40}\b",
        validator: Validator::None,
    },
    // -- Private keys ------------------------------------------------------
    RuleSpec {
        // gitleaks `private-key`. Subsumes v1's `ssh_private_key` (rule 19) and
        // `ssh_private_key_pkcs8_encrypted` (rule 20, Audit MED #5 — a real
        // miss: rule 19 did not cover the ENCRYPTED header) and adds PGP,
        // `SSH2`, `KEY BLOCK` and every other header variant (§8 row 19/20).
        // No `^` anchor, so a header mid-blob still matches.
        name: "private_key",
        category: Category::Credential,
        confidence: 0.99,
        pattern: r"-----BEGIN[ A-Z0-9_-]{0,100}PRIVATE KEY(?: BLOCK)?-----",
        validator: Validator::None,
    },
    RuleSpec {
        // **Audit MED #5** — a real miss: PuTTY `.ppk`. No gitleaks equivalent
        // (§8 row 21), so this stays custom. `(?m)` so `^` anchors per line
        // inside a pasted blob.
        name: "putty_private_key",
        category: Category::Credential,
        confidence: 0.99,
        pattern: r"(?m)^PuTTY-User-Key-File-[0-9]+:",
        validator: Validator::None,
    },
    // -- Generic / keyword-driven ------------------------------------------
    RuleSpec {
        // ⚠ INERT. **P2 fb3e**: lowered 0.80 → 0.65. Fires on
        // `Bearer YOUR_TOKEN_HERE` in curl examples and READMEs. A post-match
        // entropy guard would be a **no-op** here — the 20-char minimum already
        // satisfies any strength check — so the confidence floor is the only
        // correct control (`detector/fp.rs:36-40`).
        name: "generic_bearer",
        category: Category::Credential,
        confidence: 0.65,
        pattern: r"(?i)\bBearer\s+[A-Za-z0-9\-._~+/]{20,}",
        validator: Validator::None,
    },
    RuleSpec {
        // Above the floor *only* because of the value-strength validator
        // (§5.3, §7.1). `CopyPaste-2eet` added `access_token`, `client_secret`,
        // `refresh_token`, `db_password`; `access_token`/`refresh_token` were a
        // genuine miss (the others were already covered by the unbounded
        // `secret`/`password` substrings).
        // gitleaks `generic-api-key` gates on Shannon entropy ≥ 3.5, which
        // would reject §9.1's own fixtures (`hunter2` scores 2.81), so the
        // manifest's variety gate is kept and extended — see `value_is_strong`.
        name: "generic_password_kv",
        category: Category::Credential,
        confidence: 0.75,
        pattern: r"(?i)(?:password|passwd|secret|api_key|apikey|auth_token|access_token|client_secret|refresh_token|db_password)\s*[:=]\s*(\S{6,})",
        validator: Validator::ValueStrength,
    },
    RuleSpec {
        // §7.1: v1 had this at 0.80 with **no** value-strength validator, so
        // `API_KEY=changeme`, `DB_PASSWORD=xxx` and `MY_TOKEN=TODO` all
        // auto-wiped. The manifest offers "apply the §5.3 validator or demote";
        // applying the validator keeps the recall, so group 1 was added to the
        // regex and the rule now shares `generic_password_kv`'s gate.
        name: "dotenv_secret",
        category: Category::Infrastructure,
        confidence: 0.80,
        pattern: r"(?m)^(?:export\s+)?[A-Z][A-Z0-9_]{2,}(?:_KEY|_SECRET|_TOKEN|_PASSWORD|_PASS|_PWD|_CREDENTIALS?)\s*=\s*(\S+)",
        validator: Validator::ValueStrength,
    },
    RuleSpec {
        // gitleaks `jwt` requires **both** the header and the payload segment
        // to be base64-`ey`-prefixed — tighter than v1's single `\beyJ`
        // (§8 row 24). Adopted: it keeps §9.1's JWT fixtures and independently
        // rejects §9.2's `configsomethingeyJabc.def.ghi`.
        // The `\b` itself was Audit MED #5 (`mykeyeyJabc.def.ghi` classified as
        // a JWT).
        name: "jwt",
        category: Category::Credential,
        confidence: 0.95,
        pattern: r"\beyJ[A-Za-z0-9_-]+\.eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+",
        validator: Validator::None,
    },
    // -- Infrastructure ----------------------------------------------------
    RuleSpec {
        // Requires `user:password@host` — that anchor is what makes 0.99 safe,
        // and §8 calls it one of the best rules in the set.
        name: "db_conn_string",
        category: Category::Infrastructure,
        confidence: 0.99,
        pattern: r"(?i)(?:postgresql|postgres|mysql|mongodb|redis|amqp|mssql)://[^@\s]*:[^@\s]*@\S+",
        validator: Validator::None,
    },
    RuleSpec {
        // ⚠ INERT. `CopyPaste-8ys1`, a real FP **with data loss**: this sat at
        // exactly 0.70 — on the floor — so RFC1918 addresses (`10.x`,
        // `172.16–31.x`, `192.168.x`) in config files and docker-compose
        // snippets silently expired. A bare `IP:port` is topology, not
        // credential material; credentialed connections are caught by
        // `db_conn_string` at 0.99.
        name: "ip_with_port",
        category: Category::Infrastructure,
        confidence: 0.65,
        pattern: r"\b(?:(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\.){3}(?:25[0-5]|2[0-4]\d|[01]?\d\d?):\d{2,5}\b",
        validator: Validator::None,
    },
    // -- Financial / personal ----------------------------------------------
    RuleSpec {
        // Manifest §3.3 / §5.4. Not a regex-only rule: the candidate scan finds
        // 13–19-digit runs (optionally single-space/hyphen separated) and Luhn
        // decides. **Audit MED #6**: v1's original gate was
        // `normalised.len() <= 25 && luhn_valid(normalised)`, i.e. a card was
        // only detected when the clipboard held *nothing but* the number — a
        // card embedded in ordinary text was a silent miss.
        // In v1 this lived outside the pattern table and therefore produced no
        // spans and no rule name, while the FFI docs advertised a
        // `"credit_card"` pattern name that did not exist. §3.3 says v2 must
        // make it a first-class rule; it now is.
        name: "credit_card",
        category: Category::Financial,
        confidence: 0.99,
        pattern: r"\b(?:\d[\s-]?){12,18}\d\b",
        validator: Validator::Luhn,
    },
    RuleSpec {
        // ⚠ INERT. **P2 fb3e**, a real FP **with data loss**: legitimately
        // copied IBANs (invoices, bank transfers) were being silently
        // auto-wiped. The correct long-term fix is a mod-97 checksum validator
        // analogous to Luhn; until then the floor is the safe fallback.
        // gitleaks is secrets-only and ships no PII rules (§8.1.4).
        name: "iban",
        category: Category::Financial,
        confidence: 0.65,
        pattern: r"\b[A-Z]{2}[0-9]{2}[A-Z0-9]{4}[0-9]{7}[A-Z0-9]{0,16}\b",
        validator: Validator::None,
    },
    RuleSpec {
        // ⚠ INERT. **P2 fb3e**: lowered 0.80 → 0.65 — it also matches dates and
        // unit strings (`012 31 2024`). §4.2 names the structural validator as
        // the correct fix; it is applied here (see `ssn_structure_plausible`)
        // but the confidence stays sub-floor, because a plausible SSN is still
        // the user's own data (§4.2 reason 1).
        name: "ssn_us",
        category: Category::PersonalId,
        confidence: 0.65,
        pattern: r"\b\d{3}[-\s]\d{2}[-\s]\d{4}\b",
        validator: Validator::SsnStructure,
    },
    RuleSpec {
        // ⚠ INERT. The user meant to copy their own address.
        name: "email",
        category: Category::PersonalId,
        confidence: 0.60,
        pattern: r"\b[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}\b",
        validator: Validator::None,
    },
    RuleSpec {
        // ⚠ INERT. v1 had no leading anchor, so this matched inside longer
        // digit runs. The benign corpus entry `the order number is 1234567890`
        // is exactly that failure, and §7.7 says v2 must assert **zero** FPs on
        // the corpus rather than budget for two. Hence `PhoneShape`: an
        // unformatted 10-digit run is a number, not a phone number.
        name: "phone_us",
        category: Category::PersonalId,
        confidence: 0.55,
        pattern: r"(?:\+1[-.\s]?)?\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}\b",
        validator: Validator::PhoneShape,
    },
    RuleSpec {
        // ⚠ INERT. Minimum digits raised 6 → 9 to cut FPs on order IDs and
        // product codes.
        name: "passport",
        category: Category::PersonalId,
        confidence: 0.55,
        pattern: r"\b[A-Z]{1,2}[0-9]{9}\b",
        validator: Validator::None,
    },
];

// ---------------------------------------------------------------------------
// Validators
// ---------------------------------------------------------------------------

/// Characters in the §5.3 "special character" set. Note `$`, `#`, `%`, `/` and
/// `=` are *strength* signals here, which is why the code-shape rejection below
/// cannot simply blacklist punctuation.
const STRENGTH_SPECIALS: &str = "!@#$%^&*+/=";

/// Code punctuation that a machine-issued credential essentially never
/// contains, but that source code and templates are full of. v2 addition —
/// see `value_is_strong`.
const CODE_SHAPED_CHARS: &[char] = &['(', ')', '\'', '"', '`', '<', '>'];

/// Whole-value placeholder stopwords (gitleaks' allowlist model, §5.6 — v1 had
/// *no* allowlist of any kind and called it its biggest structural gap).
///
/// Matched against the **entire** value, case-insensitively, never as a
/// substring: a substring match could suppress a real credential that happens
/// to contain one of these letter sequences.
const VALUE_STOPWORDS: &[&str] = &[
    "changeme",
    "change_me",
    "yourkey",
    "your_key",
    "yourapikey",
    "your_api_key",
    "yoursecret",
    "your_secret",
    "yourpassword",
    "your_password",
    "placeholder",
    "redacted",
    "insert_key_here",
    "notarealsecret",
    "example",
    "todo",
    "tbd",
    "n/a",
    "none",
    "null",
    "nil",
    "undefined",
];

/// Value-strength gate — the manifest's only post-match validator for keyword
/// rules (§5.3), plus the two v2 additions §7.1 and §7.7 ask for.
///
/// The three manifest criteria, verbatim. A value is strong if **any** holds:
///
/// 1. `value.chars().count() >= 10` — **characters, not bytes**;
/// 2. contains one of `! @ # $ % ^ & * + / =`;
/// 3. contains at least one ASCII letter **and** at least one ASCII digit.
///
/// > **The char-vs-byte gate.** A 9-character CJK value (`私的秘密言葉確認鍵`)
/// > is 27 bytes. A byte-length gate would call it strong; the char gate
/// > correctly calls it weak. Pinned by `multibyte_value_gated_on_chars_not_bytes`.
///
/// Two rejections run *before* those criteria:
///
/// * **Code shape.** The benign corpus contains
///   `const password = prompt('enter password:');`. Group 1 captures
///   `prompt('enter` — 13 characters, so criterion 1 calls it strong and v1
///   auto-wiped it. That entry is one of the two FPs v1's 5 % budget silently
///   absorbed (§7.7). A value containing `(`, `)`, `'`, `"`, `` ` ``, `<`, `>`,
///   or opening with `$`/`${`/`{{` is source code or a template reference, not
///   a literal secret. Biasing toward "not a secret" is the direction CLAUDE.md
///   rule 4 and manifest I1 both require.
/// * **Stopwords** (§5.6).
fn value_is_strong(value: &str) -> bool {
    if value.starts_with('$') || value.contains("${") || value.contains("{{") {
        return false;
    }
    if value.contains(CODE_SHAPED_CHARS) {
        return false;
    }
    let lowered = value.to_lowercase();
    if VALUE_STOPWORDS.contains(&lowered.as_str()) {
        return false;
    }
    value.chars().count() >= 10
        || value.chars().any(|c| STRENGTH_SPECIALS.contains(c))
        || (value.chars().any(|c| c.is_ascii_alphabetic())
            && value.chars().any(|c| c.is_ascii_digit()))
}

/// Luhn checksum over a candidate digit run. Manifest §5.4.
///
/// **One implementation** (§7.3): v1 shipped `luhn_valid` and
/// `luhn_valid_strict` as byte-for-byte the same algorithm, justified by an
/// allocation saving that did not exist — both allocated, both ran the same
/// digit filter.
///
/// The `13 ..= 19` clamp is load-bearing: it rejects short and long numeric
/// runs outright, independently of the checksum.
fn luhn_valid(candidate: &str) -> bool {
    let digits: Vec<u32> = candidate
        .bytes()
        .filter(u8::is_ascii_digit)
        .map(|b| u32::from(b - b'0'))
        .collect();
    if !(13..=19).contains(&digits.len()) {
        return false;
    }
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| {
            if i % 2 == 1 {
                let doubled = d * 2;
                if doubled > 9 {
                    doubled - 9
                } else {
                    doubled
                }
            } else {
                d
            }
        })
        .sum();
    sum.is_multiple_of(10)
}

/// SSN group structure, per §4.2: group 1 in 001–899, group 2 in 01–99,
/// group 3 in 0001–9999, no all-zero group.
///
/// This only trims obvious non-SSNs; the rule stays below the auto-wipe floor
/// regardless, because a *real* SSN is still the user's own data (§4.2).
fn ssn_structure_plausible(matched: &str) -> bool {
    let groups: Vec<u32> = matched
        .split(|c: char| !c.is_ascii_digit())
        .filter(|g| !g.is_empty())
        .filter_map(|g| g.parse::<u32>().ok())
        .collect();
    match groups.as_slice() {
        [area, group, serial] => {
            (1..=899).contains(area) && (1..=99).contains(group) && (1..=9999).contains(serial)
        }
        _ => false,
    }
}

/// A phone number must look like one. See the `phone_us` rule comment: the
/// benign corpus entry `the order number is 1234567890` is a bare digit run,
/// and §7.7 requires zero corpus FPs.
fn phone_is_formatted(matched: &str) -> bool {
    matched.starts_with('+')
        || matched
            .chars()
            .any(|c| matches!(c, '(' | ')' | '-' | '.') || c.is_whitespace())
}

// ---------------------------------------------------------------------------
// Tests — manifest §9
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn detector() -> Detector {
        Detector::new().expect("ruleset compiles")
    }

    /// All validated matches, ranked. Test-only: production has exactly one
    /// verdict function and one label function (§7.4 — v1 shipped four
    /// overlapping "is it sensitive?" entry points, three of them dead).
    fn all_rules(det: &Detector, text: &str) -> Vec<&'static str> {
        let normalised = normalise(text);
        det.set
            .matches(&normalised)
            .into_iter()
            .filter(|&i| det.rules[i].hit(&normalised).is_some())
            .map(|i| det.rules[i].spec.name)
            .collect()
    }

    fn fired(det: &Detector, text: &str, rule: &str) -> bool {
        all_rules(det, text).contains(&rule)
    }

    fn rule(name: &str) -> &'static RuleSpec {
        RULES
            .iter()
            .find(|r| r.name == name)
            .unwrap_or_else(|| panic!("no rule named {name}"))
    }

    fn rep(c: char, n: usize) -> String {
        std::iter::repeat_n(c, n).collect()
    }

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

    /// The benign corpus. §7.7: v1's `max(len * 5 / 100, 2)` budget tolerated
    /// two unnamed FPs — `const password = prompt(...)` was one of them. v2
    /// asserts **zero**; any accepted FP must be named here with a reason.
    const BENIGN_CORPUS: &[&str] = &[
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

    /// §5.4's test-fixture bug: the original negative fixture `4242424242422`
    /// was *accidentally Luhn-valid* (digit sum 50), so the "must not match"
    /// test passed vacuously. **Assert your negative fixtures are negative.**
    #[test]
    fn negative_card_fixtures_are_actually_negative() {
        assert!(
            luhn_valid("4242424242422"),
            "the old fixture really was valid"
        );
        assert!(
            !luhn_valid("4242424242421"),
            "the replacement must be invalid"
        );
        assert!(luhn_valid("4111111111111111"));
        // the 13..=19 clamp, independent of the checksum
        assert!(!luhn_valid("42424242424"));
        assert!(!luhn_valid("41111111111111111111"));
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

    // -- §5.3 the value-strength gate ---------------------------------------

    #[test]
    fn value_strength_follows_the_manifest_criteria() {
        // 1 — ten characters
        assert!(value_is_strong("abcdefghij"));
        assert!(!value_is_strong("abcdefghi"));
        // 2 — a special character
        assert!(value_is_strong("!abcdef"));
        assert!(value_is_strong("ab=cdef"));
        // 3 — letter and digit
        assert!(value_is_strong("hunter2"));
        assert!(!value_is_strong("hunter"));
        assert!(!value_is_strong("123456"));
    }

    /// The char-vs-byte gate. `私的秘密言葉確認鍵` is 9 chars / 27 bytes: a
    /// byte-length gate would wrongly call it strong.
    #[test]
    fn multibyte_value_gated_on_chars_not_bytes() {
        let nine = "私的秘密言葉確認鍵";
        let ten = "私的秘密言葉確認鍵値";
        assert_eq!(nine.len(), 27);
        assert_eq!(nine.chars().count(), 9);
        assert!(!value_is_strong(nine));
        assert!(value_is_strong(ten));

        let det = detector();
        assert!(!det.is_sensitive(&format!("password: {nine}")));
        assert!(det.is_sensitive(&format!("password: {ten}")));
    }

    /// v2 addition (§7.7): code and template shapes are not secrets.
    #[test]
    fn code_shaped_values_are_weak() {
        assert!(!value_is_strong("prompt('enter"));
        assert!(!value_is_strong("getEnv();"));
        assert!(!value_is_strong("${API_KEY}"));
        assert!(!value_is_strong("$MY_SECRET_VAR"));
        assert!(!value_is_strong("{{ vault_password }}"));
        assert!(!value_is_strong("<your-key-here>"));
        // stopwords, matched whole-value only
        assert!(!value_is_strong("changeme"));
        assert!(!value_is_strong("PLACEHOLDER"));
        // …and never as a substring, so a real credential survives
        assert!(value_is_strong("changeme7f3a91bc2d"));
    }

    /// §7.1: v1 ran `dotenv_secret` at 0.80 with no validator, so
    /// `API_KEY=changeme` auto-wiped.
    #[test]
    fn dotenv_secret_is_value_gated() {
        let det = detector();
        for weak in [
            "API_KEY=changeme",
            "DB_PASSWORD=xxx",
            "MY_TOKEN=TODO",
            "APP_KEY=${FOO}",
        ] {
            assert!(
                !det.is_sensitive(weak),
                "{weak:?} -> {:?}",
                all_rules(&det, weak)
            );
        }
        let strong = "export DATADOG_API_KEY=8f14e45fceea167a5a36dedd4bea2543";
        assert!(fired(&det, strong, "dotenv_secret"));
        assert_eq!(det.scan(strong).unwrap().severity, Severity::HighConfidence);
    }

    // -- §5.1 / I3 normalisation --------------------------------------------

    /// The bypass NFKC closes: full-width Latin renders as ASCII but matches no
    /// ASCII character class.
    #[test]
    fn nfkc_normalised_input_detects_secrets() {
        let det = detector();
        let fullwidth = "\u{FF21}\u{FF2B}\u{FF29}\u{FF21}IOSFODNN7EXAMPLE";
        assert_ne!(fullwidth, "AKIAIOSFODNN7EXAMPLE");
        // without normalisation the raw bytes match nothing
        assert!(!Regex::new(rule("aws_access_key").pattern)
            .unwrap()
            .is_match(fullwidth));
        // with it, the same key is found
        assert!(det.is_sensitive(fullwidth));
        assert_eq!(det.scan(fullwidth).unwrap().rule, "aws_access_key");
    }

    #[test]
    fn nfkc_bypass_closed_for_full_width_digits_and_cards() {
        let det = detector();
        // 4111111111111111 written with FULLWIDTH DIGIT ZERO..NINE
        let fullwidth: String = "4111111111111111"
            .chars()
            .map(|c| char::from_u32(u32::from(c) - u32::from('0') + 0xFF10).unwrap())
            .collect();
        assert_ne!(fullwidth, "4111111111111111");
        assert_eq!(det.scan(&fullwidth).unwrap().rule, "credit_card");
    }

    /// §5.1: NFKC is the identity on ASCII — which is what lets callers index
    /// spans into the original string, and what makes the fast path sound.
    #[test]
    fn normalise_is_the_identity_on_ascii() {
        assert!(matches!(
            normalise("AKIAIOSFODNN7EXAMPLE"),
            Cow::Borrowed(_)
        ));
        assert_eq!(normalise("AKIAIOSFODNN7EXAMPLE"), "AKIAIOSFODNN7EXAMPLE");
        for text in BENIGN_CORPUS {
            assert_eq!(normalise(text), *text);
        }
    }

    /// §7.8: v1's `nfkc_zwj_in_jwt_normalises_away` asserted nothing about ZWJ
    /// — its body tested a clean ASCII JWT. This is the real test. NFKC does
    /// **not** strip default-ignorable code points, so a ZWJ spliced into a
    /// token still defeats the regex. The bypass is open, by decision:
    /// stripping default-ignorables before matching would change every offset
    /// this module might ever return, and the failure direction is a false
    /// negative (I1's preferred direction), not data loss.
    #[test]
    fn zwj_bypass_is_documented_and_still_open() {
        let det = detector();
        let spliced = "AKIA\u{200D}IOSFODNN7EXAMPLE";
        assert_eq!(normalise(spliced), spliced, "NFKC keeps ZWJ");
        assert!(!det.is_sensitive(spliced), "known gap, tracked in §7.8");
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
    fn rule_names_are_unique() {
        let mut names: Vec<_> = RULES.iter().map(|r| r.name).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(names.len(), before);
    }

    /// I1 / §9.3: the confidence-floor pin. Lowering any of these was a fix for
    /// real, observed data loss; raising one again needs a validator first.
    #[test]
    fn fp_risk_patterns_below_autowipe_floor() {
        for name in [
            "discord_bot_token",
            "twilio_signing_key_sid",
            "iban",
            "ssn_us",
            "generic_bearer",
            "ip_with_port",
            "email",
            "phone_us",
            "passport",
            // §7.1 — demoted in v2
            "aws_arn",
        ] {
            assert!(
                rule(name).confidence < AUTOWIPE_CONFIDENCE_FLOOR,
                "{name} is at or above the auto-wipe floor"
            );
        }
    }

    /// §4.2: "nothing may sit *exactly* on the floor" — `ip_with_port` did, and
    /// it caused data loss (`CopyPaste-8ys1`).
    #[test]
    fn no_rule_sits_exactly_on_the_floor() {
        for r in RULES {
            assert!(
                (r.confidence - AUTOWIPE_CONFIDENCE_FLOOR).abs() > f32::EPSILON,
                "{} sits exactly on the floor",
                r.name
            );
            assert!((0.0..=1.0).contains(&r.confidence), "{}", r.name);
        }
    }

    /// §5.2 / §9.3: the `\b` anchor pin. A token glued into a longer identifier
    /// must not match.
    #[test]
    fn prefixed_token_rules_are_word_anchored() {
        for name in [
            "sendgrid_api_key",
            "terraform_cloud_token",
            "cloudflare_api_token",
            "twilio_signing_key_sid",
            "discord_bot_token",
        ] {
            assert!(
                rule(name).pattern.contains(r"\b"),
                "{name} lost its \\b anchor"
            );
        }
        // …and the deliberate omissions stay omitted: `aws_access_key` has no
        // trailing \b because ASIA keys carry trailing digits, and
        // `azure_storage_key` is context-anchored instead of \b-anchored.
        assert!(!rule("aws_access_key").pattern.ends_with(r"\b"));
        assert!(!rule("azure_storage_key").pattern.contains(r"\b"));
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

    /// §9.3: each P2-`ozzt` rule is a credential at ≥ 0.90.
    #[test]
    fn p2_ozzt_rules_are_high_confidence_credentials() {
        for name in [
            "azure_storage_key",
            "azure_sas_token",
            "gcp_service_account_key",
            "cloudflare_api_token",
            "sendgrid_api_key",
            "terraform_cloud_token",
        ] {
            let r = rule(name);
            assert_eq!(r.category, Category::Credential, "{name}");
            assert!(r.confidence >= 0.90, "{name}");
        }
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

    #[test]
    fn ssn_structure_rejects_impossible_groups() {
        assert!(ssn_structure_plausible("123-45-6789"));
        assert!(ssn_structure_plausible("012 31 2024"));
        assert!(!ssn_structure_plausible("000-45-6789"));
        assert!(!ssn_structure_plausible("123-00-6789"));
        assert!(!ssn_structure_plausible("123-45-0000"));
        assert!(!ssn_structure_plausible("900-45-6789"));
    }

    #[test]
    fn phone_shape_rejects_bare_digit_runs() {
        assert!(phone_is_formatted("(555) 867-5309"));
        assert!(phone_is_formatted("+1 555 867 5309"));
        assert!(phone_is_formatted("555-867-5309"));
        assert!(!phone_is_formatted("1234567890"));
    }

    /// §9.3 perf pin: no rule may be catastrophically backtracking. The `regex`
    /// crate gives linear-time guarantees by construction, so this is a smoke
    /// test that the *ruleset* (prefilter + per-rule passes) stays sane — the
    /// 10 MB / 500 ms budget in the manifest is a release-build benchmark and
    /// does not belong in a debug unit test.
    #[test]
    fn large_benign_input_completes_quickly() {
        let det = detector();
        let haystack = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(4_000);
        let started = std::time::Instant::now();
        assert!(!det.is_sensitive(&haystack));
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "ruleset is pathologically slow: {:?}",
            started.elapsed()
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

//! The ruleset (manifest §3.2, §3.3 and the gitleaks mapping in §8).
//!
//! # Why this file is nothing but a table
//!
//! Everything that *acts* on a rule has been moved out: the compiled engine and
//! the confidence ranking to [`super::engine`], the false-positive gates to
//! [`super::validators`], NFKC folding to [`super::normalise`], and the rule
//! vocabulary (`Category`, `Validator`, `RuleSpec`) to [`super::spec`]. What is
//! left is one `static` array of declarative entries — no control flow, no
//! helpers, nothing to call — which is why it may stay whole even as it grows
//! past the budget. The two splits considered, and rejected:
//!
//! * **By vendor** (`rules/aws.rs`, `rules/github.rs`, …). The comment banners
//!   already group them, and grouping is the *only* thing declaration order
//!   means (§7.2). Splitting turns "is anything else at this confidence?" and
//!   "does another rule already cover this shape?" — the two questions actually
//!   asked when editing a rule — into a nine-file grep, and invites a rule to
//!   be added to a *file* rather than to the ruleset.
//! * **By confidence band** (above/below the auto-wipe floor). Worse: the band
//!   is a property that changes when a validator is added, so a rule's file
//!   would flip on tuning and the diff would hide what `⚠ INERT` makes obvious
//!   in place.
//!
//! # Reading the table
//!
//! Declaration order is **not** semantic (§7.2). It is grouped by vendor purely
//! so a human can read it.
//!
//! `⚠ INERT` in a comment marks a rule deliberately tuned below the 0.70
//! auto-wipe floor. Two different reasons live in that band (§4.2): data the
//! user legitimately owns and meant to copy (IBAN, SSN, email, phone,
//! passport), and shapes too weak to prove a secret (`discord_bot_token`,
//! `twilio_signing_key_sid`, `generic_bearer`, `ip_with_port`).
//!
//! Where §8 names a gitleaks rule that is compatible with the manifest's
//! acceptance tests, the gitleaks form is used and its rule id is cited.
//! Where adopting it would fail an acceptance test in §9.1, the v1 form is
//! kept and the conflict is written down — §8 is explicit that its table is
//! "a starting point, not a contract", while §9 is a requirements list.

use super::spec::{Category, RuleSpec, Validator};

pub(super) static RULES: &[RuleSpec] = &[
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

/// Look a rule up by name. Test-only: production never addresses a single rule,
/// it runs the whole set.
#[cfg(test)]
pub(super) fn rule(name: &str) -> &'static RuleSpec {
    RULES
        .iter()
        .find(|r| r.name == name)
        .unwrap_or_else(|| panic!("no rule named {name}"))
}

// ---------------------------------------------------------------------------
// Tests — the table's own invariants (manifest §9.3)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sensitive::finding::AUTOWIPE_CONFIDENCE_FLOOR;

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
}

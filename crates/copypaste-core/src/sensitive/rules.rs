//! The ruleset (manifest §3.2, §3.3 and the gitleaks mapping in §8).
//!
//! One `static` array of declarative entries; everything that *acts* on a rule
//! lives in [`super::engine`], [`super::validators`], [`super::normalise`] and
//! [`super::spec`]. That is why it may stay whole past the size budget
//! (`CLAUDE.md` rule 5): splitting by vendor turns "is anything else at this
//! confidence?" into a nine-file grep, and splitting by confidence band moves a
//! rule between files whenever a validator is tuned.
//!
//! `⚠ INERT` marks a rule deliberately tuned below the 0.70 auto-wipe floor.
//! Two reasons live in that band (§4.2): data the user legitimately owns and
//! meant to copy (IBAN, SSN, email, phone, passport), and shapes too weak to
//! prove a secret (`discord_bot_token`, `twilio_signing_key_sid`,
//! `generic_bearer`, `http_basic_auth`, `ip_with_port`).
//!
//! Where §8 names a gitleaks rule compatible with the acceptance tests, the
//! gitleaks form is used and its id cited; where adopting it would fail a §9.1
//! test, the v1 form is kept and the conflict written down (§8 is "a starting
//! point, not a contract", §9 is a requirements list).

use super::spec::{Category, RuleSpec, Validator};

pub(super) static RULES: &[RuleSpec] = &[
    // -- AWS ---------------------------------------------------------------
    RuleSpec {
        // gitleaks `aws-access-token`; broader than v1's `AKIA|ASIA` (§8 row 0).
        // Leading `\b` stops mid-token hits (`XAKIA…`); **no trailing `\b` on
        // purpose** — ASIA temp keys carry trailing digits (§5.2, "deliberate
        // omissions … must not be fixed blindly").
        name: "aws_access_key",
        category: Category::Credential,
        confidence: 0.99,
        pattern: r"\b(?:A3T[0-9A-Z]|AKIA|ASIA|ABIA|ACCA)[0-9A-Z]{16}",
        validator: Validator::None,
    },
    RuleSpec {
        // ⚠ INERT. §7.1: an ARN is a *resource identifier*, not credential
        // material (the reasoning that demoted `ip_with_port` under
        // `CopyPaste-8ys1`). v1 had it at 0.90, so pasting an ARN into a ticket
        // deleted it 30 s later. gitleaks ships no ARN rule (§8 row 32).
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
        // gitleaks `openai-api-key` anchors on the `T3BlbkFJ` infix and is more
        // precise (§8 row 4/5), but §9.1 requires `sk-proj-` + 48×A and that
        // fixture has no infix. v1 form kept; revisit with the fixture.
        name: "openai_new",
        category: Category::Credential,
        confidence: 0.99,
        pattern: r"sk-proj-[A-Za-z0-9]{48}",
        validator: Validator::None,
    },
    RuleSpec {
        // Must NOT double-fire on `sk-proj-` keys. The exclusion is
        // **structural, not lookahead** (the `regex` crate has none): the
        // hyphen after `proj` breaks the contiguous 48-char alnum run
        // (P2 `r6cw`).
        name: "openai_legacy",
        category: Category::Credential,
        confidence: 0.95,
        pattern: r"\bsk-[A-Za-z0-9]{48}\b",
        validator: Validator::None,
    },
    RuleSpec {
        // gitleaks `anthropic-api-key` requires the full 93-char body + `AA`
        // suffix; §9.1's fixture is `sk-ant-api03-` + 80×A, so the v1 length
        // form is kept. `admin` closes the gap in §8 row 6.
        name: "anthropic",
        category: Category::Credential,
        confidence: 0.99,
        pattern: r"sk-ant-(?:api|admin)\d{2}-[A-Za-z0-9_-]{80,}",
        validator: Validator::None,
    },
    // -- Payments / SaaS ---------------------------------------------------
    RuleSpec {
        // gitleaks `stripe-access-token` also covers `sk_test_`; test keys stay
        // excluded deliberately — they live in public docs and deleting them is
        // pure cost. `rk_` and `prod` adopted.
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
        // gitleaks `slack-bot-token` and friends. v1 matched only `xoxb` with
        // both numeric segments fixed at 11 digits; `xoxa/xoxp/xoxr/xoxs` were
        // named gaps (§3.2 row 11, §8 row 11).
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
        // ⚠ INERT. **P2 fb3e**: lowered 0.85 → 0.65 and `\b`-anchored — the
        // shape fires on any dot-separated base64url triple. gitleaks
        // `discord-api-token` keyword-anchors on "discord" and is better, but
        // §9.2 requires the bare shape to still be *detected*, which a keyword
        // anchor would not do. Adopting that anchor is what would justify
        // raising it (§8 row 13).
        name: "discord_bot_token",
        category: Category::Credential,
        confidence: 0.65,
        pattern: r"\b[MN][a-zA-Z\d]{23,25}\.[\w-]{6}\.[\w-]{27,38}\b",
        validator: Validator::None,
    },
    RuleSpec {
        // ⚠ INERT. **P2 fb3e**: was misnamed `twilio_auth_token`; this is a
        // Signing-Key SID. Real auth tokens are bare 32-hex and not
        // regex-distinguishable. Without gitleaks' keyword anchor
        // (`twilio-api-key`) this stays below the floor (§8 row 14).
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
        // `.atlasv1.`; §9.1's fixture is bare, so the v1 form is kept
        // (§8 row 39).
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
        // gitleaks `heroku-api-key`. The `heroku`-within-50-chars anchor is
        // what makes 0.95 safe: a bare UUID is not a secret.
        name: "heroku_api_key",
        category: Category::Credential,
        confidence: 0.95,
        pattern: r"(?i)heroku[^\n]{0,50}[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}",
        validator: Validator::None,
    },
    RuleSpec {
        // gitleaks `vault-service-token` requires 90–100 chars; §9.1 requires
        // `hvs.` + 32×A to be detected *and auto-wiped*, so v1's `{32,}`
        // minimum stays (added to kill FPs on short `hvs.` strings). `hvb.`
        // closes the gap in §8 row 17.
        name: "hashicorp_vault",
        category: Category::Credential,
        confidence: 0.95,
        pattern: r"\bhv[sb]\.[A-Za-z0-9_-]{32,}",
        validator: Validator::None,
    },
    RuleSpec {
        // **P2 ozzt + bug-hunt HIGH finding.** A bare 88-char base64 blob is
        // indistinguishable from a SHA-512 dump or an Ed25519 key, so matching
        // it bare at 0.90 would auto-wipe benign content. The `AccountKey=`
        // anchor is **mandatory** and the only reason this rule is allowed
        // above the floor (§5.2, §8.1.2).
        name: "azure_storage_key",
        category: Category::Credential,
        confidence: 0.90,
        pattern: r"AccountKey=[A-Za-z0-9+/]{86}==",
        validator: Validator::None,
    },
    RuleSpec {
        // **P2 ozzt.** Anchors only on the two stable markers (`sv=` and
        // `sig=`), in either order and with raw or HTML-escaped separators;
        // the previous over-specified form matched almost no real tokens.
        name: "azure_sas_token",
        category: Category::Credential,
        confidence: 0.92,
        pattern: concat!(
            r"(?i)(?:\bsv=\d{4}-\d{2}-\d{2}\b[^\s#]*&(?:amp;)?sig=[A-Za-z0-9%+/]{40,}",
            r"|\bsig=[A-Za-z0-9%+/]{40,}[^\s#]*&(?:amp;)?sv=\d{4}-\d{2}-\d{2}\b)",
        ),
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
        // gitleaks `private-key`. Subsumes v1's rules 19 and 20 (Audit MED #5 —
        // a real miss: rule 19 did not cover the ENCRYPTED header) and adds
        // PGP, `SSH2`, `KEY BLOCK` and the other header variants (§8 row
        // 19/20). No `^` anchor, so a header mid-blob still matches.
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
        // ⚠ INERT. **P2 fb3e**: lowered 0.80 → 0.65; fires on
        // `Bearer YOUR_TOKEN_HERE` in curl examples. A post-match entropy guard
        // would be a **no-op** — the 20-char minimum already satisfies any
        // strength check — so the floor is the only correct control.
        name: "generic_bearer",
        category: Category::Credential,
        confidence: 0.65,
        pattern: r"(?i)\bBearer\s+[A-Za-z0-9\-._~+/]{20,}",
        validator: Validator::None,
    },
    RuleSpec {
        // Above the floor *only* because of the value-strength validator
        // (§5.3, §7.1). `CopyPaste-2eet` added the `access_token` /
        // `refresh_token` keywords, a genuine miss. gitleaks
        // `generic-api-key` gates on Shannon entropy ≥ 3.5, which would reject
        // §9.1's own fixtures (`hunter2` scores 2.81), so the manifest's
        // variety gate is kept — see `value_is_strong`.
        // The `["']?` after the keyword is a v2 addition to §3.2 row 23: JSON
        // and YAML write `"password": "hunter2xyz"`, and the manifest pattern
        // requires `[:=]` immediately after the keyword, so that whole shape
        // matched no rule at all. Pinned by `quoted_credentials_are_detected`.
        name: "generic_password_kv",
        category: Category::Credential,
        confidence: 0.75,
        pattern: r#"(?i)(?:password|passwd|secret|api_key|apikey|auth_token|access_token|client_secret|refresh_token|db_password)["']?\s*[:=]\s*(\S{6,})"#,
        validator: Validator::ValueStrength,
    },
    RuleSpec {
        // The other half of an AWS key pair. `aws_access_key` catches the public
        // `AKIA…` id at 0.99 while the actual secret next to it in
        // `~/.aws/credentials` matched nothing: `secret` is in
        // `generic_password_kv`'s keyword list, but `aws_secret_access_key` is
        // followed by `_access_key`, not by `[:=]`. The keyword is a mandatory
        // context anchor (§5.2) — a bare 40-char base64 run is not
        // distinctive — and is what makes 0.99 safe here.
        name: "aws_secret_access_key",
        category: Category::Credential,
        confidence: 0.99,
        pattern: r#"(?i)aws_secret_access_key["']?\s*[:=]\s*["']?([A-Za-z0-9/+=]{40})"#,
        validator: Validator::None,
    },
    RuleSpec {
        // gitleaks `gitlab-pat`. A distinctive vendor prefix, like the GitHub
        // rules above; it was simply absent.
        name: "gitlab_pat",
        category: Category::Credential,
        confidence: 0.99,
        pattern: r"\bglpat-[0-9a-zA-Z_-]{20}\b",
        validator: Validator::None,
    },
    RuleSpec {
        // ⚠ INERT. `Basic` carries base64(user:password), so it is credential
        // material — but it sits in curl examples and API docs exactly like
        // `generic_bearer`, which **P2 fb3e** demoted for that reason. Same
        // treatment: detected and kept out of the index, never auto-deleted.
        name: "http_basic_auth",
        category: Category::Credential,
        confidence: 0.65,
        pattern: r"(?i)\bAuthorization:\s*Basic\s+[A-Za-z0-9+/]{16,}={0,2}",
        validator: Validator::None,
    },
    RuleSpec {
        // §7.1: v1 had this at 0.80 with **no** value-strength validator, so
        // `API_KEY=changeme`, `DB_PASSWORD=xxx` and `MY_TOKEN=TODO` all
        // auto-wiped. Applying the §5.3 validator keeps the recall, so group 1
        // was added and the rule shares `generic_password_kv`'s gate.
        name: "dotenv_secret",
        category: Category::Infrastructure,
        confidence: 0.80,
        pattern: r"(?m)^(?:export\s+)?[A-Z][A-Z0-9_]{2,}(?:_KEY|_SECRET|_TOKEN|_PASSWORD|_PASS|_PWD|_CREDENTIALS?)\s*=\s*(\S+)",
        validator: Validator::ValueStrength,
    },
    RuleSpec {
        // gitleaks `jwt` requires **both** header and payload to be
        // base64-`ey`-prefixed, tighter than v1's single `\beyJ` (§8 row 24):
        // keeps §9.1's fixtures and rejects §9.2's
        // `configsomethingeyJabc.def.ghi`. The `\b` was Audit MED #5
        // (`mykeyeyJabc.def.ghi` classified as a JWT).
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
        // ⚠ INERT. `CopyPaste-8ys1`, a real FP **with data loss**: this sat
        // exactly on the 0.70 floor, so RFC1918 addresses in config files and
        // docker-compose snippets silently expired. A bare `IP:port` is
        // topology; credentialed connections are `db_conn_string` at 0.99.
        name: "ip_with_port",
        category: Category::Infrastructure,
        confidence: 0.65,
        pattern: r"\b(?:(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\.){3}(?:25[0-5]|2[0-4]\d|[01]?\d\d?):\d{2,5}\b",
        validator: Validator::None,
    },
    // -- Financial / personal ----------------------------------------------
    RuleSpec {
        // Manifest §3.3 / §5.4. Not regex-only: the scan finds 13–19-digit
        // runs and Luhn decides. **Audit MED #6**: v1 gated on
        // `normalised.len() <= 25 && luhn_valid(...)`, so a card was detected
        // only when the clipboard held *nothing but* the number — embedded in
        // text it was a silent miss. §3.3 requires it to be a first-class rule
        // rather than living outside the table with no rule name.
        name: "credit_card",
        category: Category::Financial,
        confidence: 0.99,
        pattern: r"\b(?:\d[\s-]?){12,18}\d\b",
        validator: Validator::Luhn,
    },
    RuleSpec {
        // ⚠ INERT. **P2 fb3e**, a real FP **with data loss**: legitimately
        // copied IBANs (invoices, transfers) were auto-wiped. The long-term fix
        // is a mod-97 validator analogous to Luhn; until then the floor is the
        // safe fallback. gitleaks ships no PII rules (§8.1.4).
        name: "iban",
        category: Category::Financial,
        confidence: 0.65,
        pattern: r"\b[A-Z]{2}[0-9]{2}[A-Z0-9]{4}[0-9]{7}[A-Z0-9]{0,16}\b",
        validator: Validator::None,
    },
    RuleSpec {
        // ⚠ INERT. **P2 fb3e**: lowered 0.80 → 0.65 — it also matches dates and
        // unit strings (`012 31 2024`). §4.2's structural validator is applied,
        // but the confidence stays sub-floor because a plausible SSN is still
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
        // digit runs — the benign-corpus entry `the order number is 1234567890`
        // is that failure, and §7.7 requires **zero** FPs on the corpus. Hence
        // `PhoneShape`: an unformatted 10-digit run is a number, not a phone.
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

/// Look a rule up by name. Test-only: production runs the whole set.
#[cfg(test)]
pub(super) fn rule(name: &str) -> &'static RuleSpec {
    RULES
        .iter()
        .find(|r| r.name == name)
        .unwrap_or_else(|| panic!("no rule named {name}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sensitive::engine::test_support::{detector, fired};
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
            "http_basic_auth",
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
            "gitlab_pat",
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

    #[test]
    fn azure_sas_markers_match_in_either_order() {
        let det = detector();
        let raw_sig = format!("{}+/=", "A".repeat(41));
        let encoded_sig = format!("{}%2B%2F%3D", "b".repeat(41));
        let cases = [
            format!("sv=2024-11-04&sig={raw_sig}"),
            format!("sig={encoded_sig}&amp;sv=2024-11-04"),
            format!("sv=2024-11-04&sp=rw&se=2030-01-01&sig={encoded_sig}"),
            format!("sig={raw_sig}&amp;sp=rw&amp;se=2030-01-01&amp;sv=2024-11-04"),
        ];

        for sas in cases {
            assert!(fired(&det, &sas, "azure_sas_token"), "{sas}");
            assert!(det.may_auto_wipe(&sas), "{sas}");
        }
    }

    #[test]
    fn azure_sas_requires_both_exact_markers_in_one_query() {
        let det = detector();
        let signature = "A".repeat(40);
        let cases = [
            "sv=2024-11-04&sig=too-short".to_string(),
            format!("sig={signature}&sv=not-a-version"),
            format!("signature={signature}&sv=2024-11-04"),
            format!("sig={signature} sv=2024-11-04"),
            format!("sig={signature}#fragment&sv=2024-11-04"),
        ];

        for benign in cases {
            assert!(!fired(&det, &benign, "azure_sas_token"), "{benign}");
        }
    }
}

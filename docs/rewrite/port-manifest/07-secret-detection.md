# Port Manifest 07 — Sensitive-Data (Secret) Detection

This is the binding contract for sensitive-content detection. It defines what
must be detected, what must remain inert, what must never reach search, and the
higher confidence required before automatic deletion. The ruleset is selected
from the maintained [gitleaks](https://github.com/gitleaks/gitleaks) source plus
reviewed local validators; the in-process scanner applies it to clipboard data.

---

## 1. Purpose & scope

### 1.1 What this subsystem decides

For every clipboard item the system captures, this subsystem answers three
questions:

| Question | Consumer | Current contract |
|---|---|---|
| **Must this whole item be withheld?** | capture, search, sync, previews | `Detector::is_sensitive` |
| **May this item be auto-wiped?** | bounded sensitive sweep | `Detector::may_auto_wipe` |
| **What matched?** | labels and diagnostics | `Detector::scan` / `Finding` |
| **Where did it match?** | redaction and bounded preview metadata | `Detector::scan_all` / `SpannedFinding` |

Plus a content-independent signal:

| Question | Consumer | Current contract |
|---|---|---|
| **Did this come from a password manager?** | capture provenance | `is_password_manager_app(bundle_id)` |

### 1.2 Why the stakes are asymmetric-but-both-bad

- A **false negative** means a secret is stored in plaintext in the FTS index,
  kept forever (no TTL), synced to peers/relay, and shown unmasked in history.
- A **false positive** at the deletion boundary destroys user data. The current
  product therefore separates flagging, withholding, and automatic deletion;
  only the high-confidence band may cross the deletion boundary.

The entire confidence model in §4 exists to make the second failure mode
impossible for weakly-distinctive patterns.

### 1.3 In scope / out of scope

**In scope:** the ruleset, the confidence model, false-positive defences, the
password-manager app list, NFKC normalisation, Luhn validation, the FTS
exclusion rule, and the sensitive-TTL/expiry contract.

**Out of scope (cross-referenced only):** the `org.nspasteboard.ConcealedType`
  pasteboard-marker skip — this is *layer 0*, a pre-capture opt-out honoured by
password managers; it means the best-behaved apps never reach this subsystem
at all. Port it, but it belongs to the clipboard manifest.

---

## 2. Invariants (MUST hold)

### I1 — Never silently delete user data (the prime directive)

No content may be scheduled for automatic expiry unless a rule with
confidence **≥ 0.70** matched it, or a Luhn-valid card run was found, or the
source app is on the password-manager list. Every rule whose shape is
"plausible but not distinctive" MUST be tuned below the floor: it is still
*detected* (surfaced, masked, labelled) but is **inert** for deletion.

Pinned by `fp_risk_patterns_below_autowipe_floor`.

### I2 — Detection and deletion are separate decisions

`detect()`/spans return **all** matches at all confidences. Only the auto-wipe
gate applies the wipe floor. A rule may be detected but inert. The public
verdict, span, and label APIs must use the same rule table and explicit bands.

### I3 — Normalise before matching

Input MUST be NFKC-normalised before any regex runs. Without it, `Ａ` (U+FF21
FULLWIDTH LATIN CAPITAL A) and
friends bypass every ASCII character class. All returned offsets are indices
into the **normalised** string, and every API returning offsets must state that.

### I4 — Sensitive items are never full-text indexed

`is_sensitive = 1` ⇒ never written to the FTS table, never returned by
search. Enforced at three independent layers (§6.1). Non-negotiable — this
was a real information-disclosure bug (`CopyPaste-i6pp`).

### I5 — Sensitive items carry no thumbnail

`is_sensitive = 1` ⇒ no thumbnail may be created or backfilled
(`CopyPaste-44rq.49`).
A thumbnail of a password-manager screenshot is a readable secret.

### I6 — The app signal is independent of every other list

`is_sensitive_app` MUST be evaluated on every capture, regardless of whether
the user's app-exclusion list is empty or the frontmost-app lookup is
best-effort. The app signal must not be gated by the user exclusion list
(`CopyPaste-44rq.43`); otherwise password-manager copies lose their independent
sensitivity provenance.

### I7 — Fail-safe directions are fixed per-decision

| Decision | Failure mode on error/unknown |
|---|---|
| "Are there sensitive rows to sweep?" | **fail-closed** → assume yes and run the bounded sweep |
| Ruleset construction fails | initialization fails; never substitute an empty detector |
| Frontmost-app lookup unavailable | **fail-open** for provenance only; content rules still apply |
| Detector panics across the Android boundary | recover to no verdict / empty spans, never abort the JVM |

### I8 — Index alignment

Rule identity, category, confidence, and the compiled matcher travel together.
A rejected rule cannot shift metadata onto another matcher.

### I9 — Redaction is span-merging and UTF-8-safe

Overlapping/adjacent match spans merge into one `***REDACTED***`; span bounds
are snapped outward to char boundaries so a mid-codepoint offset can never
panic and can only ever redact *more* (`sensitive/redact.rs:14-67`).

### I10 — Never log plaintext or the content hash next to it

Logs carry only bounded classifications and counters, never captured content.

---

## 3. The full ruleset

### 3.1 Categories

| Index | Category |
|---|---|
| 0 | Credential |
| 1 | Financial |
| 2 | PersonalId |
| 3 | Infrastructure |

### 3.2 Rules — verbatim

The table records the stable rule names, shapes, categories, confidence, and
paid-for reasons. `config/sensitive-rules.toml` is the reviewed authoring
source; generated Rust is not edited directly. Dialect: the Rust `regex` crate
— **no lookaround, no backreferences**.

`⚠ INERT` marks a rule deliberately tuned below the 0.70 auto-wipe floor.

> **Markdown fidelity caveat:** alternation pipes inside the table cells are
> written `\|` so the table renders. Read every `\|` as a literal `|`. When
> copying a regex out of this document, unescape them first, or take the
> authoritative text from `config/sensitive-rules.toml`.

| # | Pattern name | Regex (verbatim) | Cat | Conf | Notes |
|---|---|---|---|---|---|
| 0 | `aws_access_key` | `\b(?:AKIA\|ASIA)[0-9A-Z]{16}` | 0 | 0.99 | Leading `\b` stops mid-token hits (`XAKIA…`). **No trailing `\b` on purpose**: ASIA temp keys may carry trailing digits, and `E1` has no boundary between two word chars. |
| 1 | `github_fine_grained` | `github_pat_[A-Za-z0-9]{22}_[A-Za-z0-9]{59}` | 0 | 0.99 | |
| 2 | `github_classic_pat` | `ghp_[A-Za-z0-9]{36}` | 0 | 0.99 | |
| 3 | `github_actions_token` | `ghs_[a-zA-Z0-9]{36}` | 0 | 0.99 | |
| 4 | `openai_new` | `sk-proj-[A-Za-z0-9]{48}` | 0 | 0.99 | |
| 5 | `openai_legacy` | `\bsk-[A-Za-z0-9]{48}\b` | 0 | 0.95 | Must NOT double-fire on `sk-proj-` keys. Exclusion is **structural, not lookahead**: the hyphen after `proj` breaks the contiguous 48-char alnum run.; the earlier "(?!proj-) lookahead" comment was wrong and was corrected in P2 `r6cw` |
| 6 | `anthropic` | `sk-ant-api\d{2}-[A-Za-z0-9_-]{80,}` | 0 | 0.99 | |
| 7 | `stripe_live` | `sk_live_[0-9A-Za-z]{24}` | 0 | 0.99 | Live keys only; `sk_test_` deliberately not matched |
| 8 | `stripe_webhook` | `whsec_[a-zA-Z0-9]{32,64}` | 0 | 0.99 | |
| 9 | `npm_token` | `npm_[A-Za-z0-9]{36}` | 0 | 0.99 | |
| 10 | `pypi_token` | `pypi-[A-Za-z0-9_-]{180,}` | 0 | 0.99 | |
| 11 | `slack_bot` | `xoxb-[0-9]{11}-[0-9]{11}-[a-zA-Z0-9]{24}` | 0 | 0.99 | Only `xoxb`; `xoxa/xoxp/xoxr/xoxs` are **gaps** |
| 12 | `slack_webhook` | `https://hooks\.slack\.com/services/T[A-Z0-9]+/B[A-Z0-9]+/[a-zA-Z0-9]+` | 0 | 0.99 | |
| 13 | `discord_bot_token` ⚠ INERT | `\b[MN][a-zA-Z\d]{23,25}\.[\w-]{6}\.[\w-]{27,38}\b` | 0 | **0.65** | **P2 fb3e**: lowered 0.85 → 0.65 + added `\b`. Shape fires on any dot-separated base64url triple, not just Discord. |
| 14 | `twilio_signing_key_sid` ⚠ INERT | `\bSK[a-f0-9]{32}\b` | 0 | **0.65** | **P2 fb3e**: was named `twilio_auth_token`; the regex actually matches a Twilio **Signing-Key SID**, not the auth token. Real auth tokens are bare 32-hex with no prefix and are *not regex-distinguishable*. Renamed, `\b`-anchored, dropped below the floor. |
| 15 | `google_api_key` | `AIza[0-9A-Za-z\-_]{35}` | 0 | 0.99 | |
| 16 | `heroku_api_key` | `(?i)heroku[^\n]{0,50}[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}` | 0 | 0.95 | Context-anchored on the word `heroku` within 50 chars of a UUID |
| 17 | `hashicorp_vault` | `hvs\.[A-Za-z0-9]{32,}` | 0 | 0.95 | `{32,}` minimum added to kill FP on short `hvs.`-prefixed strings. |
| 18 | `gcp_oauth` | `GOCSPX-[A-Za-z0-9_-]{28}` | 0 | 0.99 | |
| 19 | `ssh_private_key` | `-----BEGIN (?:RSA \|EC \|OPENSSH \|DSA \|)?PRIVATE KEY-----` | 0 | 0.99 | Trailing empty alternative matches bare PKCS#8 `-----BEGIN PRIVATE KEY-----` |
| 20 | `ssh_private_key_pkcs8_encrypted` | `(?m)^-----BEGIN ENCRYPTED PRIVATE KEY-----` | 0 | 0.99 | **Audit MED #5** — real miss: rule 19 does not cover the `ENCRYPTED` header. `(?m)` so `^` anchors per line inside a pasted blob. |
| 21 | `ssh_private_key_putty` | `(?m)^PuTTY-User-Key-File-[0-9]+:` | 0 | 0.99 | **Audit MED #5** — real miss: PuTTY `.ppk`. |
| 22 | `generic_bearer` ⚠ INERT | `(?i)\bBearer\s+[A-Za-z0-9\-._~+/]{20,}` | 0 | **0.65** | **P2 fb3e**: lowered 0.80 → 0.65. Fires on `Bearer YOUR_TOKEN_HERE` in curl examples and READMEs. Explicitly documented that a post-match entropy guard would be a **no-op** (the 20-char minimum already satisfies the strength check), so the confidence floor is the only correct control. |
| 23 | `generic_password_kv` | `(?i)(?:password\|passwd\|secret\|api_key\|apikey\|auth_token\|access_token\|client_secret\|refresh_token\|db_password)\s*[:=]\s*(\S{6,})` | 0 | 0.75 | **Only rule with a post-match validator** (§5.3). Capture group 1 is the value. **CopyPaste-2eet** added `access_token`, `client_secret`, `refresh_token`, `db_password`; `access_token`/`refresh_token` were a **genuine miss** (the others were already covered by unbounded `secret`/`password` substrings). |
| 24 | `jwt` | `\beyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+` | 0 | 0.95 | **Audit MED #5**: `\b` added so `mykeyeyJabc.def.ghi` no longer classifies as a JWT. |
| 25 | `iban` ⚠ INERT | `\b[A-Z]{2}[0-9]{2}[A-Z0-9]{4}[0-9]{7}[A-Z0-9]{0,16}\b` | 1 | **0.65** | **P2 fb3e**, real FP with data loss: legitimately-copied IBANs (invoices, bank transfers) were being **silently auto-wiped**. Correct long-term fix is a mod-97 checksum validator (analogous to Luhn); dropping below the floor is the interim safe fallback. |
| 26 | `ssn_us` ⚠ INERT | `\b\d{3}[-\s]\d{2}[-\s]\d{4}\b` | 2 | **0.65** | **P2 fb3e**: lowered 0.80 → 0.65. Also matches dates/unit strings (`012 31 2024`, `012-31-2024`). Correct fix is the structural validator (group1 001–899, group2 01–99, group3 0001–9999, no all-zero group). |
| 27 | `email` ⚠ INERT | `\b[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}\b` | 2 | 0.60 | |
| 28 | `phone_us` ⚠ INERT | `(?:\+1[-.\s]?)?\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}\b` | 2 | 0.55 | No leading anchor — matches inside longer digit runs |
| 29 | `passport` ⚠ INERT | `\b[A-Z]{1,2}[0-9]{9}\b` | 2 | 0.55 | Min digits raised 6 → 9 to cut FP on order IDs / product codes. |
| 30 | `ip_with_port` ⚠ INERT | `\b(?:(?:25[0-5]\|2[0-4]\d\|[01]?\d\d?)\.){3}(?:25[0-5]\|2[0-4]\d\|[01]?\d\d?):\d{2,5}\b` | 3 | **0.65** | **CopyPaste-8ys1**, real FP with data loss: was **exactly 0.70** — on the floor — so RFC1918 IPs (`10.x`, `172.16–31.x`, `192.168.x`) in config files and docker-compose snippets silently expired. Rationale: bare `IP:port` is *topology*, not credential material; credentialed connections are caught by `db_conn_string` @0.99. |
| 31 | `db_conn_string` | `(?i)(?:postgresql\|postgres\|mysql\|mongodb\|redis\|amqp\|mssql)://[^@\s]*:[^@\s]*@\S+` | 3 | 0.99 | Requires `user:password@host` — that is what makes 0.99 safe |
| 32 | `aws_arn` | `\barn:aws:[a-z][a-z0-9\-]*:[a-z0-9\-]*:[0-9]{12}:[^\s]+` | 3 | 0.90 | ⚠ **Above the floor but not a secret** — see §7.1 |
| 33 | `dotenv_secret` | `(?m)^(?:export\s+)?[A-Z][A-Z0-9_]{2,}(?:_KEY\|_SECRET\|_TOKEN\|_PASSWORD\|_PASS\|_PWD\|_CREDENTIALS?)\s*=\s*\S+` | 3 | 0.80 | ⚠ Above the floor with **no value-strength validator** — see §7.1 |
| 34 | `azure_storage_key` | `AccountKey=[A-Za-z0-9+/]{86}==` | 0 | 0.90 | **P2 ozzt + bug-hunt HIGH finding.** A bare 88-char base64 blob is indistinguishable from a SHA-512 dump / Ed25519 key / random token; matching it bare at 0.90 would silently auto-wipe benign content. **Context anchor `AccountKey=` is mandatory** and is the reason this rule is allowed above the floor. |
| 35 | `azure_sas_token` | `(?i)\bsv=\d{4}-\d{2}-\d{2}\b[^\s]*&sig=[A-Za-z0-9%+/]{40,}` | 0 | 0.92 | **P2 ozzt.** Anchors only on the two stable, order-independent markers (`sv=` version and `&sig=`). The previous over-specified form (`s[a-z]=(?:b\|c\|f\|q)`) matched almost no real tokens — a **recall bug from over-specification**. |
| 36 | `gcp_service_account_key` | `(?m)"private_key"\s*:\s*"-----BEGIN RSA PRIVATE KEY-----` | 0 | 0.99 | **P2 ozzt.** Only matches the RSA header inside the JSON field; a service-account JSON with a PKCS#8 `-----BEGIN PRIVATE KEY-----` body still trips rule 19, so coverage is incidental, not designed |
| 37 | `cloudflare_api_token` | `(?i)\b(?:CLOUDFLARE_API_(?:TOKEN\|KEY)\|CF_API_TOKEN)\s*=\s*[A-Za-z0-9_-]{40}\b` | 0 | 0.92 | **P2 ozzt.** Cloudflare tokens have no standalone prefix, so the env-var context is mandatory — otherwise any 40-char alnum string would auto-wipe. |
| 38 | `sendgrid_api_key` | `\bSG\.[A-Za-z0-9_-]{22}\.[A-Za-z0-9_-]{43}\b` | 0 | 0.99 | **P2 ozzt** |
| 39 | `terraform_cloud_token` | `\batlasv1\.[A-Za-z0-9_-]{64,}\b` | 0 | 0.99 | **P2 ozzt** |

**Rule 23 tolerates one internal delimiter.** An underscore-only spelling of each
compound keyword with an underscore only, so `api-key:`, `access.token:` and
`auth token =` reached no rule at all: there is no bare `key` or `token`
keyword to fall back on, unlike `client secret` and `db-password`, which the
bare `secret` and `password` keywords already match inside. v2 accepts exactly
one `-`, `_`, `.` or space in `api…key`, `auth…token`, `access…token` and
`refresh…token`. The delimiter is not optional — written `api[-_. ]?key` the
literal the prefilter can require collapses from `api_key` to `api`, and every
rule in the set pays for that. The value-strength gate is unchanged, so a wider
keyword cannot make a weak value wipeable.

**Rule 23 is two rules (DMY-162).** The `api…key` and `apikey`
alternatives are `api_key_kv`, identical in confidence, threshold, shape guard
and deletion refusal, and carrying one thing the rest may not: gitleaks' counted
placeholder vocabulary. That is the only keyword family upstream already judges
by word list on the same bytes, and §5.6 gives the measurement for why it may
not be applied to the whole of rule 23.

### 3.3 Rule 41 — credit card (NOT a regex rule)

Card candidates use one space or one hyphen between digit groups, never a
newline, tab, mixed separator, or repeated separator. A candidate must satisfy
the 13–19 digit clamp, an issuer range, the brand's length, and Luhn through the
`card-validate` crate. Validation is ordered cheapest-first.

The card rule produces ordinary spans and a high-confidence classification,
but is `RESTRICTED`: a valid card number may also be an order or account ID, so
it is never auto-deleted (DMY-162). It is still withheld from FTS and sync and
its preview is masked. Audit MED #6 pins candidate scanning inside surrounding
text; a card need not be the whole clipboard value.

---

## 4. Confidence / threshold model and action rules

### 4.1 The numbers

| Constant | Value | Where |
|---|---|---|
| `CLASSIFY_CONFIDENCE_FLOOR` | **0.70** | `sensitive/finding.rs` |
| `AUTOWIPE_CONFIDENCE_FLOOR` | **0.85** | `sensitive/finding.rs` |
| Default sensitive TTL | **30 s** (off sentinel remains `0`) | `copypaste_ipc::ConfigData::sensitive_ttl_secs` |
| `sensitive_ttl_secs == 0` | sentinel: **auto-wipe disabled**, never clamped away | `config/mod.rs:248`, `:286` |
| FP-corpus budget | **zero unnamed false positives** | benign acceptance corpus |
| TP-corpus recall | **100 %**, ≥ 20 entries | `tests/true_positive_corpus.rs:368-391` |

### 4.2 What the bands mean

| Band | Members | Semantics |
|---|---|---|
| **0.90 – 0.99** | Prefixed/structural tokens with a unique literal or a mandatory context anchor | "Cannot plausibly be anything else." Safe to auto-delete on a unique literal; a rule resting on a context anchor alone is restricted instead (§5.6). |
| **0.75 – 0.80** | `generic_password_kv` (0.75), `api_key_kv` (0.75), `dotenv_secret` (0.80) | Keyword-driven. All gate the value on strength and randomness, and all are restricted (§5.6). |
| **≥ 0.70 classify floor** | — | Withholding boundary. Matches below this stay Flag. |
| **≥ 0.85 wipe floor** | — | Auto-wipe boundary. Nothing may sit *exactly* on it. `generic_api_key` at 0.75 is Restricted. |
| **0.55 – 0.65 — INERT** | `phone_us`, `passport`, `email`, `iban`, `ssn_us`, `discord_bot_token`, `twilio_signing_key_sid`, `generic_bearer`, `http_basic_auth`, `ip_with_port` | Detected, labelled, masked, redacted in logs — **never deleted**, and still searchable. |
| **RESTRICTED** | `credit_card`, `cloudflare_api_token`, `azure_storage_key`, `aws_secret_access_key`, `dotenv_secret`, `generic_password_kv`, `api_key_kv`, `heroku_api_key`, `generic_api_key` | Classified sensitive — never indexed, never synced, preview masked — but **never deleted**. Includes `never_auto_delete` rules and the 0.70–0.85 band. |

**The restricted band is explicit (DMY-162).** "How sure is this a card?" and
"may this be erased?" are different questions, and §3.3 records the case where the
answers diverge: the classification is certain and the deletion is unjustified.
Confidence keeps its meaning, and a separate per-rule `never_auto_delete` says
what a correct match licenses. It is opt-in and never derived, so a new rule
above the floor is deletable unless someone writes down why it must not be —
the direction that keeps a rule from dropping out of the gate by being
forgotten. The engine's two whole-item predicates differ by exactly this band:
`is_sensitive` withholds, `may_auto_wipe` deletes.

**The band is per rule; the verdict is per item (DMY-162).**
`never_auto_delete` says what *one rule's* match licenses, and on its own it
protects nothing: written as a plain disjunction over findings, `may_auto_wipe`
took any high-confidence match as permission, so `generic_api_key` at 0.75 —
judging the *same value* through the *same* anchor on weaker gates — reinstated
every deletion the band had just refused. Eight of eleven real credentials
across the four context-anchored rules were still destroyed with the band in
place. **A rule above the floor licenses deletion of the item only where no
restricted match covers the same bytes *and* the rule rests on a context anchor
alone.** Overlap, not containment: the generic rule's match can run a byte past
its neighbour's. Rules on *disjoint* spans are independent evidence and still
delete, so a distinct secret pasted beside a card or a `.env` line is not
protected by it.

**Only an anchor-only rule is overruled (DMY-162).** Written over
*every* match above the floor, the veto took the deletion from a secret with a
unique literal of its own whenever a weaker rule happened to cover it:
`GITHUB_TOKEN=ghp_…`, `VAULT_TOKEN=hvs.…`, `AUTH_TOKEN=<JWT>`,
`SLACK_TOKEN=xoxb-…` and `api_key: ghp_…` all stopped auto-wiping, against a
band table that calls a unique literal safe to delete and §9.1 rows that bind
`hvs.`+32 and the JWT as auto-wipes. `.env` is the commonest way a secret is
pasted, so this was most of the feature. Two rules over one span are one
judgement only where the second read the *same anchor* on weaker gates;
`anchor_only` is a per-rule declaration with a written decision, derived into the
predicate the way the band is, and it holds for exactly `generic_api_key`.

**The band only protects an anchor-only rule that has a neighbour (DMY-162).**
`anchor_only` withholds *where a restricted match covers the same bytes*, so a
rule standing alone still deletes. `heroku_api_key` was declared `anchor_only`
on the argument that a UUID beside the word `heroku` is "distinctive to
nothing", and that argument holds for `heroku config: <UUID>` with no neighbour
at all — an app, dyno, release and addon id are all UUIDs, and the CLI prints
them beside that word. It is `never_auto_delete` instead: a restricted rule
never deletes anywhere, and it cannot be reinstated by a generic neighbour
either. Read `anchor_only` as the flag for a rule that must still delete
*somewhere*.

**A restricted rule is load-bearing for *not* deleting
(DMY-162).** Before the aggregate rule, a restricted rule that stopped firing
cost withholding. It can now cost the item: the anchor-only match it was
vetoing becomes unvetoed and deletes. **Tightening any restricted rule is
therefore a potential data-loss change**, which is not what tightening a
suppressing gate normally means, and it is why raising `cloudflare_api_token`'s
threshold to reach one template would have been the wrong repair. A regression
test must pin the **pair** — the restricted match and the deletion it withholds
— rather than each rule alone, or the overlap can disappear silently.

Read a rule's own row in §9.1/§9.2 as what that rule does. Where a row says an
item "never auto-wipes", it is this aggregate rule that makes it true.

The inert band exists for two distinct reasons:

1. **Genuinely sensitive but user-owned data** (IBAN, SSN, email, phone,
   passport). The user *meant* to copy their own bank details. Flagging is
   helpful; deleting is hostile.
2. **Shapes too weak to prove a secret** (`discord_bot_token`,
   `twilio_signing_key_sid`, `generic_bearer`, `ip_with_port`). The right fix
   is a validator (mod-97, SSN structure, Discord snowflake decode); until one
   exists, the floor is the safe fallback.

### 4.3 Action rules

```
scan(text):
    normalised := NFKC(text) with default ignorables removed
    findings := every validated rule and card match over normalised

is_sensitive(findings):
    true when any finding reaches the classify floor or is restricted

may_auto_wipe(findings):
    true only for a high-confidence deletable finding that is not overruled by
    an overlapping restricted finding under the anchor-only rule
```

Password-manager provenance is an independent capture-time reason to withhold
an item. The stored item never carries plaintext into FTS or sync. A non-zero
TTL produces a derived deadline from `created_at`; the sweep re-opens and
reclassifies before deleting. Whole-sensitive previews contain no content,
while inert embedded findings expose only bounded redacted span metadata.

### 4.4 Cross-platform parity is part of the contract

macOS, Android, and Windows use the same Rust detector and the same rule table.
The platform adapters report facts; they do not reproduce confidence or expiry
logic.

| Defect | Binding rule |
|---|---|
| **AB-6a** | an inert match is not promoted to the whole-item verdict on one platform |
| **PG-23 / l9z8** | label, withholding, and deletion read the same finding bands |
| **PG-3 / 349q** | one capture decision owns classification and TTL derivation |

Platform boundaries may recover from a panic, but may not invent a different
verdict or compute expiry independently.

---

## 5. False-positive defences

### 5.1 NFKC normalisation — and the bypass it prevents

Applied before every match (I3). The bypass it closes: full-width Latin
letters/digits, compatibility ligatures, and other NFKC-foldable forms that
render as ASCII but do not match ASCII character classes. Pinned by
`nfkc_normalised_input_detects_secrets` using
`\u{FF21}\u{FF2B}\u{FF29}\u{FF21}` + 16 ASCII chars → must detect as an AWS
key.

Three consequences that must be carried:

- **Idempotence on ASCII** — `nfkc_normalize("AKIAIOSFODNN7EXAMPLE")` is the
  identity. This is what lets callers usually index
  spans into the original string.
- **Offsets belong to the normalised string.** Redaction must be applied
  against the same normalised string, and the byte→char offset mapping for UI
  masking must be computed over it.
- **Default-ignorable code points are stripped** after NFKC (ZWJ, ZWSP,
  variation selectors, …). A spliced secret such as `AKIA\u{200D}IOS…` must
  still match. Spans index the stripped string. Auto-wipe stays on the
  high-confidence band: inert PII with ignorables stripped must not become
  deletable (`AGENTS.md` rule 4).

### 5.2 Structural anchoring (the primary defence)

Most FP control is in the regex itself, not in post-filters:

- **`\b` word-boundary anchors** on every prefixed-token rule, so a token glued
  into a longer identifier does not match. Enforced by a meta-test over
  `sendgrid_api_key`, `terraform_cloud_token`,
  `cloudflare_api_token`, `twilio_signing_key_sid`, `discord_bot_token`.
- **Mandatory context anchors** where the token shape alone is not
  distinctive — `AccountKey=` (azure), `CLOUDFLARE_API_TOKEN=|CF_API_TOKEN=`
  (cloudflare), `heroku` within 50 chars (heroku), `"private_key":` (GCP SA),
  `user:password@` (db_conn_string). **This is the single most valuable
  technique in the whole ruleset** and must be preserved when re-sourcing from
  gitleaks: a bare high-entropy blob is never enough to justify deletion.
- **Deliberate omissions** are documented and must not be "fixed" blindly:
  `aws_access_key` has no trailing `\b` (ASIA keys carry trailing digits);
  `azure_storage_key` is excluded from the `\b` meta-test because it is
  context-anchored instead.
- **Minimum-length tightenings** driven by observed FPs: `hashicorp_vault`
  `{32,}`; `passport` 6 → 9 digits; `jwt` `\b`-prefixed.

### 5.3 Value-strength gate

Keyword-KV rules validate the captured value after the regex match. The value
is structurally strong if **any** of:

1. `value.chars().count() >= 10` — **characters, not bytes**;
2. contains one of `! @ # $ % ^ & * + / =`;
3. contains at least one ASCII letter **and** at least one ASCII digit.

**Bug + rule — the char-vs-byte gate:** a 9-character CJK value
(`私的秘密言葉確認鍵`) is 27 bytes. A byte-length gate would call it strong; the
char gate correctly calls it weak. Pinned by
`multibyte_value_gated_on_chars_not_bytes`. Scan, label, withholding, and wipe
all consume the same validated findings.

### 5.4 Luhn validation

The only checksum validator in the system. Two independent things it buys:

- 13–19-digit runs that are not Luhn-valid do not classify as `CreditCard`.
- The clamp `13 ≤ digits ≤ 19` rejects short/long numeric runs outright.

**Test-fixture bug worth recording:** the FP fixture `"4242424242422"` was
*accidentally Luhn-valid* (digit sum 50 ≡ 0 mod 10), so the "must not match"
test was passing vacuously. Replaced with `"4242424242421"` (sum 49).
Negative fixtures must prove they are actually negative.

`card-validate` carries the checksum, the issuer ranges and the
per-brand lengths together, and §3.3 records why the checksum alone was a
data-loss defect.

### 5.5 Entropy / variety gate — asymmetry to resolve

Selected rules carry reviewed entropy thresholds over the captured value.
Entropy is a suppressing gate: below-threshold text remains available to lower
confidence or inert rules and never gains deletion authority. Thresholds are
measured against both template-like text and random values over each rule's
alphabet; they are authored in `config/sensitive-rules.toml`.

Repository telemetry does not maintain a second credential ruleset. Publication
redaction is generated from the same selected rules, with the deliberate bias
toward over-redaction at an external boundary.

### 5.6 Allowlists

Selected upstream content allowlists, stopwords, and `gitleaks:allow` are
applied where `config/sensitive-rules.toml` admits them. Repository path and
commit allowlists have no clipboard equivalent and are not applied.

**An allowlist suppresses, so it fails closed.** `all` over an
empty set is true, so an `AND` allowlist carrying neither a regex nor a stopword
would silence every match of the rule it is attached to, with nothing to say so.
An allowlist with no checks allows nothing.

**Randomness, not a word list (DMY-162).** A context anchor
(§5.2) proves which *field* matched and says nothing about the *value*, so
`AccountKey=`, `CLOUDFLARE_API_TOKEN=`, `aws_secret_access_key =` and
`dotenv_secret`'s variable-name suffix all matched README examples at 0.80-0.99.
Each of those rules captures its value and gates it on two things it already
had: §5.3's value-strength model, and an **entropy threshold**.

**Two word lists were tried first and both were wrong.** A per-rule stopword
allowlist tested with `contains` suppressed any real Azure, Cloudflare, AWS or
dotenv credential carrying `todo` or `your` anywhere inside it — suppressed
*entirely*, not made inert, so it was neither flagged, nor masked, nor kept out
of `clipboard_fts`. Anchoring the same list to the *start* of the value made
that rarer and no safer: it moved the failure to credentials beginning with a
listed word, and it still fails **open**, which is the one direction the
fail-closed amendment above forbids. No finite word list closes either end.

Randomness does. A placeholder is repetitive or written out of words; a
credential is neither, and that is a property of the value rather than a guess
about its spelling. `AccountKey=<86 identical characters>` measures 0.0 and is
rejected; `AccountKey=your<82 random characters>` measures above 5 and is
detected, which is the correct answer and the one no word list could give.

**The number has to be measured, not inherited (DMY-162).** Upstream's
thresholds belong to a repository scanner, and §5.5 fixes the asymmetry: it
prefers a false positive, this detector prefers a false negative. Taking them
unchanged for four rules that then deleted at 0.80-0.99 imported the wrong bias,
and `cloudflare_api_token` had no number of its own at all — it inherited 2.0 and
deleted `CLOUDFLARE_API_TOKEN=YOUR_TOKEN_HERE_<24 X>`, which measures 2.233.
Each threshold is now set against two measured populations at the value shape
its pattern admits: 80 000 word-shaped templates per length, and 500 000 random
values over the rule's own alphabet.

| Rule | Value shape | Single-case word ceiling | Real values rejected | Threshold |
|---|---|---|---|---|
| `cloudflare_api_token` | 40 chars, 64 symbols | 4.225 | 3.4 in 10 000 | **4.3** |
| `aws_secret_access_key` | 40 chars, 64 symbols | 4.225 | 3.4 in 10 000 | **4.3** |
| `azure_storage_key` | 86 chars, 64 symbols | 4.268 | 0 in 500 000 | **4.8** |
| `dotenv_secret` | `\S+`, any alphabet | 4.268 | below | **4.4** |
| `generic_password_kv` / `api_key_kv` | `\S{6,}`, any alphabet | *unreachable* | 1 145 in 100 000 at 6 chars, 0 at 10 | **2.0** |

**4.225 is one population's ceiling, not the ceiling (DMY-162).**
The 80 000 templates were single-case and word-only. Ordinary README spelling is
mixed case *with digits*, and at the same 40 characters it clears 4.3:
`Replace_With_Your_Real_Token_Value123456` measures **4.354**,
`My_Cloudflare_Token_Goes_Right_Here_9876` 4.425 and
`Copy_Your_API_Token_From_The_Dashboard12` 4.325. Read the column above as the
ceiling of the population named beside it. Raising the threshold to cover the
gap is the wrong repair twice: 4.4 rejects 0.305 % of real 40-character tokens
against 4.3's 0.043 %, and by §4.2's second amendment it would *create*
deletions, because the restricted match vetoing `generic_api_key` disappears
with it.

`dotenv_secret` fixes neither a length nor an alphabet, so its number clears the
ceiling at *every* length `\S+` admits rather than at one. It is therefore the
tightest against its own values: a 32-character alphanumeric token clears 4.4
about 86 % of the time, and **no hex value ever can**, because log2(16) is 4.
Those fall to `generic_api_key` at 0.75, which still withholds them.

**`generic_password_kv` is the one rule where the template ceiling is out of
reach, and pretending otherwise would cost detections (DMY-162).** It carried no
threshold at all and deleted twelve of twelve plausible README / `.env` lines.
Its values are not machine-issued tokens: `password=` admits what a person
chose, so §9.1's own `password=hunter2` and `secret = !abcdef` measure 2.807 and
fix the ceiling from *below*. 2.5 rejects 19 % of six-character values and 2.8
rejects `passw0rd` at 2.750, while the word-spelled templates start at 3.516. So
2.0 answers only the **repetitive** half of the pair — `ACxxxx…` at 0.382,
`key-0000…` at 0.741 — and the shape guard answers the other. The templates
neither reaches are written in words *and* digits, and they belong to the third
gate below rather than to this number.

**Randomness leaves an overlap, so it is not the last gate.** A value whose
characters are nearly all distinct measures like a credential because by that
gate it *is* one: `MY_API_TOKEN=sk_test_abcdefghijklmnopqrstuvwx` measures 4.515
and was deleted, while the neighbouring `stripe_live` refuses that same prefix on
purpose (§3.2, rule 7). Two things close it, and neither is a word list.

**Gitleaks' own `^[a-zA-Z_.-]+$` secret allowlist** guards the four rules whose
values are machine-issued, borrowed from the vendored config rather than
restated so the pinned checksum governs it: a value carrying no digit and no
symbol is a template whatever it spells. The cost is a real value drawn without
one — about 1 in 900 at Cloudflare's 40 characters, 1 in 7,500 at AWS's, 1 in 57
million at Azure's 86, and 1 in 280 for a 32-character alphanumeric `.env`
value.

**the guard is wrong on the two keyword-KV rules, and was removed
(DMY-162).** Those odds are for values drawn at random by a machine.
`password:` and `api_key:` admit what a person or a deployment tool typed, for
whom "no digit and no symbol" is common rather than rare, so the guard's cost
there is paid in missed detections rather than in arithmetic:
`password: correcthorsebatterystaple` and `api_key: correcthorsebatterystaple`
reached **no rule at all**. And it bought nothing in exchange. Both rules are
`never_auto_delete`, so the guard could not prevent a single deletion; its only
reachable effect was to move a value from *classified and withheld* to *not
detected* — neither flagged, nor masked, nor kept out of `clipboard_fts`, which
is the direction this section's fail-closed amendment forbids and what I4
prohibits outright. `password: abcdefghij` returns to §9.1 as classified and
never auto-wiped, and the §9.2 rows the guard closed are `Restricted` instead.
The four machine-issued rules keep it.

**a third gate, counted, for one rule only (DMY-162).** A
template written in words *and* digits reaches neither of the two above: it
carries a digit, so the shape guard leaves it, and it measures 3.5 to 4.4, above
every ceiling these rules may set. Six ordinary README lines sat there,
withheld from the index and from sync for nothing. The answer is gitleaks'
1 446-word placeholder vocabulary — borrowed from the vendored config, governed
by `config_sha256`, never restated — applied on a **count of distinct markers**
rather than on one:

| Rule | Minimum | Templates it closes | What one marker would have cost |
|---|---|---|---|
| `cloudflare_api_token` | **3** | the six §9.2 Cloudflare rows, at 3–6 markers each | 3 681 in 200 000 real tokens beginning `todo`; at 3 it is 46, and 0 with no marker |

This *is* a word list over the value, which the amendment above rejects for the
other rules, and it is defensible only here and only counted: the value shape is
fixed at 40 characters over 64 symbols and the population is machine-issued, and
the alternative was raising a threshold that §4.2 says would create deletions. A
minimum of one is the `contains` form this section already refused: §9.1 binds a
value that contains or begins with a single `todo`, `your`, `dummy` or `sample`,
and a count of one takes every one of them.

**the same list on `api_key_kv` failed open, and was removed
(DMY-162).** It was taken on the argument that `generic_api_key` already applies
this identical list at a minimum of one to these identical lines, so the borrow
only disagreed with upstream in the safe direction. The accounting was against
the wrong population. `api_key=` values are not random tokens; they are what a
person or a deployment tool wrote — hyphenated, word-shaped, with a digit — and
markers are ordinary there. `api_key: my-production-service-account-key-2024`
carries three (`account`, `product`, `service`),
`api_key = prod-datadog-agent-key-1a2b3c4d5e6f` two, and both reached **no rule
at all**: not flagged, not masked, plaintext into `clipboard_fts` and offered to
sync. Nor did the gate buy anything on the deletion side, because the rule is
`never_auto_delete` and cannot delete. Its only reachable effect was to move a
value from *classified and withheld* to *not detected*, trading a reversible cost
for an irreversible one against I1 and I4. The five §9.2 `api_key=` rows are
`Restricted` instead.

**The split is now only a name.** Borrowing the list onto the undivided
`generic_password_kv` would have suppressed **eight** §9.1 positives outright —
`value`, `acces` and `word` are all stopwords — which is why only the
`api[-_. ]key|apikey` alternative moved, into `api_key_kv`. With the list gone
that rule is `generic_password_kv` with a different keyword family and the same
threshold and deletion refusal; merging them back is a rename with no behaviour
change and is not part of this repair.

**Upstream's guard also has to be read against upstream's capture.**
`generic_api_key`'s group admits `=`, so `AccountKey=<86 letters>==` reaches it
with the base64 padding attached and `^[a-zA-Z_.-]+$` matches nothing — the
all-letter Azure template that the other five reject was deletable through the
one rule that missed it. Its own copy of the allowlist is widened to
`^[a-zA-Z_.-]+={0,3}$` through the generator's existing regex-override, with the
reason recorded there. The five that borrow the allowlist take the vendored
literal unchanged.

**All six are `never_auto_delete`.** The anchor proves the *field*, and nothing
available to any gate proves that a human-readable value is a credential, so a
template that clears every gate must cost searchability rather than the data
(I1). An accepted finding is `Restricted` — withheld from the index, from sync
and from previews, never a reason to delete. That band is the guarantee **only
together with §4.2's aggregate rule**; alone it was inert, because a generic
rule matching the same bytes deleted the item anyway. The gates decide how
often the band is entered. `generic_api_key` keeps its own 0.75 band and its own
stopwords, so a value these six refuse may still be classified there.

The generator refuses a value-gated rule that states no threshold of its own, and
pairs the shape guard and the deletion refusal each with a written decision, so
neither the silent inheritance nor a silent band change can recur.

**This costs the synthetic fixtures, and that is the trade.** Ten rules carried
`entropy_override = 0.0` for no reason but to admit a repeated-character
fixture from §9.1, which is the same as having no gate. Those overrides are
gone and §9.1's fixtures are now credential-*shaped*: random over the rule's own
alphabet. Only `hashicorp_vault` keeps an override, because it merges two
upstream rules whose thresholds differ and so has none to inherit.

A carve-out also disappears: `aws_secret_access_key` needed one for `example`,
because AWS's own published key *ends* `EXAMPLEKEY`. It measures 4.663, clears
4.3, and nothing tests for that word now.

### 5.7 Defence layers, in order

0. `org.nspasteboard.ConcealedType` pasteboard marker → item never captured
   (`copypaste-daemon/src/clipboard/monitor.rs:174-197`).
1. Source app on the password-manager list → forced sensitive regardless of
   content (§5.8).
2. NFKC normalisation.
3. Regex + structural/context anchors.
4. Post-match validators (value strength; Luhn).
5. Confidence floor at 0.70 → controls *deletion* only.
6. Storage guards: no FTS, no thumbnail, masked preview (§6).

---

## 5.8 The password-manager bundle-ID list

Current bundle IDs and name fragments:

```
com.1password.1password
com.1password.7.1password
com.agilebits.onepassword
com.agilebits.onepassword4
com.agilebits.onepassword-osx-helper
com.bitwarden.desktop
com.bitwarden.desktop.safari
com.keepassxc.keepassxc
org.keepassxc.keepassxc-browser
com.lastpass.lastpass
de.peterb.Dashlane
com.dashlane.dashlane
com.enpass.Enpass
net.sourceforge.keepass
com.stegosafe.StegSafe
com.webpas.webpas
com.roboform.roboform
com.nordpass.macos
com.logmeininc.lastpass
--- process-name fragments (substring-matched) ---
1password
bitwarden
keepass
dashlane
lastpass
enpass
nordpass
roboform
```

### How it is used

```
is_sensitive_app(id) := SENSITIVE_APP_BUNDLE_IDS.any(known => id.to_lowercase().contains(known))
```

The needle is the list entry and the haystack is the input, so
`com.agilebits.onepassword4` matches the entry
`1password` by substring, and `COM.1PASSWORD.1PASSWORD` matches
case-insensitively. The list deliberately favours withholding over exposure;
automatic deletion still requires the wipe-time verdict.

### Why it exists (the earned insight)

> "a freshly-copied password is often a random string with low confidence"

A strong, randomly-generated password matches **no** pattern in §3. Content
detection cannot see it. The app signal is the *only* thing that protects
password-manager copies, which makes I6 (always evaluate it) a security
invariant rather than a nicety.

### Maintenance rules

- Matching stays case-insensitive and independent of the exclusion list.
- The source-app lookup is bounded; failure does not disable content detection.
- Additions require positive and ordinary-app negative tests.

---

## 6. Interaction with storage

### 6.1 FTS exclusion (`CopyPaste-i6pp`)

**Rule: `is_sensitive = 1` ⇒ never written to `clipboard_fts`, never returned
by `search_items`.**

FTS5 shadow tables hold **plaintext** from the
application's perspective. SQLCipher encrypts pages at rest, but the main
`content` column additionally carries application-layer XChaCha20-Poly1305
ciphertext — the FTS index does not. So FTS is a strictly lower-security store,
and FTS5 offers no per-value transformation hook.

Four independent enforcement layers are binding:

| Layer | Guard |
|---|---|
| 1. Capture/write | a sensitive item is never offered to the index |
| 2. Update | sensitivity is re-read in the same transaction as an FTS write; sensitive or missing rows write nothing |
| 3. Query | search joins only non-sensitive text rows |
| 4. Bounded purge | current rules rescan indexed rows and remove stale sensitive text without deleting the clipboard item |

**Bug + rule — `CopyPaste-44rq.64` (TOCTOU):** the layer-2 sensitivity check
must never occur: a concurrent sensitivity change cannot race an FTS write.
The sensitivity read and write share one transaction. The bounded current-
schema purge is idempotent and accounts for rules added after capture.

**Accepted consequence:** sensitive items are not searchable by content, only
by `item_id` or via the history list. The policy is unconditional.

### 6.2 Sensitive TTL / expiry

**The deadline is derived, not stored.** A sensitive
row's deadline is `created_at + ttl`, evaluated by the one predicate in
`copypaste_core::sensitive::sweep_sensitive`. Three of the four bugs in this
section are shapes of "the deadline was stored, and the store drifted from the
rule" — `CopyPaste-3e7y` (two predicates disagreed and a row with no
`expires_at` outlived its TTL), `CopyPaste-44rq.62` (backfill and delete in
separate transactions) and `CopyPaste-8ebg.2` (a dedup bump inherited a stale
deadline). None of them is reachable without the column: there is nothing to
backfill, nothing to keep atomic with the delete, and the bump resets the
deadline for free because it restamps `created_at`. The `0` sentinel, the pinned
exemption, the cheap existence probe and the startup purge remain binding.

**The wipe decision is re-derived from the plaintext.** The sweep re-scans the
candidate row at wipe time and requires `Severity::HighConfidence`, so a row is deleted only if it was
flagged at capture *and* is still above the floor. Deleting on a flag written by
a ruleset that has since changed is a decision nobody can review before it
fires, and `AGENTS.md` rule 4 ranks that above the cost of one AEAD per expired
candidate.

**Consequence, stated rather than hidden:** changing the TTL re-dates every
existing sensitive item rather than only new ones.

| Property | Value |
|---|---|
| Config field | `sensitive_ttl_secs`; `0` disables and is never clamped |
| Deadline | `created_at + ttl`; no stored expiry column |
| Pinned items | never expire |
| Wipe gate | re-open and reclassify; delete only `Severity::HighConfidence` |

- `CopyPaste-8ebg.1` / `CopyPaste-8ebg.8`: one TTL field is wired end to end;
  retired setting names are rejected.
- `CopyPaste-8ebg.2`: a duplicate re-copy restamps `created_at`, restarting the
  derived deadline.
- `CopyPaste-3e7y` / `CopyPaste-44rq.62`: one predicate and one transaction own
  selection, deletion, and index cleanup.

**Fail-closed probe:** "are there sensitive unpinned rows?" returns `true` on
query error so the sweep always runs.

### 6.3 Other storage effects

- Thumbnails suppressed for sensitive items (I5).
- History preview replaced with `[sensitive — id:XXXXXXXX]`
  (`ipc/handlers_items_read.rs:268-270`).
- `CopyPaste-mnte`: the detector is constructed **once per page**, not per
  item, and the page normalises once and calls `detect_normalised` to avoid a
  second NFKC pass (`handlers_items_read.rs:261-264`, `:286-296`). Keep the
  "normalise once, detect on the normalised form" split in v2.

---

## 8. gitleaks mapping

The current selection is sourced from the pinned gitleaks defaults plus the
local overlay. This map records how the named contract rules relate to upstream
rule IDs.

Rule IDs and regexes change between releases. Updating the pin requires review
of every selected rule, override, allowlist, confidence, and acceptance row.

| # | Contract rule | gitleaks rule id | Notes |
|---|---|---|---|
| 0 | `aws_access_key` | `aws-access-token` | gitleaks is broader (`A3T[A-Z0-9]`, `ABIA`, `ACCA` too) — a **recall win** |
| 1 | `github_fine_grained` | `github-fine-grained-pat` | |
| 2 | `github_classic_pat` | `github-pat` | |
| 3 | `github_actions_token` | `github-app-token` | gitleaks also covers `ghu_`, `gho_`, and `ghr_` forms |
| 4,5 | `openai_new`, `openai_legacy` | `openai-api-key` | upstream adds a distinctive infix instead of relying on length alone |
| 6 | `anthropic` | `anthropic-api-key` (+ `anthropic-admin-api-key`) | verify presence in the pinned version |
| 7 | `stripe_live` | `stripe-access-token` | gitleaks covers `sk_`/`rk_` × `test`/`live`/`prod` |
| 8 | `stripe_webhook` | — | **no equivalent**; keep as a custom rule |
| 9 | `npm_token` | `npm-access-token` | |
| 10 | `pypi_token` | `pypi-upload-token` | gitleaks anchors the `AgEIcHlwaS5vcmc` payload — better than length-only |
| 11 | `slack_bot` | `slack-bot-token` | gitleaks also ships app, user, and config-token rules |
| 12 | `slack_webhook` | `slack-webhook-url` | |
| 13 | `discord_bot_token` | `discord-api-token` (partial) | upstream keyword-anchors on "discord"; the local shape-only rule stays inert |
| 14 | `twilio_signing_key_sid` | `twilio-api-key` | same `SK`+32-hex shape; gitleaks keyword-anchors. Keep sub-floor unless the context anchor is adopted |
| 15 | `google_api_key` | `gcp-api-key` | |
| 16 | `heroku_api_key` | `heroku-api-key` | |
| 17 | `hashicorp_vault` | `vault-service-token` (+ `vault-batch-token` for `hvb.`) | upstream has tighter lengths and covers the batch prefix |
| 18 | `gcp_oauth` | `gcp-oauth-client-secret` (verify) | `GOCSPX-` |
| 19,20 | `ssh_private_key`, `…_pkcs8_encrypted` | `private-key` | one upstream rule covers both headers and additional standard forms |
| 21 | `ssh_private_key_putty` | — | **no equivalent**; keep as a custom rule (real bug fix, Audit MED #5) |
| 22 | `generic_bearer` | `generic-api-key` (partial) | gitleaks adds an entropy threshold; consider replacing outright |
| 23 | `generic_password_kv` | `generic-api-key` | keyword list, entropy, and stopwords; keep the char-not-byte strength rule |
| 24 | `jwt` | `jwt` | requires both header and payload base64 prefixes |
| 25 | `iban` | — | gitleaks is secrets-only, **no PII rules**. Keep as a custom rule; add mod-97 |
| 26 | `ssn_us` | — | custom; add the structural validator |
| 27 | `email` | — | custom |
| 28 | `phone_us` | — | custom |
| 29 | `passport` | — | custom |
| 30 | `ip_with_port` | — | custom (and arguably drop entirely) |
| 31 | `db_conn_string` | — | no single default equivalent; keep custom (it is one of the best rules in the set) |
| 32 | `aws_arn` | — | not a secret; gitleaks correctly has no rule. **Drop or demote** |
| 33 | `dotenv_secret` | `generic-api-key` (partial) | replace with gitleaks' entropy-gated generic rule |
| 34 | `azure_storage_key` | — (cf. `azure-ad-client-secret`) | keep custom; **preserve the `AccountKey=` context anchor** |
| 35 | `azure_sas_token` | — | keep custom |
| 36 | `gcp_service_account_key` | `private-key` (incidental) | keep custom for the JSON-field anchor |
| 37 | `cloudflare_api_token` | `cloudflare-api-key` / `cloudflare-global-api-key` / `cloudflare-origin-ca-key` | gitleaks is broader; **preserve the env-var context anchor** for the token form |
| 38 | `sendgrid_api_key` | `sendgrid-api-token` | |
| 39 | `terraform_cloud_token` | `terraform-api-token` | gitleaks requires the `<14-char org>.atlasv1.` prefix — more precise |
| 41 | credit card (Luhn) | — | secrets-only; keep custom, keep Luhn |

### 8.1 Non-negotiables when adopting gitleaks rules

1. **Every adopted rule needs a confidence assignment and must respect the
   0.70 floor.** gitleaks has no notion of "detected but inert"; that is *our*
   model and it is what prevents data loss.
2. **Do not lose the context anchors** (§5.2). gitleaks' entropy + keyword
   allowlists are a good substitute for some of them, but where this contract requires
   `AccountKey=` / `CLOUDFLARE_API_TOKEN=` / `user:password@`, that requirement
   is the reason the rule is allowed above the floor at all. It
   is not the reason it may delete — an anchor proves the field and not the
   value, so those rules are restricted rather than deletable (§5.6).
3. **Keep the reviewed gitleaks allowlists/stopwords** (§5.6).
4. **Keep the PII rules and the card rule as a local overlay.** gitleaks will
   never ship IBAN/SSN/email/phone/passport/credit-card.
5. **Keep the per-rule provenance comments.** Every `CopyPaste-*` / `P2 *` /
   `Audit MED #*` note in §3 records a real production miss or a real deletion
   of user data. Carry the *reasons* forward even when the regex is replaced.

### 8.2 v2 vendored source

The v2 rules are generated from Gitleaks `v8.30.1`, commit
`83d9cd684c87d95d656c1458ef04895a7f1cbd8e`. The exact default config is
vendored at `config/gitleaks/gitleaks.toml` with SHA-256
`e163e53b9e7e8a8511e77271e2b323ed057759542a6d988258afe3a1fa329caf`;
the adjacent upstream license is MIT. `config/sensitive-rules.toml` is the
review boundary: it lists only selected upstream IDs and records every regex,
entropy, keyword or allowlist override needed by this manifest.

Normal builds and tests never download. The explicit maintenance path is
`cargo run -p copypaste-sensitive-rules -- update`, then `generate`, then
`check`. The in-process engine applies selected upstream keywords, entropy,
stopwords, secret/match/line regex allowlists and `gitleaks:allow`. Repository
path and commit allowlists have no clipboard equivalent and are rejected or
omitted according to the reviewed decision in the selection file. The global
repository allowlist is not applied because it suppresses binding synthetic
true-positive fixtures; relevant per-rule content allowlists remain enabled.

---

## 9. Acceptance tests

These rows bind the current engine, validators, redaction, storage interaction,
and cross-platform adapters.

### 9.1 True positives — MUST be detected

**Fixture rule (DMY-162).** Where a row below is written `<prefix>` + *n*×`A`,
read *n* **random** characters over that rule's own alphabet, not *n* identical
ones. Every rule above the floor gates on its value's entropy (§5.6), so a
repeated-character fixture is a placeholder by construction and is rejected on
purpose; spelling these as repeats is what forced ten `entropy_override = 0.0`
entries and left the gate off. The lengths and the structure are what bind.

For the same reason the Slack and GitHub App rows no longer spell one out. A
literal of a checksum-free provider shape is one every credential scanner must
treat as live, so each is a value the repository scan and GitHub push
protection have to be told separately to ignore.

| Input | Expected |
|---|---|
| `AKIAIOSFODNN7EXAMPLE` | detected; `AwsKey`; **auto-wipes** |
| `AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE` | detected |
| `ASIAIOSFODNN7EXAMPLE1234` | detected (trailing digits must not break it) |
| `ghp_` + 36×`A` | detected |
| `github_pat_` + 22×`A` + `_` + 59×`B` | detected |
| `ghs_` + 36×`A` | detected |
| `sk-proj-` + 48×`A` | detected; **`openai_legacy` must NOT also fire** |
| `sk-` + 48×`A` | detected; auto-wipes |
| `sk-ant-api03-` + 80×`A` | detected |
| `sk_live_` + 24×`A` | detected |
| `whsec_aAbBcCdDeEfFgGhHiIjJkKlLmMnNoOpPqQrRsStT` | detected |
| `npm_` + 36×`A` | detected |
| `xoxb-` + 11 digits + `-` + 11 digits + `-` + 24×`A` | detected |
| `https://hooks.slack.com/services/T00000000/B00000000/` + 24×`X` | detected |
| `AIzaSyD-9tSrke72EmVt4TenJheB96ABCDE12345` | detected |
| `hvs.` + 32×`A` | detected; **auto-wipes** |
| `-----BEGIN RSA PRIVATE KEY-----\nMIIEo...` | `SshPrivateKey`; auto-wipes |
| `-----BEGIN OPENSSH PRIVATE KEY-----` | `SshPrivateKey` |
| `-----BEGIN EC PRIVATE KEY-----` | `SshPrivateKey` |
| `-----BEGIN PRIVATE KEY-----` (bare PKCS#8) | `SshPrivateKey` |
| `garbage prefix\n-----BEGIN ENCRYPTED PRIVATE KEY-----\nMIIFD...` | `SshPrivateKey` (Audit MED #5) |
| `PuTTY-User-Key-File-2: ssh-rsa\nEncryption: none\n…` | `SshPrivateKey` (Audit MED #5) |
| `# SSH key below\n-----BEGIN RSA PRIVATE KEY-----\n…` | detected (header mid-blob, not line 1) |
| `eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c` | `Jwt`; auto-wipes |
| RS256 JWT (longer header segment) | detected |
| `Bearer <that JWT>` | detected |
| `postgresql://alice:S3cr3tP@ss@db.example.com:5432/mydb` | detected |
| `mysql://root:hunter2@127.0.0.1:3306/prod` | detected |
| `mongodb://admin:P@ssw0rd!@mongo.internal:27017/mydb?authSource=admin` | detected |
| `redis://:my_redis_secret_password@redis.example.com:6379/0` | detected (empty username) |
| `access_token=abc123XYZlongvalue99` | detected (`CopyPaste-2eet`) |
| `access_token: gh_access_abc123XYZ` | detected (colon separator) |
| `export access_token=abc123XYZlongvalue99` | detected |
| `client_secret=Sup3rS3cr3tV@lue!` | detected |
| `refresh_token=rt_abc123XYZlong_value` | detected |
| `refresh_token = rt_PROD_abc123XYZlongval` | detected (ini spacing) |
| `db_password=S3cur3Pass!word` | detected |
| `password=hunter2` | detected (letter+digit); 2.807, and the ceiling `generic_password_kv`'s threshold is measured against (§5.6) |
| `secret = !abcdef` | detected (special char) |
| `SG.` + 22×`A` + `.` + 43×`B` | detected; auto-wipes |
| `atlasv1.` + 64×`A` | detected; auto-wipes |
| `{"private_key": "-----BEGIN RSA PRIVATE KEY-----\nMIIEo..."}` | detected; auto-wipes |
| `AccountKey=` + 86 random base64 chars + `==` | detected; classified, **never auto-wipes** — including when `generic_api_key` matches the same bytes (§4.2, §5.6, DMY-162) |
| an Azure, Cloudflare, AWS or dotenv value of a credential's randomness that *contains or begins with* `todo`, `your`, `dummy` or `sample` | detected; classified, never auto-wipes — nothing tests the value for words (§5.6), and the aggregate rule holds whether or not a generic rule also matches (§4.2) |
| `aws_secret_access_key = wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY` | detected; classified, never auto-wipes — AWS's published secret carries a `/`, so the shape guard leaves it alone |
| a real `password=`, `api_key:` or `client_secret=` value | detected; classified, **never auto-wipes** — the keyword proves the field and nothing proves the value (§4.2, §5.6) |
| `heroku config: <UUID>`, `heroku app id = <UUID>`, `heroku addon: <UUID>` | detected; classified, **never auto-wipes** — the value is a UUID and the word is the whole evidence, so an app, dyno, release or addon id is indistinguishable from a key (§4.2, DMY-162). A bare UUID with no context word reaches no rule |
| a card or a `.env` credential with a **distinct** secret beside it (`ghp_…`, `AKIA…`, a PEM header) | detected; the item **auto-wipes** — a disjoint span is independent evidence, not the same judgement twice (§4.2) |
| `GITHUB_TOKEN=ghp_…`, `VAULT_TOKEN=hvs.…`, `AUTH_TOKEN=<JWT>`, `SLACK_TOKEN=xoxb-…`, `api_key: ghp_…`, `password=sk-…` | detected; the item **auto-wipes** — a unique literal is independent evidence *over the same bytes*, and a restricted match must be shown to cover it or the test passes on a fixture that never reached the outer rule (§4.2, DMY-162) |
| `4111111111111111` | `CreditCard`; classified, **never auto-wipes** (§3.3, DMY-162) |
| `Customer card: 4111 1111 1111 1111 — expires 12/26` | `CreditCard` (Audit MED #6); classified, never auto-wipes |
| `please charge 4111-1111-1111-1111 today` | `CreditCard`; classified, never auto-wipes |
| `378282246310005` / `3782 822463 10005` (Amex 4-6-5) | `CreditCard`; the bare and grouped spellings must agree |
| `30569309025904` / `3056 930902 5904` (Diners 4-6-4) | `CreditCard` |
| `5555555555554444`, `6011111111111117`, `3530111333300000` | `CreditCard`, bare and in 4-4-4-4 groups |
| `api-key: abc123XYZlong`, `api key: abc123XYZlong`, `access-token: rt_abc123XYZlong_value`, `auth token = abc123XYZlongvalue99` | detected — the delimiter spellings of rule 23; the `api[-_. ]key` half is `api_key_kv` (§5.6) |
| `my_api_key = "correcthorsebatterystaple"`, `export MY_API_KEY="S3cr3tValue123"` | detected — a quoted value keeps its quotes in the capture, and one placeholder marker is not enough to suppress it |
| `api_key: my-production-service-account-key-2024`, `api_key = prod-datadog-agent-key-1a2b3c4d5e6f`, `apikey=github-actions-deploy-token-2026` | detected; classified, never auto-wipes — a deployment tool's key is hyphenated and word-shaped, and a counted vendored word list took all three (§5.6, DMY-162) |
| `client secret = Sup3rS3cr3tV@lue!`, `db-password: S3cur3Pass!word` | detected via the bare `secret` / `password` keyword, with no branch of their own |
| `password: abcdefghij`, `password: correcthorsebatterystaple`, `api_key: correcthorsebatterystaple` | detected; classified, never auto-wipes — an all-letter value is ordinary for what a person types, and the `^[a-zA-Z_.-]+$` guard that closed these sent them to `clipboard_fts` instead (§5.6, I4, DMY-162) |
| `\u{FF21}\u{FF2B}\u{FF29}\u{FF21}IOSFODNN7EXAMPLE` (full-width AKIA) | detected as AWS key **after NFKC** |
| 9 CJK chars, e.g. `私的秘密言葉確認鍵` as a KV value | value-strength = **weak** (char gate) |
| 10 CJK chars `私的秘密言葉確認鍵値` as a KV value | value-strength = **strong** |

### 9.2 False positives — MUST NOT match (or MUST NOT auto-wipe)

**Must produce no detection at all:**

| Input | Why |
|---|---|
| `Lorem ipsum dolor sit amet, consectetur adipiscing elit.` | plain prose |
| `fn main() { println!("Hello, world!"); }` | code |
| `password: foo` | value too short / no variety |
| `secret = nope` | value too short / no digit / no special |
| `access_token=short` | weak value (`CopyPaste-2eet` guard) |
| `refresh_token=abc` | weak value |
| `hvs.abc123` | below the `{32,}` vault minimum |
| bare 40×`A` (no `CLOUDFLARE_API_TOKEN=`) | context anchor missing |
| bare 86×`A` + `==` (no `AccountKey=`) | context anchor missing — **bug-hunt HIGH finding** |
| `SGfoo bar` | `SG` without the two-dot structure |
| `configsomethingeyJabc.def.ghi notajwt` | must NOT classify as `Jwt` (`\b` anchor) |
| `ref=4242424242421 EOT` | Luhn-invalid 13-digit run must NOT classify as `CreditCard` |
| `4111\n1111\n1111\n1111`, `4111\t1111\t1111\t1111` | a column is not a card: the separator may not be a newline or a tab (§3.3, DMY-162) |
| `4111 1111-1111 1111`, `4111  1111  1111  1111` | mixed or repeated separators are not a card spelling |
| `41111111 11111111`, `4111 111111111111` | the leading group is four digits, so 8+8 and 4+12 are not card spellings (§3.3, DMY-162) |
| `1234567890123452` | Luhn-valid, no issuer range — an order id, not a card |
| `9780132350883` | Luhn-valid ISBN-13 prefix — not an issuer range |
| `ISBN 978-012-13-234567 qty 12` | ISBN plus quantity must produce no card candidate |
| `order <Luhn-valid 16 digits beginning 4, 51, 35 or 60>` | indistinguishable from a card, so it *is* classified — and must never be **deleted** (§3.3, DMY-162) |
| `AccountKey=your` + 82×`A` + `==` | repetitive value, context anchor present (§5.6) |
| `CLOUDFLARE_API_TOKEN=your` + 36×`b` | repetitive value, and `dotenv_secret` must not classify it either |
| `aws_secret_access_key = your` + 36×`c` | repetitive value |
| `AccountKey=` + 86×`A` + `==`, `CLOUDFLARE_API_TOKEN=` + 40×`b` | **no word at all** — a repetitive value is a placeholder whatever it spells |
| `CLOUDFLARE_API_TOKEN=YOUR_TOKEN_HERE_` + 24×`X` | 2.233, under the rule's 4.3. The value is **40** characters, the length the pattern requires: written 38 it never reaches the rule, so the row constrains nothing and the test quoting it passes on the length (§5.4) |
| `CLOUDFLARE_API_TOKEN=REPLACE_WITH_YOUR_CLOUDFLARE_API_TOKEN_X`, `=your-cloudflare-api-token-goes-right-her` | 4.009 and 3.984 — ordinary README templates written at the 40 the rule requires |
| `aws_secret_access_key = REPLACE/WITH/YOUR/AWS/SECRET/ACCESS/KEYX`, `= your+aws+secret+access+key+goes+right+he` | 3.759 and 3.615, under 4.3 |
| `AccountKey=PutYourOwnAzureStorageAccountKeyHereBeforeDeployPutYourOwnAzureStorageAccountKeyHereOk==` | 4.198, under 4.8 — mixed case, and still a template |
| `MY_API_TOKEN=REPLACE_ME_WITH_THE_REAL_VALUE_PLEASE_OK`, `=changeme-please-before-you-deploy`, `=TODO_before_release` | 3.631, 3.820 and 3.321 — under `dotenv_secret`'s 4.4 |
| `MY_API_TOKEN=sk_test_abcdefghijklmnopqrstuvwx` | 4.515 — a credential's variety, and above every threshold these four carry. No digit and no symbol, so the `^[a-zA-Z_.-]+$` secret guard rejects it; `stripe_live` refuses the same prefix on purpose (§5.6, DMY-162) |
| `CLOUDFLARE_API_TOKEN=TheQuickBrownFoxJumpsOverTheLazyDogAbcde`, `=Deploy-Your-Own-Cloudflare-Token-Here-Ab` | 4.753 and 4.106 — over and under 4.3, and the shape guard rejects both |
| `aws_secret_access_key = ReplaceWithYourAwsSecretAccessKeyPleaseX` | 4.072, and no digit |
| an Azure, Cloudflare, AWS or dotenv value of 40 or 86 **random letters** | above every threshold these four carry, and still a template by shape: the guard has to hold where entropy cannot. `generic_api_key` had to be read against its own capture before this was true of the Azure spelling (§5.6) |
| `TWILIO_API_KEY=ACxxxx…` (0.382), `MAILGUN_API_KEY=key-0000…` (0.741) | repetitive, under `generic_password_kv`'s 2.0 |
| `export DATADOG_API_KEY=YOUR_DATADOG_API_KEY_HERE_PLEASE`, `=replace-with-your-datadog-api-key`, `STRIPE_API_KEY=sk_test_replace_me_before_you_deploy`, `SLACK_API_KEY=xoxb-put-your-own-workspace-token-here`, `OPENAI_API_KEY=sk-proj-REPLACE-ME-WITH-YOUR-OWN-KEY` | ordinary README / `.env` lines, no digit and no symbol. **Amended (DMY-162):** `Restricted` — classified, withheld, never auto-wiped — not "no detection at all". The shape guard that closed them took real all-letter passwords and keys with it (§5.6) |
| `api_key: PUT_YOUR_KEY_HERE_2024`, `=0000_REPLACE_THIS_WITH_A_REAL_TOKEN_1234`, `=paste_the_token_from_settings_here_2024ab`, `=YOUR_AZURE_COGNITIVE_SERVICES_KEY_2024`, `= "CHANGE_THIS_VALUE_BEFORE_YOU_SHIP_IT"` | words *and* digits, so neither the 2.0 nor the shape guard reaches them: 3.516 – 4.063, all above the 2.807 ceiling §5.6 measures. **Amended (DMY-162):** these are `Restricted` — classified, withheld, never auto-wiped — not "no detection at all". `api_key_kv`'s counted list closed them and took real word-shaped values with it (§5.6) |
| `CLOUDFLARE_API_KEY=Replace_With_Your_Real_Token_Value123456` | 4.354 at exactly the 40 characters the rule requires, so it clears 4.3 and falsifies §5.6's 4.225 ceiling for mixed case with digits. 4 markers, and `cloudflare_api_token`'s minimum of 3 is what closes it — **not** a higher threshold, which would create deletions (§4.2) |
| the 20 README / `.env` rows above | asserted as **one** corpus against all three public predicates, each row carrying its own verdict — `Restricted` where a gate was removed, no detection otherwise, and `!may_auto_wipe` for every one. Splitting the corpus into two tests left a row in neither half and untested (DMY-162) |
| `api-key: see the wiki`, `client secret: ask ops`, `db-password: ${VAULT_DB}` | the widened rule-23 keyword is still value-gated |

**The 50-entry benign corpus** (`tests/false_positive_corpus.rs:14-73`) must be
re-created verbatim. Highlights that specifically stress `generic_password_kv`:

```
the password is great, you should try it
my secret is to drink coffee every morning
I forgot my password again, time to reset it
password protected zip files are common
the auth token expired, please log in again
// example: set api_key=demo to enable test mode
# password: <set in your env file>
/* secret = TBD, fill in before deploy */
Note: passwd:enabled means SSH password auth is on
Set apikey: yourkey in the config (do not commit)
// auth_token: see README for setup
# api_key=demo for examples only
fn check_password(pw: &str) -> bool { pw.len() > 8 }
const SECRET_NAME = "prod-key";
let api_key = getEnv(); // value loaded later
const password = prompt('enter password:');
AWS region us-east-1 is recommended
see arn naming conventions in the AWS docs
the GitHub repo URL is https://github.com/example/repo
https://example.com/login?next=/dashboard
open the file at C:\Users\Public\Documents
the order number is 1234567890
tracking ID 0010 0020 0030
ticket #4815 has been assigned to you
version 1.2.3 was released yesterday
The api_key returns 401, please investigate
```

The target is **zero** false positives on this corpus.

**Must be detected but MUST NOT auto-wipe (the inert band):**

| Input | Rule | Why it matters |
|---|---|---|
| `Call me at (555) 867-5309` | `phone_us` 0.55 | |
| `Send to alice@example.com` | `email` 0.60 | |
| `Order AB123456789 is ready` | `passport` 0.55 | |
| `DE89370400440532013000` | `iban` 0.65 | user's own bank details |
| `012 31 2024` | `ssn_us` 0.65 | a date, not an SSN |
| `MNabcdefghijklmnopqrstuvwx.ABCDEF.ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456` | `discord_bot_token` 0.65 | |
| `SK` + 32×`a` | `twilio_signing_key_sid` 0.65 | |
| `Authorization: Bearer eyJhbGci0iJSUzI1NiIsInR5cCI6IkpXVCJ9` | `generic_bearer` 0.65 | **must still appear in detect() results** |
| `db_host=10.0.0.1:5432` | `ip_with_port` 0.65 | `CopyPaste-8ys1` |
| `172.16.0.5:6379` | `ip_with_port` 0.65 | Docker/VPC default range |
| `192.168.1.100:8080` | `ip_with_port` 0.65 | home/office LAN |
| `192.168.1.1:5432` | `ip_with_port` 0.65 | **must still appear in detect() results** with `confidence < 0.70` |

### 9.3 Structural / meta tests

| Test | Assertion |
|---|---|
| Every rule compiles | and the combined set builds |
| Rule count parity | compiled set length == declared rule count (no silent drops) |
| Confidence floor pin | `discord_bot_token`, `twilio_signing_key_sid`, `iban`, `ssn_us`, `generic_bearer`, `ip_with_port` all `< 0.70` |
| `\b` anchor pin | `sendgrid_api_key`, `terraform_cloud_token`, `cloudflare_api_token`, `twilio_signing_key_sid`, `discord_bot_token` contain `\b` |
| Context-anchor pin | `azure_storage_key` and `cloudflare_api_token` must NOT match without their anchors |
| Restricted-band pin | the rules that classify without deleting are exactly `credit_card` and the seven keyword/context-anchored rules — asserted as a set, so the band can neither spread nor lose one |
| Secret-shape pin | the four machine-issued rules carry gitleaks' `^[a-zA-Z_.-]+$` secret allowlist, borrowed from the vendored config rather than restated, and the two keyword-KV rules carry none; `generic_api_key`'s own copy admits base64 padding, because its capture swallows it (§5.6) |
| Anchor-only pin | the rules a restricted match may overrule are exactly `generic_api_key` — asserted as a set, derived from the rule table the way the band is, and refused on a rule that is itself restricted. `heroku_api_key` is restricted instead, because standing alone it had no neighbour to be overruled by (§4.2) |
| Counted-word-list pin | `cloudflare_api_token` is the only rule carrying a word list over its value; it is the vendored 1 446-entry list rather than a copy, and its minimum is above one (§5.6) |
| Fail-open pin | a word-shaped `api_key=` value a deployment tool would write is `is_sensitive` and not `may_auto_wipe`. Nothing pinned this while `api_key_kv` carried the counted list, and three such values reached no rule (§5.6, DMY-162) |
| Aggregate-verdict pin | a real credential of each of the four classes is `is_sensitive` and **not** `may_auto_wipe` even where a generic rule matches the same bytes — and a disjoint secret beside one still deletes. The per-rule assertion this replaces could not see the defect (§4.2, DMY-162) |
| Unique-literal pin | a `ghp_`, `hvs.`, JWT, Slack or OpenAI secret inside a `.env` or `api_key:` wrapper **does** auto-wipe, and the overlapping restricted match is asserted to exist, so the test cannot pass on a fixture that never reached the outer rule |
| Overlap-shape pin | `withholds` takes any shared byte and no fewer: identical, nested and off-by-one spans withhold, touching spans do not — and overlap alone is not sufficient, because the match must also be anchor-only |
| Aggregate-verdict cost | the ordered lookup answers what the nested scan answers, over thousands of `.env` lines where every high match is vetoed and neither loop can leave early. Asking the question by re-scanning the findings is quadratic in them: 4 MiB of `CLOUDFLARE_API_TOKEN=` cost 6.0 s against `scan_all`'s 1.3 s, and `sweep_sensitive` pays it once per expired row on every pass (DMY-162). Deterministic, not wall-clock |
| Category/confidence pin | each P2-ozzt rule is category 0 with confidence ≥ 0.90 |
| Idempotence | `nfkc_normalize` is the identity on ASCII |
| Perf | `detect()` over 10 MB of text stays within the release-test budget — pins that no rule is catastrophically backtracking |

### 9.4 Password-manager app tests

| Input | Expected |
|---|---|
| `com.1password.1password` | `true` |
| `com.bitwarden.desktop` | `true` |
| `com.keepassxc.keepassxc` | `true` |
| `com.dashlane.dashlane` | `true` |
| `bitwarden`, `keepass` (bare process names) | `true` |
| `com.Bitwarden.Desktop`, `COM.1PASSWORD.1PASSWORD` | `true` (case-insensitive) |
| `com.agilebits.onepassword4` | `true` (fragment substring) |
| `com.apple.finder`, `com.google.chrome`, `""` | `false` |
| with an **empty** app-exclusion list | still `true` for password managers (`CopyPaste-44rq.43`) |
| with a **non-empty** exclusion list containing an unrelated app | unchanged results (orthogonal signals) |

### 9.5 Redaction tests

| Case | Expected |
|---|---|
| no matches | input returned unchanged |
| match at start / middle / end | `***REDACTED***` in place, surrounding text intact |
| two disjoint matches | two placeholders |
| overlapping spans `[0,4)` + `[2,6)` | merged → **exactly one** placeholder |
| adjacent spans `[0,2)`,`[2,4)`,`[4,6)` | merged → one placeholder |
| span bounds mid-codepoint (`"héllo"`, span `[1,2)`) | must not panic; `é` fully covered |
| multibyte surroundings (`"🔑clé=hunter2🚀"`) | `hunter2` gone, emoji and `clé=` intact |
| end-to-end with the detector | `AKIA…` gone, `"normal text"` survives |
| end-to-end `password=hunter2` | `hunter2` gone |

### 9.6 Storage-interaction tests

| Test | Assertion |
|---|---|
| insert sensitive item with non-empty FTS text | **no** FTS row written |
| `upsert_fts` on a sensitive row | no-op, returns success |
| `upsert_fts` where sensitivity flips concurrently | check and write share one transaction (`CopyPaste-44rq.64`) |
| `search_items` with a manually-injected FTS row for a sensitive item | item never returned |
| bounded purge on current schema with stale sensitive FTS rows | index rows purged; clipboard items kept; idempotent; no-op on a clean history |
| insert sensitive item carrying a thumbnail | `thumb` stored as `NULL`; cannot be backfilled |
| sensitive item, `ttl = 30` | deadline derives as `created_at + 30_000` |
| sensitive item, `ttl = 0` | never swept |
| re-copy of an identical sensitive item | `created_at` restamps and the derived deadline restarts (`CopyPaste-8ebg.2`) |
| sensitive item that is **pinned** | never expired |
| sweep transaction | selection, delete, and FTS cleanup are atomic (`CopyPaste-3e7y`, `CopyPaste-44rq.62`) |
| "any sensitive rows?" probe errors | returns `true` (fail-closed) |
| history page for a sensitive item | preview is `[sensitive — id:XXXXXXXX]`, `sensitive_spans` empty |
| history page for a benign item containing an embedded secret | spans returned as **char** offsets into the NFKC-normalised preview |

### 9.7 Cross-platform parity tests

| Test | Assertion |
|---|---|
| same input → macOS gate and Android gate | identical verdict (0.70 floor on both) |
| `sensitive_kind()` non-null | implies the verdict is `true` (`PG-23`) |
| `capture_decision(text, now, 0)` | `expires_at_ms == None` even when sensitive |
| `capture_decision(benign, now, 30)` | `{false, None, None}` |
| detector panics | recover to "not sensitive" / empty spans, never abort |
| byte→char offset at `s.len()` | maps to the char count; out-of-range saturates |

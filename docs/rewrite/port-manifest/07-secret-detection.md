# Port Manifest 07 — Sensitive-Data (Secret) Detection

> STATUS: HARVESTED. Source of truth for the v2 rewrite of the sensitive-data
> subsystem. Written as an **implementation-independent specification**: it
> describes the *contract* (what must be detected, what must never be detected,
> what may be deleted), not the v1 Rust code.
>
> Harvested from `copypaste-core` @ `crates/copypaste-core/src/sensitive/**`,
> `crates/copypaste-telemetry/src/scrubber.rs`, `docs/adr/ADR-015-fts-sensitive-exclusion.md`,
> and the daemon/Android call sites.
>
> **Audit verdict carried in:** the *ruleset* should be re-sourced from the
> maintained [gitleaks](https://github.com/gitleaks/gitleaks) rules rather than
> hand-authored; the *in-process scanner* is an earned exception and stays (no
> Rust crate IS a ruleset). See §8 for the per-pattern gitleaks mapping.

---

## 1. Purpose & scope

### 1.1 What this subsystem decides

For every clipboard item the system captures, this subsystem answers three
questions:

| Question | Consumer | v1 entry point |
|---|---|---|
| **Is this a secret?** (binary, high bar) | daemon capture, Android capture | `is_sensitive_for_autowipe(text) -> bool` (`detector/kind.rs:54`) |
| **What kind of secret?** (label) | UI badge, logs | `detect(text) -> Option<SensitiveKind>` (`detector/kind.rs:58`) |
| **Where in the text?** (byte/char spans) | UI bullet-masking, log redaction | `SensitiveDetector::detect(_normalised)` (`detector/engine.rs:54,66`) |

Plus a content-independent signal:

| Question | Consumer | v1 entry point |
|---|---|---|
| **Did this come from a password manager?** | daemon capture (text + file) | `is_sensitive_app(bundle_id) -> bool` (`detector/apps.rs:37`) |

### 1.2 Why the stakes are asymmetric-but-both-bad

- A **false negative** means a secret is stored in plaintext in the FTS index,
  kept forever (no TTL), synced to peers/relay, and shown unmasked in history.
- A **false positive** means user data is **silently deleted** 30 seconds after
  it is copied (v1 default `sensitive_ttl_secs = 30`,
  `crates/copypaste-core/src/config/defaults.rs:51`). The user is not asked and
  is not told.

  **v2 ships this default as `0` (off).** Not because the confidence model
  failed — v2 fixed all three §7.1 rules — but because "is not told" became
  literal: v2's Settings has no control for the TTL and the sweep raises no
  notice, so the deletion is silent, irreversible and undiscoverable. The 30 s
  default is correct and should be restored once a Settings control and a
  visible notice both exist. See `copypaste_ipc::ConfigData::sensitive_ttl_secs`.

The entire confidence model in §4 exists to make the second failure mode
impossible for weakly-distinctive patterns.

### 1.3 In scope / out of scope

**In scope:** the ruleset, the confidence model, false-positive defences, the
password-manager app list, NFKC normalisation, Luhn validation, the FTS
exclusion rule, and the sensitive-TTL/expiry contract.

**Out of scope (cross-referenced only):** the `org.nspasteboard.ConcealedType`
pasteboard-marker skip (`crates/copypaste-daemon/src/clipboard/monitor.rs:72`,
`:174-197`) — this is *layer 0*, a pre-capture opt-out honoured by
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

> v1 pins this with a test: `fp_risk_patterns_below_autowipe_floor`
> (`sensitive/patterns.rs:382-403`).

### I2 — Detection and deletion are separate decisions

`detect()`/spans return **all** matches at all confidences. Only the auto-wipe
gate applies the 0.70 floor. A rule may be "detected but inert". Any API that
collapses the two (e.g. `is_sensitive() -> bool` over the whole ruleset,
`sensitive_kind()` returning a label for a 0.55 hit) is a **defect** — see
§7.4 for the divergences v1 accumulated and had to fix three times.

### I3 — Normalise before matching

Input MUST be NFKC-normalised before any regex runs
(`detector/normalize.rs:10`, applied at `engine.rs:55`, `kind.rs:59`,
`engine.rs:142`). Without it, `Ａ` (U+FF21 FULLWIDTH LATIN CAPITAL A) and
friends bypass every ASCII character class. All returned offsets are indices
into the **normalised** string, and this MUST be documented at every API
boundary that returns offsets (v1 does: `ffi_sensitive.rs:56-65`,
`engine.rs:49-53`).

### I4 — Sensitive items are never full-text indexed

`is_sensitive = 1` ⇒ never written to the FTS table, never returned by
search. Enforced at three independent layers (§6.1). Non-negotiable — this
was a real information-disclosure bug (`CopyPaste-i6pp`, ADR-015).

### I5 — Sensitive items carry no thumbnail

`is_sensitive = 1` ⇒ `thumb` is forced to `NULL` on insert and can never be
backfilled (`storage/items/insert.rs:22-33`, `:110-120`, `CopyPaste-44rq.49`).
A thumbnail of a password-manager screenshot is a readable secret.

### I6 — The app signal is independent of every other list

`is_sensitive_app` MUST be evaluated on every capture, regardless of whether
the user's app-exclusion list is empty or the frontmost-app lookup is
best-effort. v1 shipped a bug where an empty exclusion list short-circuited
the frontmost-app query, so `is_sensitive_app` always received `None` and
**every 1Password/Bitwarden copy was stored unflagged and never expired**
(`CopyPaste-44rq.43`, `daemon/capture/tick.rs:79-84`, regression test at
`tick.rs:419-489`).

### I7 — Fail-safe directions are fixed per-decision

| Decision | Failure mode on error/unknown |
|---|---|
| "Are there sensitive rows to sweep?" | **fail-closed** → assume yes, run the sweep (`storage/items/delete.rs:107`) |
| Regex set failed to compile | **fail-open to no-detection**, log an error, never panic on the clipboard hot path (`patterns.rs:300-308`, `:323-329`) — but `is_sensitive` must then fall back to the per-pattern path rather than returning `false` silently (`engine.rs:96-102`) |
| Frontmost-app lookup unavailable / non-macOS | **fail-open** (`false`) — content patterns still apply (`capture/tick.rs:48-55`) |
| Detector panics across the FFI boundary | recover to "not sensitive" / empty spans, never abort the JVM (`ffi_sensitive.rs:92-94`, `:253-274`) |

### I8 — Index alignment

If the implementation keeps parallel arrays (compiled regex ↔ name ↔ category
↔ confidence), a compile failure MUST NOT shift indices. v1's rule: degrade
the *whole* set to empty rather than `filter_map` a single bad pattern, which
would desync names from matches (`patterns.rs:311-330`). A v2 design that
carries name/confidence *with* the compiled rule makes this invariant
structurally unnecessary — prefer that.

### I9 — Redaction is span-merging and UTF-8-safe

Overlapping/adjacent match spans merge into one `***REDACTED***`; span bounds
are snapped outward to char boundaries so a mid-codepoint offset can never
panic and can only ever redact *more* (`sensitive/redact.rs:14-67`).

### I10 — Never log plaintext or the content hash next to it

(`daemon/capture/text.rs:86-88`.)

---

## 3. The full ruleset

### 3.1 Categories

| Index | Category | v1 name |
|---|---|---|
| 0 | Credential | `SensitiveCategory::Credential` |
| 1 | Financial | `SensitiveCategory::Financial` |
| 2 | PersonalId | `SensitiveCategory::PersonalId` |
| 3 | Infrastructure | `SensitiveCategory::Infrastructure` |

Source: `sensitive/patterns.rs:34`, `detector/engine.rs:12-29`.

### 3.2 Rules — verbatim

All 40 regexes below are copied verbatim from
`crates/copypaste-core/src/sensitive/patterns.rs:35-291`, in declaration order
(declaration order is load-bearing in v1 — see §7.2). Rust raw-string syntax
(`r"…"`) is stripped; the regex body is exact. Dialect: the Rust `regex` crate
— **no lookaround, no backreferences**.

`⚠ INERT` marks a rule deliberately tuned below the 0.70 auto-wipe floor.

> **Markdown fidelity caveat:** alternation pipes inside the table cells are
> written `\|` so the table renders. Read every `\|` as a literal `|`. When
> copying a regex out of this document, unescape them first, or take the
> authoritative text from `sensitive/patterns.rs:35-291`.

| # | Pattern name | Regex (verbatim) | Cat | Conf | Notes |
|---|---|---|---|---|---|
| 0 | `aws_access_key` | `\b(?:AKIA\|ASIA)[0-9A-Z]{16}` | 0 | 0.99 | Leading `\b` stops mid-token hits (`XAKIA…`). **No trailing `\b` on purpose**: ASIA temp keys may carry trailing digits, and `E1` has no boundary between two word chars. `patterns.rs:37-40` |
| 1 | `github_fine_grained` | `github_pat_[A-Za-z0-9]{22}_[A-Za-z0-9]{59}` | 0 | 0.99 | |
| 2 | `github_classic_pat` | `ghp_[A-Za-z0-9]{36}` | 0 | 0.99 | |
| 3 | `github_actions_token` | `ghs_[a-zA-Z0-9]{36}` | 0 | 0.99 | |
| 4 | `openai_new` | `sk-proj-[A-Za-z0-9]{48}` | 0 | 0.99 | |
| 5 | `openai_legacy` | `\bsk-[A-Za-z0-9]{48}\b` | 0 | 0.95 | Must NOT double-fire on `sk-proj-` keys. Exclusion is **structural, not lookahead**: the hyphen after `proj` breaks the contiguous 48-char alnum run. `patterns.rs:50-53`; the earlier "(?!proj-) lookahead" comment was wrong and was corrected in P2 `r6cw` (`detector/mod.rs:670-680`) |
| 6 | `anthropic` | `sk-ant-api\d{2}-[A-Za-z0-9_-]{80,}` | 0 | 0.99 | |
| 7 | `stripe_live` | `sk_live_[0-9A-Za-z]{24}` | 0 | 0.99 | Live keys only; `sk_test_` deliberately not matched |
| 8 | `stripe_webhook` | `whsec_[a-zA-Z0-9]{32,64}` | 0 | 0.99 | |
| 9 | `npm_token` | `npm_[A-Za-z0-9]{36}` | 0 | 0.99 | |
| 10 | `pypi_token` | `pypi-[A-Za-z0-9_-]{180,}` | 0 | 0.99 | |
| 11 | `slack_bot` | `xoxb-[0-9]{11}-[0-9]{11}-[a-zA-Z0-9]{24}` | 0 | 0.99 | Only `xoxb`; `xoxa/xoxp/xoxr/xoxs` are **gaps** |
| 12 | `slack_webhook` | `https://hooks\.slack\.com/services/T[A-Z0-9]+/B[A-Z0-9]+/[a-zA-Z0-9]+` | 0 | 0.99 | |
| 13 | `discord_bot_token` ⚠ INERT | `\b[MN][a-zA-Z\d]{23,25}\.[\w-]{6}\.[\w-]{27,38}\b` | 0 | **0.65** | **P2 fb3e**: lowered 0.85 → 0.65 + added `\b`. Shape fires on any dot-separated base64url triple, not just Discord. `patterns.rs:71-80` |
| 14 | `twilio_signing_key_sid` ⚠ INERT | `\bSK[a-f0-9]{32}\b` | 0 | **0.65** | **P2 fb3e**: was named `twilio_auth_token`; the regex actually matches a Twilio **Signing-Key SID**, not the auth token. Real auth tokens are bare 32-hex with no prefix and are *not regex-distinguishable*. Renamed, `\b`-anchored, dropped below the floor. `patterns.rs:81-88` |
| 15 | `google_api_key` | `AIza[0-9A-Za-z\-_]{35}` | 0 | 0.99 | |
| 16 | `heroku_api_key` | `(?i)heroku[^\n]{0,50}[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}` | 0 | 0.95 | Context-anchored on the word `heroku` within 50 chars of a UUID |
| 17 | `hashicorp_vault` | `hvs\.[A-Za-z0-9]{32,}` | 0 | 0.95 | `{32,}` minimum added to kill FP on short `hvs.`-prefixed strings. `patterns.rs:96` |
| 18 | `gcp_oauth` | `GOCSPX-[A-Za-z0-9_-]{28}` | 0 | 0.99 | |
| 19 | `ssh_private_key` | `-----BEGIN (?:RSA \|EC \|OPENSSH \|DSA \|)?PRIVATE KEY-----` | 0 | 0.99 | Trailing empty alternative matches bare PKCS#8 `-----BEGIN PRIVATE KEY-----` |
| 20 | `ssh_private_key_pkcs8_encrypted` | `(?m)^-----BEGIN ENCRYPTED PRIVATE KEY-----` | 0 | 0.99 | **Audit MED #5** — real miss: rule 19 does not cover the `ENCRYPTED` header. `(?m)` so `^` anchors per line inside a pasted blob. `patterns.rs:105-116` |
| 21 | `ssh_private_key_putty` | `(?m)^PuTTY-User-Key-File-[0-9]+:` | 0 | 0.99 | **Audit MED #5** — real miss: PuTTY `.ppk`. `patterns.rs:117-122` |
| 22 | `generic_bearer` ⚠ INERT | `(?i)\bBearer\s+[A-Za-z0-9\-._~+/]{20,}` | 0 | **0.65** | **P2 fb3e**: lowered 0.80 → 0.65. Fires on `Bearer YOUR_TOKEN_HERE` in curl examples and READMEs. Explicitly documented that a post-match entropy guard would be a **no-op** (the 20-char minimum already satisfies the strength check), so the confidence floor is the only correct control. `patterns.rs:123-136`, `detector/fp.rs:36-40` |
| 23 | `generic_password_kv` | `(?i)(?:password\|passwd\|secret\|api_key\|apikey\|auth_token\|access_token\|client_secret\|refresh_token\|db_password)\s*[:=]\s*(\S{6,})` | 0 | 0.75 | **Only rule with a post-match validator** (§5.3). Capture group 1 is the value. **CopyPaste-2eet** added `access_token`, `client_secret`, `refresh_token`, `db_password`; `access_token`/`refresh_token` were a **genuine miss** (the others were already covered by unbounded `secret`/`password` substrings). `patterns.rs:137-153` |
| 24 | `jwt` | `\beyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+` | 0 | 0.95 | **Audit MED #5**: `\b` added so `mykeyeyJabc.def.ghi` no longer classifies as a JWT. `patterns.rs:154-162` |
| 25 | `iban` ⚠ INERT | `\b[A-Z]{2}[0-9]{2}[A-Z0-9]{4}[0-9]{7}[A-Z0-9]{0,16}\b` | 1 | **0.65** | **P2 fb3e**, real FP with data loss: legitimately-copied IBANs (invoices, bank transfers) were being **silently auto-wiped**. Correct long-term fix is a mod-97 checksum validator (analogous to Luhn); dropping below the floor is the interim safe fallback. `patterns.rs:164-174` |
| 26 | `ssn_us` ⚠ INERT | `\b\d{3}[-\s]\d{2}[-\s]\d{4}\b` | 2 | **0.65** | **P2 fb3e**: lowered 0.80 → 0.65. Also matches dates/unit strings (`012 31 2024`, `012-31-2024`). Correct fix is the structural validator (group1 001–899, group2 01–99, group3 0001–9999, no all-zero group). `patterns.rs:175-181` |
| 27 | `email` ⚠ INERT | `\b[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}\b` | 2 | 0.60 | |
| 28 | `phone_us` ⚠ INERT | `(?:\+1[-.\s]?)?\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}\b` | 2 | 0.55 | No leading anchor — matches inside longer digit runs |
| 29 | `passport` ⚠ INERT | `\b[A-Z]{1,2}[0-9]{9}\b` | 2 | 0.55 | Min digits raised 6 → 9 to cut FP on order IDs / product codes. `patterns.rs:194-196` |
| 30 | `ip_with_port` ⚠ INERT | `\b(?:(?:25[0-5]\|2[0-4]\d\|[01]?\d\d?)\.){3}(?:25[0-5]\|2[0-4]\d\|[01]?\d\d?):\d{2,5}\b` | 3 | **0.65** | **CopyPaste-8ys1**, real FP with data loss: was **exactly 0.70** — on the floor — so RFC1918 IPs (`10.x`, `172.16–31.x`, `192.168.x`) in config files and docker-compose snippets silently expired. Rationale: bare `IP:port` is *topology*, not credential material; credentialed connections are caught by `db_conn_string` @0.99. `patterns.rs:197-211` |
| 31 | `db_conn_string` | `(?i)(?:postgresql\|postgres\|mysql\|mongodb\|redis\|amqp\|mssql)://[^@\s]*:[^@\s]*@\S+` | 3 | 0.99 | Requires `user:password@host` — that is what makes 0.99 safe |
| 32 | `aws_arn` | `\barn:aws:[a-z][a-z0-9\-]*:[a-z0-9\-]*:[0-9]{12}:[^\s]+` | 3 | 0.90 | ⚠ **Above the floor but not a secret** — see §7.1 |
| 33 | `dotenv_secret` | `(?m)^(?:export\s+)?[A-Z][A-Z0-9_]{2,}(?:_KEY\|_SECRET\|_TOKEN\|_PASSWORD\|_PASS\|_PWD\|_CREDENTIALS?)\s*=\s*\S+` | 3 | 0.80 | ⚠ Above the floor with **no value-strength validator** — see §7.1 |
| 34 | `azure_storage_key` | `AccountKey=[A-Za-z0-9+/]{86}==` | 0 | 0.90 | **P2 ozzt + bug-hunt HIGH finding.** A bare 88-char base64 blob is indistinguishable from a SHA-512 dump / Ed25519 key / random token; matching it bare at 0.90 would silently auto-wipe benign content. **Context anchor `AccountKey=` is mandatory** and is the reason this rule is allowed above the floor. `patterns.rs:230-242` |
| 35 | `azure_sas_token` | `(?i)\bsv=\d{4}-\d{2}-\d{2}\b[^\s]*&sig=[A-Za-z0-9%+/]{40,}` | 0 | 0.92 | **P2 ozzt.** Anchors only on the two stable, order-independent markers (`sv=` version and `&sig=`). The previous over-specified form (`s[a-z]=(?:b\|c\|f\|q)`) matched almost no real tokens — a **recall bug from over-specification**. `patterns.rs:243-253` |
| 36 | `gcp_service_account_key` | `(?m)"private_key"\s*:\s*"-----BEGIN RSA PRIVATE KEY-----` | 0 | 0.99 | **P2 ozzt.** Only matches the RSA header inside the JSON field; a service-account JSON with a PKCS#8 `-----BEGIN PRIVATE KEY-----` body still trips rule 19, so coverage is incidental, not designed |
| 37 | `cloudflare_api_token` | `(?i)\b(?:CLOUDFLARE_API_(?:TOKEN\|KEY)\|CF_API_TOKEN)\s*=\s*[A-Za-z0-9_-]{40}\b` | 0 | 0.92 | **P2 ozzt.** Cloudflare tokens have no standalone prefix, so the env-var context is mandatory — otherwise any 40-char alnum string would auto-wipe. `patterns.rs:263-274` |
| 38 | `sendgrid_api_key` | `\bSG\.[A-Za-z0-9_-]{22}\.[A-Za-z0-9_-]{43}\b` | 0 | 0.99 | **P2 ozzt** |
| 39 | `terraform_cloud_token` | `\batlasv1\.[A-Za-z0-9_-]{64,}\b` | 0 | 0.99 | **P2 ozzt** |

**v2 amendment — rule 23 tolerates one internal delimiter.** v1 spelled each
compound keyword with an underscore only, so `api-key:`, `access.token:` and
`auth token =` reached no rule at all: there is no bare `key` or `token`
keyword to fall back on, unlike `client secret` and `db-password`, which the
bare `secret` and `password` keywords already match inside. v2 accepts exactly
one `-`, `_`, `.` or space in `api…key`, `auth…token`, `access…token` and
`refresh…token`. The delimiter is not optional — written `api[-_. ]?key` the
literal the prefilter can require collapses from `api_key` to `api`, and every
rule in the set pays for that. The value-strength gate is unchanged, so a wider
keyword cannot make a weak value wipeable.

### 3.3 Rule 41 — credit card (NOT a regex rule)

Credit cards are **not** in the pattern table. They are a separate,
always-on check with **implicit confidence 0.99**:

- Candidate scanner: `\b(?:\d[\s-]?){12,18}\d\b` — a digit run of 13–19 digits
  with optional single space or hyphen between digits
  (`detector/luhn.rs:92`).
- Each candidate is then **Luhn-validated** after stripping non-digits, with a
  hard `13 ≤ len ≤ 19` clamp (`luhn.rs:50`, `:13`).
- Runs on the **already-NFKC-normalised** string, so full-width digit bypass is
  defeated by the same pass (`kind.rs:74-83`).
- Consequences: `SensitiveKind::CreditCard`, and it triggers auto-wipe even
  when **no** regex rule fired (`engine.rs:146-149`) and even when all regex
  rules that did fire were below the floor (`engine.rs:166-167`).

**v2 amendment — Luhn is not enough, and `\s` was too much (DMY-162).** The v1
scanner above accepts a newline or a tab between digits, so a column of amounts
one per line, or an ISBN with a quantity beside it, becomes one candidate; Luhn
then passes about one run in ten. Measured on a 600-sample corpus of ordinary
numeric data, **10.8 % classified as `credit_card` at 0.99** and were hard-
deleted wherever auto-wipe was on. v2 changes both halves:

- A separator is one space or one hyphen between groups — never a newline, a
  tab, or a mixture — and the leading group is four digits. Uniformity is
  checked in the validator rather than by spelling the two spacings as separate
  regex alternatives.
- The digit run must be a card: issuer range and per-brand length, from the
  maintained `card-validate` crate, on top of the `13 ≤ len ≤ 19` clamp, which
  stays because it is load-bearing independently of any brand table.
- **Order the validator cheapest-first.** Every digit run in ordinary text is a
  candidate, and `Validate::from` walks twelve brand regexes. Running it before
  the clamp and Luhn made a 16-digit hex id cost 33 % of a 64-byte scan.

The same corpus now yields **0 %**, and every §9.1 card fixture, plus the Amex,
Diners, Discover, Mastercard and JCB grouped forms, is still detected. The
narrowing is one-directional by choice: a 19-digit Visa is now a false negative,
which is the direction I1 requires.

**Bug history — Audit MED #6:** the original gate was
`normalised.len() <= 25 && luhn_valid(normalised)`, i.e. cards were only
detected when the clipboard contained *nothing but* the card number. A card
embedded in ordinary text (`"Customer card: 4111 1111 1111 1111 — expires
12/26"`) was a **silent miss**. Replaced by the per-candidate scan.
(`kind.rs:74-80`, tests at `detector/mod.rs:110-122`.)

**Known gap:** because the card check is outside the pattern table, it
produces **no `PatternMatch` and therefore no span**. `detect_sensitive_spans`
(`ffi_sensitive.rs:253`) and the daemon's `sensitive_spans`
(`ipc/handlers_items_read.rs:286-311`) can never bullet-mask a card embedded in
otherwise-benign text. The FFI doc comment even lists `"credit_card"` as an
example `pattern_name` (`ffi_sensitive.rs:73`) — that name does not exist.
**v2 must make the card rule a first-class rule that yields spans.**

---

## 4. Confidence / threshold model and action rules

### 4.1 The numbers

| Constant | Value | Where |
|---|---|---|
| `AUTOWIPE_CONFIDENCE_FLOOR` | **0.70** | `detector/engine.rs:141` |
| Default sensitive TTL | **30 s** in v1; **`0` (off)** in v2 — see §1.2 | `config/defaults.rs:51` |
| `sensitive_ttl_secs == 0` | sentinel: **auto-wipe disabled**, never clamped away | `config/mod.rs:248`, `:286` |
| FP-corpus budget | ≤ 5 % of benign corpus, floor of 2 absolute | `tests/false_positive_corpus.rs:91-103` |
| TP-corpus recall | **100 %**, ≥ 20 entries | `tests/true_positive_corpus.rs:368-391` |

### 4.2 What the bands mean

| Band | Members | Semantics |
|---|---|---|
| **0.90 – 0.99** | Prefixed/structural tokens with a unique literal or a mandatory context anchor | "Cannot plausibly be anything else." Safe to auto-delete. |
| **0.75 – 0.80** | `generic_password_kv` (0.75), `dotenv_secret` (0.80) | Keyword-driven. 0.75 is gated by a value-strength validator; **0.80 is not** (§7.1). |
| **≥ 0.70 floor** | — | Auto-wipe boundary. Nothing may sit *exactly* on it: `ip_with_port` did, and it caused data loss (`CopyPaste-8ys1`). Treat 0.70 as exclusive-in-spirit. |
| **0.55 – 0.65 — INERT** | `phone_us`, `passport`, `email`, `iban`, `ssn_us`, `discord_bot_token`, `twilio_signing_key_sid`, `generic_bearer`, `ip_with_port` | Detected, labelled, masked, redacted in logs — **never deleted**. |

The inert band exists for two distinct reasons, both worth preserving in v2:

1. **Genuinely sensitive but user-owned data** (IBAN, SSN, email, phone,
   passport). The user *meant* to copy their own bank details. Flagging is
   helpful; deleting is hostile.
2. **Shapes too weak to prove a secret** (`discord_bot_token`,
   `twilio_signing_key_sid`, `generic_bearer`, `ip_with_port`). The right fix
   is a validator (mod-97, SSN structure, Discord snowflake decode); until one
   exists, the floor is the safe fallback.

### 4.3 Action rules

```
capture(text, source_app):
    sensitive := is_autowipe_sensitive(text) OR is_password_manager(source_app)

is_autowipe_sensitive(text):
    n := NFKC(text)
    hits := rules_matching(n)
    for h in hits:
        if h.confidence < 0.70: continue
        if h.rule == generic_password_kv and not value_is_strong(h.value): continue
        return true
    return contains_luhn_valid_card_run(n)      # both when hits is empty and
                                                # when all hits were sub-floor
```

Then, if `sensitive`:

| Effect | Rule |
|---|---|
| `is_sensitive = 1` on the stored row | always |
| `expires_at` | `now_ms + sensitive_ttl_secs * 1000`, **or `NULL` when `sensitive_ttl_secs == 0`** (`capture/text.rs:227-234`, `ffi_sensitive.rs:159-164`) |
| FTS indexing | **skipped** (§6.1) |
| Thumbnail | forced `NULL` (I5) |
| History preview | replaced with `[sensitive — id:XXXXXXXX]` (`ipc/handlers_items_read.rs:268-270`) |
| Spans computed | **no** — spans are only computed for *non*-sensitive text items (`handlers_items_read.rs:286`), i.e. masking applies to secrets embedded in otherwise-benign content, not to whole-secret items (which are hidden outright) |

### 4.4 Cross-platform parity is part of the contract

macOS daemon, Android FFI, and any future client MUST use the **same** 0.70
gate. v1 shipped three separate divergences and had to fix each:

| Bug | Divergence | Fix |
|---|---|---|
| **AB-6a** | Android `is_sensitive` used `detect().is_some()` (ANY match), so a phone number was *dropped* on Android but *kept* on macOS | call `is_sensitive_for_autowipe` (`ffi_sensitive.rs:79-94`) |
| **PG-23 / l9z8** | `sensitive_kind()` returned `Some("Phone")` while `is_sensitive()` returned `false` for the same text | gate the label at the same 0.70 floor (`ffi_sensitive.rs:98-128`) |
| **PG-3 / 349q** | Kotlin coordinated three separate calls and computed expiry itself | one `sensitive_capture_decision(text, now_ms, ttl) -> {is_sensitive, kind, expires_at_ms}` (`ffi_sensitive.rs:144-176`) |

**v2 rule:** expose exactly **one** capture-time verdict function. Everything
else is derived from it.

---

## 5. False-positive defences

### 5.1 NFKC normalisation — and the bypass it prevents

Applied before every match (I3). The bypass it closes: full-width Latin
letters/digits, compatibility ligatures, and other NFKC-foldable forms that
render as ASCII but do not match ASCII character classes. Pinned by
`nfkc_normalised_input_detects_secrets` (`detector/mod.rs:229-236`) using
`\u{FF21}\u{FF2B}\u{FF29}\u{FF21}` + 16 ASCII chars → must detect as an AWS
key.

Two consequences that must be carried:

- **Idempotence on ASCII** — `nfkc_normalize("AKIAIOSFODNN7EXAMPLE")` is the
  identity (`detector/mod.rs:249-253`). This is what lets callers usually index
  spans into the original string.
- **Offsets belong to the normalised string.** Redaction must be applied
  against the same normalised string (`normalize.rs:6-9`), and the
  byte→char offset mapping for UI masking must be computed over it
  (`ffi_sensitive.rs:283-292`, daemon `handlers_items_read.rs:290-306`).

### 5.2 Structural anchoring (the primary defence)

Most FP control is in the regex itself, not in post-filters:

- **`\b` word-boundary anchors** on every prefixed-token rule, so a token glued
  into a longer identifier does not match. Enforced by a meta-test
  (`patterns.rs:522-543`) over `sendgrid_api_key`, `terraform_cloud_token`,
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
  context-anchored instead (`patterns.rs:524-527`).
- **Minimum-length tightenings** driven by observed FPs: `hashicorp_vault`
  `{32,}`; `passport` 6 → 9 digits; `jwt` `\b`-prefixed.

### 5.3 Value-strength gate (the only post-match validator)

Applies to `generic_password_kv` **only** (`detector/fp.rs:11-32,41-60`).
Capture group 1 (the value) is "strong" if **any** of:

1. `value.chars().count() >= 10` — **characters, not bytes**;
2. contains one of `! @ # $ % ^ & * + / =`;
3. contains at least one ASCII letter **and** at least one ASCII digit.

**Bug + rule — the char-vs-byte gate:** a 9-character CJK value
(`私的秘密言葉確認鍵`) is 27 bytes. A byte-length gate would call it strong; the
char gate correctly calls it weak. Pinned by
`multibyte_value_gated_on_chars_not_bytes` (`detector/mod.rs:284-302`).

**Where it is enforced:** the general detect path (`engine.rs:71`), the
first-match path (`kind.rs:62-70`), and — critically — the auto-wipe path
(`engine.rs:155-163`), which re-validates before returning `true`. The fast
`RegexSet` path in `is_sensitive()` also has a special case: if the *only*
matching rules are `generic_password_kv`, it must fall through to the full
validated path rather than returning `true` (`engine.rs:107-117`).

### 5.4 Luhn validation

The only checksum validator in the system. Two independent things it buys:

- 13–19-digit runs that are not Luhn-valid do not classify as `CreditCard`
  (`detector/mod.rs:124-139`).
- The clamp `13 ≤ digits ≤ 19` rejects short/long numeric runs outright.

**Test-fixture bug worth recording:** the FP fixture `"4242424242422"` was
*accidentally Luhn-valid* (digit sum 50 ≡ 0 mod 10), so the "must not match"
test was passing vacuously. Replaced with `"4242424242421"` (sum 49).
(`detector/mod.rs:129-132`.) In v2, **assert your negative fixtures are
actually negative.**

**v2 amendment:** §7.3's preferred outcome — take Luhn from a crate — is what
v2 does. `card-validate` carries the checksum, the issuer ranges and the
per-brand lengths together, and §3.3 records why the checksum alone was a
data-loss defect.

### 5.5 Entropy / variety gate — asymmetry to resolve

There is **no entropy gate in the core detector.** The only variety heuristic
lives in the *telemetry scrubber*: `has_token_variety` requires ≥1 uppercase,
≥1 lowercase and ≥1 ASCII digit before redacting a ≥32-char base64url
candidate (`copypaste-telemetry/src/scrubber.rs:154-159`, applied at
`:385-402`). It is explicitly documented as a cheap *necessary* condition, not
Shannon entropy (`scrubber.rs:150-153`).

Two opposite biases are in play and both are correct **for their own context**:

| | Core detector | Telemetry scrubber |
|---|---|---|
| Prefers | false **negatives** (never delete user data) | false **positives** (never leak PII) |
| Stated at | `patterns.rs:71-74`, `:164-168`, `:198-205` | `scrubber.rs:199-201`, `:303-308` |

**v2 must keep the two biases but stop maintaining two rulesets** — §7.5.

### 5.6 Allowlists

**There are none.** No stopword list, no path/domain allowlist, no
`# gitleaks:allow`-style inline suppression, no per-user exclusions for
content patterns. The only "allowlist"-shaped artefact is the benign corpus in
`tests/false_positive_corpus.rs`, which is a *regression harness*, not a
runtime mechanism.

This is a real gap and the strongest single argument for re-sourcing from
gitleaks, which ships curated allowlists and stopwords per rule. **v2 SHOULD
adopt gitleaks' allowlist model** (regex + stopword + path allowlists, and an
inline-suppression comment convention).

**v2 amendment — an allowlist suppresses, so it fails closed.** `all` over an
empty set is true, so an `AND` allowlist carrying neither a regex nor a stopword
would silence every match of the rule it is attached to, with nothing to say so.
An allowlist with no checks allows nothing.

**v2 amendment — placeholder stopwords for the context-anchored rules.** A
context anchor (§5.2) proves which *field* matched and says nothing about the
*value*, so `AccountKey=`, `CLOUDFLARE_API_TOKEN=`, `aws_secret_access_key =`
and `dotenv_secret`'s variable-name suffix all matched README examples at
0.80-0.99. Each of those rules now captures its value and gates it against a
placeholder stopword list. `aws_secret_access_key` omits `example` for the same
reason `aws_access_key` gives up gitleaks' `.+EXAMPLE` allowlist: AWS's own
published secret key ends `EXAMPLEKEY` and §9.1 binds it.

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

Verbatim from `crates/copypaste-core/src/sensitive/detector/apps.rs:3-33`:

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

(`apps.rs:37-42`.) Note the direction: **the needle is the list entry and the
haystack is the input** — so `com.agilebits.onepassword4` matches the entry
`1password` by substring, and `COM.1PASSWORD.1PASSWORD` matches
case-insensitively (`detector/mod.rs:208-224`). The bare fragments make the
list deliberately over-broad; that is acceptable because a false positive here
costs a 30-second TTL on one item, not silent loss of unrelated data.

### Call sites

| Path | Behaviour |
|---|---|
| Text capture | `is_sensitive = content_is_sensitive OR app_is_sensitive` (`daemon/capture/text.rs:73-78`) |
| File capture | `item.is_sensitive = app_is_sensitive_file` (`daemon/capture/file.rs:41-43`, `:79-80`) |
| Image capture | best-effort; documented as degraded when the frontmost lookup fails (`capture/tick.rs:56-63`) |

### Why it exists (the earned insight)

> "a freshly-copied password is often a random string with low confidence"
> — `daemon/capture/text.rs:69-72`

A strong, randomly-generated password matches **no** pattern in §3. Content
detection cannot see it. The app signal is the *only* thing that protects
password-manager copies, which makes I6 (always evaluate it) a security
invariant rather than a nicety.

### Porting notes

- This list is macOS-shaped (bundle IDs) and stale-prone. **v2 should treat it
  as configuration, not code**, and let users add entries.
- Missing notables: Proton Pass, Strongbox, Secretive, KeePassium, Apple
  Passwords (`com.apple.Passwords`), Bitwarden's newer bundle IDs.
- The frontmost-app lookup has a 750 ms budget
  (`daemon/capture/frontmost.rs:13`) and is shared with the exclusion check
  (`frontmost.rs:85`) — see I6 for what happens when that sharing is done wrong.

---

## 6. Interaction with storage

### 6.1 FTS exclusion (ADR-015 / `CopyPaste-i6pp`)

**Rule: `is_sensitive = 1` ⇒ never written to `clipboard_fts`, never returned
by `search_items`.**

Rationale (ADR-015 §Context): FTS5 shadow tables hold **plaintext** from the
application's perspective. SQLCipher encrypts pages at rest, but the main
`content` column additionally carries application-layer XChaCha20-Poly1305
ciphertext — the FTS index does not. So FTS is a strictly lower-security store,
and FTS5 offers no per-value transformation hook.

Three independent enforcement layers (all must be ported):

| Layer | Guard | Location |
|---|---|---|
| 1. Write | `if !plaintext_for_fts.is_empty() && !item.is_sensitive` — unconditional, ignores what the caller passed | `storage/items/insert.rs:169-183` |
| 2. Update | `upsert_fts` re-reads `is_sensitive` **inside the same transaction** as the FTS write; `Some(1)` or row-missing ⇒ return `Ok(())` without writing | `storage/items/fts.rs:136-173` |
| 3. Query | `search_items` adds `AND ci.is_sensitive = 0` to the JOIN predicate | `storage/items/query.rs:356-400` |

**Bug + rule — `CopyPaste-44rq.64` (TOCTOU):** the layer-2 sensitivity check
was originally *outside* the transaction, so a concurrent `UPDATE` flipping
`is_sensitive = 1` between the `SELECT` and the `INSERT` would let secret
plaintext reach the index. **Rule: the sensitivity read and the FTS write must
share one write lock.**

**Migration v13** purges pre-fix rows, idempotently and atomically inside
`apply_migrations` (`storage/schema/versions.rs:138-160`,
`storage/schema/mod.rs:82`, `:410`):

```sql
DELETE FROM clipboard_fts
WHERE id IN (SELECT id FROM clipboard_items WHERE is_sensitive = 1);
```

**Accepted consequence:** sensitive items are not searchable by content, only
by `item_id` or via the history list. ADR-015 explicitly rejects a user opt-in
("an opt-in is a footgun; the policy must be unconditional").

### 6.2 Sensitive TTL / expiry

**v2 amendment — the deadline is derived, not stored.** The rules below are
binding; the `expires_at` *column* is not. v2 has no such column: a sensitive
row's deadline is `created_at + ttl`, evaluated by the one predicate in
`copypaste_core::sensitive::sweep_sensitive`. Three of the four bugs in this
section are shapes of "the deadline was stored, and the store drifted from the
rule" — `CopyPaste-3e7y` (two predicates disagreed and a row with no
`expires_at` outlived its TTL), `CopyPaste-44rq.62` (backfill and delete in
separate transactions) and `CopyPaste-8ebg.2` (a dedup bump inherited a stale
deadline). None of them is reachable without the column: there is nothing to
backfill, nothing to keep atomic with the delete, and the bump resets the
deadline for free because it restamps `created_at`. The `0` sentinel, the pinned
exemption, the cheap existence probe and the startup purge are unchanged and
implemented.

**v2 amendment — the wipe decision is re-derived from the plaintext.** v1
stamped wipe eligibility at capture. v2 re-scans the candidate row at wipe time
and requires `Severity::HighConfidence` then, so a row is deleted only if it was
flagged at capture *and* is still above the floor. Deleting on a flag written by
a ruleset that has since changed is a decision nobody can review before it
fires, and `CLAUDE.md` rule 4 ranks that above the cost of one AEAD per expired
candidate.

**Consequence, stated rather than hidden:** changing the TTL re-dates every
existing sensitive item rather than only new ones.

| Property | Value |
|---|---|
| Config field | `sensitive_ttl_secs`, v1 default **30**, v2 default **0** (off — §1.2) |
| `0` | "auto-wipe disabled" — a valid value, **explicitly not clamped** (`config/mod.rs:248`, `:286`) |
| Stamped at capture | `expires_at = now_ms + ttl_secs * 1000` (`capture/text.rs:233`) |
| Pinned items | never expire — excluded by both the backfill and the delete (`storage/items/delete.rs:130-131`, `:151`, `:166`) |
| Canonical predicate | `expires_at IS NOT NULL AND expires_at < now_ms AND pinned = 0` |

**Bug + rule — `CopyPaste-8ebg.1`:** capture read `sensitive_ttl_local_secs`
(default 1800 s), a field with no IPC setter and no UI control. The
user-facing `sensitive_ttl_secs` setting was therefore **dead** — a password
always lived 30 minutes no matter what the user configured
(`capture/text.rs:228-232`). *Rule: one TTL field, wired end-to-end, or delete
the field.* Related dead fields (`sync_ttl_secs`,
`sensitive_ttl_relay_secs`, `sensitive_ttl_local_secs`,
`encryption_chunk_kb`) were removed under `CopyPaste-8ebg.8` and are now
rejected as unknown keys (`config/mod.rs:540-569`).

**Bug + rule — `CopyPaste-8ebg.2`:** re-copying an identical sensitive item
deduped to the existing row and *kept the old `expires_at`*, so the re-copy
inherited a deadline that could be seconds away. Fix: dedup-bump recomputes
`expires_at = now_ms + ttl` for sensitive rows
(`storage/items/query.rs:402-426`: `expires_at = CASE WHEN is_sensitive = 1
THEN ?1 + ?4 ELSE expires_at END`; test at `storage/items/tests.rs:2321-2348`).

**Bug + rule — `CopyPaste-3e7y` (unified TTL path):** two divergent predicates
existed — the sensitive sweep used `wall_time < now_ms - ttl_ms` while
`delete_expired` used `expires_at < now_ms`. A sensitive row without an
explicit `expires_at` was invisible to the latter and could outlive its TTL.
Fix: **backfill then delegate** — one canonical `expires_at` predicate
(`storage/items/delete.rs:112-131`).

**Bug + rule — `CopyPaste-44rq.62` (atomicity):** the backfill `UPDATE` ran in
autocommit while the delete opened its own transaction; a crash between them
left rows stamped-but-not-deleted. Fix: single `unchecked_transaction` covering
backfill + select-ids + `pending_uploads` cleanup + delete + FTS cleanup
(`delete.rs:139-175`).

**Fail-closed probe:** "are there sensitive unpinned rows?" returns `true` on
query error so the sweep always runs (`delete.rs:87-109`).

### 6.3 Other storage effects

- Thumbnails suppressed for sensitive items (I5).
- History preview replaced with `[sensitive — id:XXXXXXXX]`
  (`ipc/handlers_items_read.rs:268-270`).
- `CopyPaste-mnte`: the detector is constructed **once per page**, not per
  item, and the page normalises once and calls `detect_normalised` to avoid a
  second NFKC pass (`handlers_items_read.rs:261-264`, `:286-296`). Keep the
  "normalise once, detect on the normalised form" split in v2.

---

## 7. Known-unjustified complexity — do NOT port as-is

### 7.1 Rules above the floor that should not be

| Rule | Conf | Problem |
|---|---|---|
| `aws_arn` | 0.90 | An ARN is a **resource identifier**, not credential material — the same class of reasoning that got `ip_with_port` demoted under `CopyPaste-8ys1`. Copying an ARN into a ticket today gets it silently deleted in 30 s. **Demote below the floor or drop.** |
| `dotenv_secret` | 0.80 | Matches `FOO_KEY=<anything non-space>` with **no value-strength validator** — `API_KEY=changeme`, `DB_PASSWORD=xxx`, `MY_TOKEN=TODO` all auto-wipe. Apply the §5.3 validator or demote. |
| `generic_password_kv` | 0.75 | Above the floor *only* because of the validator. If v2 re-sources rules from gitleaks, the equivalent (`generic-api-key`) carries an entropy threshold — use it, and keep the value gate. |

### 7.2 `detect()` returns the lowest **index**, not the highest **confidence**

`detect()` iterates `RegexSet::matches` in ascending pattern-index order and
returns the first hit (`kind.rs:60-73`). Declaration order therefore becomes a
semantic contract nobody documented. Consequence: text containing both an
email (index 27) and a Terraform token (index 39) is labelled
`Other("email")`, and that label is what the UI badge and the Android
`sensitive_kind` show. `highest_confidence()` exists (`engine.rs:171-177`) and
is exactly what should have been used — **it is dead code.**

**v2: rank by confidence, then by specificity. Declaration order must not be
semantic.**

### 7.3 Duplicate Luhn implementations

`luhn_valid` (`detector/luhn.rs:4-34`, public API) and `luhn_valid_strict`
(`luhn.rs:39-71`, internal) are **byte-for-byte the same algorithm** —
identical digit filter, identical `13..=19` clamp, identical doubling loop,
identical `sum % 10 == 0`. The stated justification is "inlined here to avoid
an extra allocation+digit-filter pass on the per-candidate hot path"
(`luhn.rs:37-38`), but both allocate a `Vec<u32>` and both run the same filter
— **the claimed optimisation does not exist.**

**v2: one Luhn function.** (Or none — take it from a crate; `luhn`/`card-validate`
exist. The algorithm is trivial, so a single 15-line internal function is
also defensible.)

### 7.4 Three overlapping "is it sensitive?" entry points, two of them dead

| API | Semantics | Callers |
|---|---|---|
| `is_sensitive_for_autowipe` | 0.70 floor + Luhn + KV validation | daemon, Android FFI — **the real one** |
| `SensitiveDetector::is_sensitive` | ANY match at ANY confidence (an email returns `true`) | **none** |
| `SensitiveDetector::is_sensitive_threshold` | caller-supplied threshold | **none** |
| `SensitiveDetector::highest_confidence` | best match | **none** |

`is_sensitive` also carries a bespoke fast/slow `RegexSet` path with a
`generic_password_kv` special case (`engine.rs:89-118`) that no one exercises.
Also note `use is_sensitive_for_autowipe as _;` at
`crates/copypaste-daemon/src/daemon/mod.rs:866` — an import kept alive only to
silence a warning.

**v2: one public verdict function, one span function, one label function.
Delete the rest.**

### 7.5 The telemetry scrubber is a second, divergent regex engine

`copypaste-telemetry/src/scrubber.rs` (464 LOC) re-implements NFKC
normalisation (`scrubber.rs:376`), an email regex that is a near-copy of rule
27 (`scrubber.rs:55` vs `patterns.rs:184` — differing only in the `\b`
anchors), a JWT regex, an IPv4 regex, and a base64url-token rule with its own
variety gate — none of which share code with the core detector. It also
carries a long tail of hard-won ordering constraints (URL-auth before email;
email before long-hex; JWT before single-token base64url; no trailing boundary
assertion on IPv6 or adjacent addresses leak — `scrubber.rs:186-198`,
`:283-308`) that are genuinely earned knowledge but belong to *one* engine.

**v2:** either drive the scrubber from the same rule table with a
`bias = PreferOverRedaction` flag, or drop it entirely — telemetry is unwired
today (`scrubber.rs:307-308`) and `sentry`'s `before_send` can host the
redaction. Keep the **ordering constraints** and the **variety gate** as
documented requirements either way.

### 7.6 Degrade-to-empty regex initialisation

`patterns.rs:293-330` builds an elaborate "if any regex fails to compile,
degrade the whole set to empty and log" scheme, plus an `init_patterns()`
called from `Database::open()` (`storage/db/mod.rs:65`), plus a test asserting
the fallback can never fire (`patterns.rs:358-368`). All of this exists to
satisfy a no-`unwrap()` rule against a `&'static`-returning signature over
**compile-time constants**.

**v2:** validate the ruleset once at build time (a `const`/`build.rs`/`LazyLock`
that is unit-tested), and let the type system carry name+confidence *with* the
compiled rule so I8 is structural. Then this entire mechanism disappears.

### 7.7 Fixed 5 % FP budget with an absolute floor of 2

`allowed = max(len * 5 / 100, 2)` (`tests/false_positive_corpus.rs:91-92`)
means the 50-entry benign corpus tolerates **2 false positives without naming
them**. A budget hides which entries regressed. **v2: assert zero, and
explicitly `#[ignore]`-list any entry that is a known, accepted FP with a
comment saying why.**

### 7.8 Miscellaneous

- `pypi_token` requires `{180,}` chars after `pypi-`; real PyPI tokens are
  ~172–200 chars but the prefix payload `pypi-AgEIcHlwaS5vcmc…` is the
  distinctive part. Length-only gating is fragile; use gitleaks' form.
- `SensitiveKind` is a closed enum with an `Other(String)` escape hatch and a
  `from_pattern_name` string-prefix dispatch (`kind.rs:25-45`). Prefix matching
  on names (`n.starts_with("aws")`) means renaming a rule silently changes its
  label. **v2: the label belongs in the rule definition.**
- `nfkc_zwj_in_jwt_normalises_away` (`detector/mod.rs:239-247`) asserts nothing
  about ZWJ — its body tests a clean ASCII JWT and the doc comment admits NFKC
  keeps ZWJ. **The ZWJ bypass is untested and probably still open.** Write a
  real test in v2 (and decide whether to strip default-ignorable code points
  before matching).

---

## 8. gitleaks mapping

Per the audit: re-source the ruleset from gitleaks rather than maintaining 40
hand-authored regexes. Mapping below is against gitleaks' default
`config/gitleaks.toml` rule IDs.

> ⚠ **Verify every row against the pinned gitleaks version at port time.**
> Rule IDs and regexes change between releases; this table is a starting point,
> not a contract.

| # | v1 pattern | gitleaks rule id | Notes |
|---|---|---|---|
| 0 | `aws_access_key` | `aws-access-token` | gitleaks is broader (`A3T[A-Z0-9]`, `ABIA`, `ACCA` too) — a **recall win** |
| 1 | `github_fine_grained` | `github-fine-grained-pat` | |
| 2 | `github_classic_pat` | `github-pat` | |
| 3 | `github_actions_token` | `github-app-token` | gitleaks covers `ghu_`/`ghs_`; also `github-oauth` (`gho_`) and `github-refresh-token` (`ghr_`) — **v1 gaps** |
| 4,5 | `openai_new`, `openai_legacy` | `openai-api-key` | gitleaks anchors on the `T3BlbkFJ` infix — far more precise than v1's length-only `sk-` rule |
| 6 | `anthropic` | `anthropic-api-key` (+ `anthropic-admin-api-key`) | verify presence in the pinned version |
| 7 | `stripe_live` | `stripe-access-token` | gitleaks covers `sk_`/`rk_` × `test`/`live`/`prod` — v1 only `sk_live_` |
| 8 | `stripe_webhook` | — | **no equivalent**; keep as a custom rule |
| 9 | `npm_token` | `npm-access-token` | |
| 10 | `pypi_token` | `pypi-upload-token` | gitleaks anchors the `AgEIcHlwaS5vcmc` payload — better than length-only |
| 11 | `slack_bot` | `slack-bot-token` | gitleaks also ships `slack-app-token`, `slack-user-token`, `slack-config-*` — **v1 gaps** |
| 12 | `slack_webhook` | `slack-webhook-url` | |
| 13 | `discord_bot_token` | `discord-api-token` (partial) | gitleaks' rule is keyword-anchored on "discord"; **strictly better than v1's shape-only rule.** Adopting it may justify raising this above the floor |
| 14 | `twilio_signing_key_sid` | `twilio-api-key` | same `SK`+32-hex shape; gitleaks keyword-anchors. Keep sub-floor unless the context anchor is adopted |
| 15 | `google_api_key` | `gcp-api-key` | |
| 16 | `heroku_api_key` | `heroku-api-key` | |
| 17 | `hashicorp_vault` | `vault-service-token` (+ `vault-batch-token` for `hvb.`) | gitleaks requires 90–100 chars vs v1's `{32,}`; `hvb.` is a **v1 gap** |
| 18 | `gcp_oauth` | `gcp-oauth-client-secret` (verify) | `GOCSPX-` |
| 19,20 | `ssh_private_key`, `…_pkcs8_encrypted` | `private-key` | gitleaks' single `-----BEGIN[ A-Z0-9_-]{0,100}PRIVATE KEY( BLOCK)?-----` subsumes **both** v1 rules and more (PGP, `DSA`, `SSH2`) |
| 21 | `ssh_private_key_putty` | — | **no equivalent**; keep as a custom rule (real bug fix, Audit MED #5) |
| 22 | `generic_bearer` | `generic-api-key` (partial) | gitleaks adds an entropy threshold; consider replacing outright |
| 23 | `generic_password_kv` | `generic-api-key` | gitleaks: keyword list + entropy ≥ 3.5 + stopword allowlist. **Strictly better than v1's variety heuristic** — adopt, but keep v1's char-not-byte lesson |
| 24 | `jwt` | `jwt` | gitleaks requires `ey…\.ey…` (both header *and* payload base64-`ey`-prefixed) — tighter than v1 |
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
   allowlists are a good substitute for some of them, but where v1 requires
   `AccountKey=` / `CLOUDFLARE_API_TOKEN=` / `user:password@`, that requirement
   is the reason the rule is allowed to auto-delete.
3. **Adopt gitleaks' allowlists/stopwords** (§5.6) — v1's biggest structural gap.
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

## 9. Acceptance tests to re-create

Ported from `sensitive/patterns.rs` tests, `sensitive/detector/mod.rs` tests,
`sensitive/redact.rs` tests, `crates/copypaste-core/tests/true_positive_corpus.rs`
and `crates/copypaste-core/tests/false_positive_corpus.rs`.

### 9.1 True positives — MUST be detected

| Input | Expected |
|---|---|
| `AKIAIOSFODNN7EXAMPLE` | detected; `AwsKey`; **auto-wipes** |
| `AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE` | detected |
| `ASIAIOSFODNN7EXAMPLE1234` | detected (trailing digits must not break it) |
| `ghp_` + 36×`A` | detected |
| `github_pat_` + 22×`A` + `_` + 59×`B` | detected |
| `ghs_16C7e42F292c6912E7710c838347Ae178B4a` | detected |
| `sk-proj-` + 48×`A` | detected; **`openai_legacy` must NOT also fire** |
| `sk-` + 48×`A` | detected; auto-wipes |
| `sk-ant-api03-` + 80×`A` | detected |
| `sk_live_` + 24×`A` | detected |
| `whsec_aAbBcCdDeEfFgGhHiIjJkKlLmMnNoOpPqQrRsStT` | detected |
| `npm_` + 36×`A` | detected |
| `xoxb-17653285717-17653285718-AbCdEfGhIjKlMnOpQrStUvWx` | detected |
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
| `password=hunter2` | detected (letter+digit) |
| `secret = !abcdef` | detected (special char) |
| `password: abcdefghij` | detected (10 chars) |
| `SG.` + 22×`A` + `.` + 43×`B` | detected; auto-wipes |
| `atlasv1.` + 64×`A` | detected; auto-wipes |
| `{"private_key": "-----BEGIN RSA PRIVATE KEY-----\nMIIEo..."}` | detected; auto-wipes |
| `AccountKey=` + 86×`A` + `==` | detected; auto-wipes |
| `4111111111111111` | `CreditCard`; auto-wipes |
| `Customer card: 4111 1111 1111 1111 — expires 12/26` | `CreditCard` (Audit MED #6) |
| `please charge 4111-1111-1111-1111 today` | `CreditCard` |
| `378282246310005` / `3782 822463 10005` (Amex 4-6-5) | `CreditCard`; the bare and grouped spellings must agree |
| `30569309025904` / `3056 930902 5904` (Diners 4-6-4) | `CreditCard` |
| `5555555555554444`, `6011111111111117`, `3530111333300000` | `CreditCard`, bare and in 4-4-4-4 groups |
| `api-key: abc123XYZlong`, `api key: abc123XYZlong`, `access-token: rt_abc123XYZlong_value`, `auth token = abc123XYZlongvalue99` | detected — the delimiter spellings of rule 23 |
| `client secret = Sup3rS3cr3tV@lue!`, `db-password: S3cur3Pass!word` | detected via the bare `secret` / `password` keyword, with no branch of their own |
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
| `1234567890123452` | Luhn-valid, no issuer range — an order id, not a card |
| `9780132350883` | Luhn-valid ISBN-13 prefix — not an issuer range |
| `ISBN 978-012-13-234567 qty 12` | ISBN plus quantity must produce no card candidate |
| `AccountKey=your` + 82×`A` + `==` | placeholder value, context anchor present (§5.6) |
| `CLOUDFLARE_API_TOKEN=your` + 36×`b` | placeholder value, and `dotenv_secret` must not classify it either |
| `aws_secret_access_key = your` + 36×`c` | placeholder value |
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

> v2 target: **zero** FPs on this corpus (v1 allows 2 — §7.7).

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
| Category/confidence pin | each P2-ozzt rule is category 0 with confidence ≥ 0.90 |
| Idempotence | `nfkc_normalize` is the identity on ASCII |
| Perf | `detect()` over 10 MB of text completes well under the budget (v1: < 500 ms, release only) — pins that no rule is catastrophically backtracking |

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
| migration v13 on a DB with stale sensitive FTS rows | rows purged; idempotent; no-op on a clean DB; **fails loudly** if `clipboard_fts` is absent |
| insert sensitive item carrying a thumbnail | `thumb` stored as `NULL`; cannot be backfilled |
| sensitive item, `ttl = 30` | `expires_at == now_ms + 30_000` |
| sensitive item, `ttl = 0` | `expires_at` is `NULL`; never swept |
| re-copy of an identical sensitive item | `expires_at` **recomputed** from the new `now_ms` (`CopyPaste-8ebg.2`) |
| sensitive item that is **pinned** | never expired, never backfilled |
| sensitive row with `expires_at IS NULL` | backfilled from `wall_time + ttl`, then swept, **in one transaction** (`CopyPaste-3e7y`, `CopyPaste-44rq.62`) |
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

---

## 10. Source index

| File | Lines of interest |
|---|---|
| `crates/copypaste-core/src/sensitive/mod.rs` | 1-11 — public surface |
| `crates/copypaste-core/src/sensitive/patterns.rs` | 16-31 init; **35-291 the ruleset**; 293-348 accessors; 350-543 tests |
| `crates/copypaste-core/src/sensitive/detector/mod.rs` | 1-12 re-exports; 14-700 the full behavioural test suite |
| `crates/copypaste-core/src/sensitive/detector/engine.rs` | 11-37 types; 54-83 detect; 89-118 `is_sensitive`; **139-168 auto-wipe gate**; 171-177 dead `highest_confidence` |
| `crates/copypaste-core/src/sensitive/detector/kind.rs` | 6-45 `SensitiveKind`; 58-85 `detect()` (index-order bug §7.2) |
| `crates/copypaste-core/src/sensitive/detector/fp.rs` | 11-32 value strength; 41-60 FP filter |
| `crates/copypaste-core/src/sensitive/detector/luhn.rs` | 4-34 and 39-71 (**duplicates**, §7.3); 80-104 candidate scan |
| `crates/copypaste-core/src/sensitive/detector/apps.rs` | 3-33 the list; 37-42 matcher |
| `crates/copypaste-core/src/sensitive/detector/normalize.rs` | 10-12 |
| `crates/copypaste-core/src/sensitive/redact.rs` | 14-84 impl; 88-222 tests |
| `crates/copypaste-core/tests/true_positive_corpus.rs` | whole file |
| `crates/copypaste-core/tests/false_positive_corpus.rs` | whole file |
| `crates/copypaste-telemetry/src/scrubber.rs` | 48-116 patterns; 154-159 variety gate; 178-343 ordering constraints; 375-406 scrub |
| `crates/copypaste-core/src/storage/items/insert.rs` | 22-33, 110-120 thumb suppression; 169-183 FTS write guard |
| `crates/copypaste-core/src/storage/items/fts.rs` | 131-173 `upsert_fts` guard |
| `crates/copypaste-core/src/storage/items/query.rs` | 356-400 search filter; 402-426 `expires_at` recompute |
| `crates/copypaste-core/src/storage/items/delete.rs` | 87-109 fail-closed probe; 112-175 unified TTL sweep |
| `crates/copypaste-core/src/storage/schema/versions.rs` | 138-160 migration v13 |
| `crates/copypaste-core/src/config/defaults.rs` | 49-51 TTL constants |
| `crates/copypaste-core/src/config/mod.rs` | 48-60, 248, 286, 540-569 TTL config + removed dead fields |
| `crates/copypaste-daemon/src/daemon/capture/text.rs` | 48-78 the verdict; 227-234 expiry stamp |
| `crates/copypaste-daemon/src/daemon/capture/tick.rs` | 44-94 app resolution; 419-489 `CopyPaste-44rq.43` regression test |
| `crates/copypaste-daemon/src/daemon/capture/file.rs` | 41-43, 79-80 |
| `crates/copypaste-daemon/src/clipboard/monitor.rs` | 71-73, 174-197 nspasteboard markers |
| `crates/copypaste-daemon/src/ipc/handlers_items_read.rs` | 261-311 preview + spans |
| `crates/copypaste-android/src/ffi_sensitive.rs` | whole file — the parity contract |
| `docs/adr/ADR-015-fts-sensitive-exclusion.md` | whole file |

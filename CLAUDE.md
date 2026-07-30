# CopyPaste — working rules

## 1. Dependencies are the default

**Reach for a maintained crate or package first. Writing the code yourself is
the exceptional path and needs a written reason.**

This rule exists because of a measured failure. The previous codebase carried a
"prefer hand-rolling" norm — cited in source comments as a rule from a
`CLAUDE.md` that had already been deleted from the repository. The norm outlived
its own documentation, and by v0.4.1 an audit of all eight subsystems found the
same wheel carved over and over:

| Concept | Independent implementations |
|---|---|
| retry / backoff | **6** — while an unused `BackoffScheduler` sat in the tree |
| rate limiting | **3** — while `governor` was already a dependency |
| the IPC wire contract | **3** — typed DTOs that the CLI never imported |
| Lamport ordering | **4** |
| ASN.1 / DER parsing, by hand | **2** |
| regex secret/PII engines | **2** |
| hex encoding | ~6 sites — while `hex` was a dependency |

None of these were decisions. Each was a local judgement that forty lines was
cheaper than a dependency. Forty lines times thirty places is how a clipboard
manager reached ~150k lines of Rust.

**"It's only a few lines" is not a justification.** If you catch yourself
writing that sentence, you are in the failure mode this rule was written for.

### The only three exemptions

Each requires an ADR entry naming which one applies:

1. **No maintained package provides the behaviour.** Verified, with links to
   what you evaluated and why it does not fit.
2. **The package cannot see what it would need to see.** The real example: our
   clipboard payloads are opaque ciphertext, so a structural CRDT
   (`automerge`, `yrs`) cannot operate on them at all. Last-write-wins over
   metadata is not a shortcut here, it is the only thing that works.
3. **It would pull a second crypto or TLS stack into the tree.**

Cost of the dependency is a *tradeoff to state*, not an exemption. Binary size,
build time and audit surface are real; write them down and decide, rather than
defaulting to "then I'll write it myself".

### Before you write a helper

Search the tree first. Several of the duplications above existed while the
correct implementation was already present and merely un-imported.

## 2. The port manifests are the specification

`docs/rewrite/port-manifest/` holds ~9,100 lines harvested from the previous
implementation and its tests: roughly 500 acceptance tests and over 200
recovered bug IDs, each recording a defect that was found and fixed in
production.

**Treat them as requirements, not history.** A subsystem is not done until the
acceptance tests in its manifest pass. If you think a manifest rule is wrong,
say so explicitly and change the manifest in the same commit — do not silently
build something that contradicts it.

Every manifest has a section listing complexity that looks gratuitous but is
load-bearing. Read it before "cleaning up" anything.

**Not every manifest rule is binding — see rule 3.** Since v2 drops backward
compatibility, the parts that exist purely to preserve v1 *formats* are now
reference material: byte layouts, the migration ladder, `key_version` dispatch,
and warts kept for bug-compatibility. What stays binding is everything about
*behaviour* — the platform quirks, the security properties, the accessibility
contract, the secret-detection ruleset, and the several hundred acceptance tests
that encode bugs someone already paid for. `docs/rewrite/port-manifest/README.md`
says which is which, per manifest.

## 3. There is no backward compatibility with v0.4.x

**Decided deliberately.** v2 does not read databases, ciphertext, or pairings
written by any earlier version. Existing installs lose their clipboard history
and their paired devices on upgrade.

This is the single largest simplification available to the rewrite, and it
removes a great deal of the complexity catalogued in the manifests:

- One schema. No migration ladder, no `user_version` dispatch, no idempotency
  guards for partially-applied upgrades.
- One key derivation and one AEAD path. No `key_version` dispatch, no rotation
  sweep, no repair pass for rows that were stamped v2 but encrypted with v1.
- Chunking can use `aead::stream` (STREAM) directly, with no legacy decoder and
  no bespoke framing to preserve.
- Warts that only existed to stay bug-compatible can simply be fixed. The dedup
  bucket is the clearest: v1 buckets on `(wall_time / 60)` where `wall_time` is
  in milliseconds, so the "minute" is not a minute. v2 uses a real interval.

**The one obligation this creates:** v2 must not open — or appear to open — a v1
database. Use a distinct filename so an old file is never touched. A user who
downgrades or reinstalls should find their old data intact on disk, and a v2
build that stumbles onto it must say so plainly rather than failing with a
decryption error that reads like corruption.

Do not add a migration path later without deciding it as a feature. Retrofitting
one is materially harder than writing it now, because the v1 formats will no
longer be in the tree.

## 4. Correctness rules carried from v1

- **Data loss is the worst outcome.** Sensitive-content detection may flag, but
  auto-deletion needs high confidence — a false positive destroys user data
  that is not recoverable.
- **Sensitive items must never reach the search index.** Enforced at write time,
  at read time, and by a purge migration for databases predating the rule.
- **Errors shown to users must never contain paths.** The daemon socket path
  discloses the local username.
- **Fail closed on crypto.** A wrong key, a wrong AAD or a wrong key version
  must produce an authentication failure, never a fallback read.

## 5. File-size budget

**Target ≤500 source lines per file. Above that, split it.**

Test modules do not count toward the budget — a file may be 500 lines of code
and 900 of tests and still be fine.

The only exemption is a file that is one genuinely indivisible responsibility,
most often a data table. Claiming it requires a line in the module header naming
what you considered extracting and why doing so would make the code worse.
"It's cohesive" is not an argument on its own: every god file feels cohesive to
whoever wrote it.

v1 reached ~25 production files over 1000 lines, with the worst mixing many
responsibilities — `daemon/p2p/mod.rs` at 2415, `ipc.rs` at ~12,500 before it
was broken up. The cost is not aesthetic. Edits carry a blast radius across
unrelated concerns, tests get coarse, and cohesive logic stays buried instead of
being reused — which is one of the ways the duplication in rule 1 accumulated.

Splitting is behaviour-preserving refactoring. Move a responsibility, keep the
public surface identical by re-exporting from a thin `mod.rs`, move each test
with the code it exercises, and compile and test after every extraction.

## 6. Scope

macOS and Android. Not Windows, not Linux desktop.

Every dependency added must work on both, or be behind a platform cfg with the
other side implemented.

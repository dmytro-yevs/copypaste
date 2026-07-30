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

## 6. A feature is not done until it has a UI

**Ship the interface with the capability, in the same stretch of work — not as a
later phase.**

Peer sync is the example this rule was written from. The transport, the merge,
discovery, the daemon wiring and the CLI commands all landed and passed their
tests, and the feature was called done while the only way to pair two devices
was to type a command into a terminal. For a clipboard manager on macOS and
Android, that is not a shipped feature; it is a feature with no users.

Deferring the interface is expensive in a specific way: the shape of the UI is
what exposes the awkward parts of an API, and finding them a month later means
changing a contract that other code now depends on. Pairing is again the
example — a screen has to show a code, a QR, a progress state, a failure, and a
list of known devices, and none of those needs was visible while the work was
only a CLI verb.

The CLI does not satisfy this rule. It is a tool for scripting and for tests,
not the product surface.

## 7. Scope

macOS and Android. Not Windows, not Linux desktop.

Every dependency added must work on both, or be behind a platform cfg with the
other side implemented.

## 8. Do not document the obvious

**A comment earns its place by recording something the code cannot say.**
Default to none.

Worth writing:

- A defect that actually happened. Name it: `CopyPaste-bfiu`, INV-C2, I-33.
  The test to apply is whether deleting the comment would let a competent
  person reintroduce a bug someone already paid for.
- Why a non-obvious choice is what it is — an ordering, a constant, a bound,
  a refusal. `deleted` being the third merge key is the model: subtle,
  correct, and someone will "simplify" it away without the argument.
- A security property and what upholds it.
- That something is unverified, and why.

Not worth writing:

- What the code plainly says. `/// One peer record` above `struct Peer`.
- The same point at two levels — in `lib.rs`, again in the module header,
  again on the function.
- Layout tables in a `mod.rs` listing the submodules. The directory says it,
  and the table goes stale the moment a file moves.
- Narration of how the code got here: "extracted from", "kept here because",
  "rewritten after". That is git's job.
- A long build-up to a short rule. State the rule.

This applies everywhere, not just to Rust: components, documents, commit
messages, ADRs. An ADR records a decision and its consequences, not an essay.

The volume itself is the risk. Long prose rots faster than code because
nothing compiles it — `copypaste-cloud/src/sync/mod.rs` carried an
eighty-line header whose central claim, the merge ordering, had gone stale in
two places while the code stayed correct. A comment that is wrong is worse
than one that is missing.

## 9. What may reach `main`

**Every commit on `main` compiles, passes its tests, and contains what its
message says.** No exceptions for work in progress.

This rule exists because of a measured failure in this repository, not a
hypothetical. During one day of parallel agent work `main` received: commits
labelled "in flight"; a tree that did not compile; `e2e/node_modules`, staged
by a `git add -A` that ran while an agent was installing; and three commits
that swept in other agents' half-finished changes under a message describing
only one agent's work. One of those had to be corrected by a later commit.

Every one of those came from the same habit — staging everything while
somebody else was writing.

### The rules

1. **Stage explicit paths.** Not `git add -A`, ever, while another agent holds
   any part of the tree. The index is shared; a wide stage is a wide commit.
2. **Verify before committing, not after.** Build, tests, and whatever
   end-to-end checks exist. A red tree goes to a scratch branch, never here.
3. **The message must match the diff.** If a commit turns out to contain more
   than it claims, say so in a follow-up rather than leaving the record wrong.
   Rewriting shared history to fix prose is worse than the inaccuracy.
4. **Unverified work is labelled in the message, not in the branch name.**
   "Never executed on its target platform" is a fact worth recording; "in
   flight" means it should not be here at all.

### Snapshotting without disturbing anyone

Losing an agent's work to a reclaimed container is a real risk, and the fix is
not to commit it early. Build the snapshot as an object and move a scratch
branch to it — this touches neither the working tree nor the index:

```sh
git add -A
TREE=$(git write-tree)
COMMIT=$(git commit-tree "$TREE" -p HEAD -m "WIP snapshot")
git branch -f wip/snapshot "$COMMIT"
git reset -q            # unstage; working tree untouched
```

`git commit` followed by `git reset --soft` looks equivalent and is not: it
leaves the index full, so the *next* `git add <one file>` commits everything.
That is how two of the four failures above happened.

## 10. Commit messages

**Subject line, imperative, 72 characters, no full stop, one change.** A body
only when the subject is not enough, and never more than 12 lines.

```
Fix inaccessible selected-row state

Problem:
Selected and hovered rows were visually indistinguishable.

Change:
- Add an accent edge to selected rows
- Update contrast checks

Risk:
One sentence, and only when there is a real one.
```

`Problem:` and `Risk:` are optional. `Change:` takes one to four points.

### Not in a commit message

Changelogs · file-by-file summaries · another agent's work · how the change
was arrived at · test results, which CI already reports · ADR content · audit
findings · alternatives that were not chosen · anything already written in a
manifest, an ADR or a document · unfinished work, TODOs and known problems,
which belong in an issue · AI `Co-Authored-By` trailers · session URLs.

**Needing more than 12 lines is the signal, not the problem.** Split the
commit, write the decision as an ADR, put the audit in a document, or put the
context in the pull request.

### One commit, one logical change

Do not batch several agents' work, or several subsystems, into one commit.
This is where the length came from: the three longest messages in this
repository each described three or four subsystems, because a wide `git add`
had made a wide commit inevitable. Rule 9's ban on `git add -A` and this rule
are the same rule seen from two ends.

### Enforced, not merely written

`scripts/check-commit-msg.sh` runs from `.githooks/commit-msg` and from CI.
Enable the hook once per clone:

```sh
git config core.hooksPath .githooks
git config commit.template .gitmessage
```

Rule 8 already said "this applies to commit messages" and gave no number, so
it bound nothing: twelve consecutive commits averaged 25 body lines against
what is now a 12-line budget. A rule with no check is a preference.

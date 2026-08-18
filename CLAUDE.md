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

**Not every manifest rule is binding — see rule 3.** Since v2 uses only its own
formats, the parts that exist purely to preserve prior-product *formats* are now
reference material: byte layouts, the migration ladder, `key_version` dispatch,
and warts kept for bug-compatibility. What stays binding is everything about
*behaviour* — the platform quirks, the security properties, the accessibility
contract, the secret-detection ruleset, and the several hundred acceptance tests
that encode bugs someone already paid for. `docs/rewrite/port-manifest/README.md`
says which is which, per manifest.

## 3. v2 uses only its own database

**v2 opens only its own DB filename.** Never open, migrate, or probe any prior
product's files. Do not add `LegacyDatabase`, encounter detection, or special
messaging about old versions — old installs are irrelevant.

One schema, one key path, one AEAD path. Manifest format history is reference
only (see rule 2). Do not add a migration path later without deciding it as a
feature.

## 4. Correctness rules

- **Data loss is the worst outcome.** Sensitive-content detection may flag, but
  auto-deletion needs high confidence — a false positive destroys user data
  that is not recoverable.
- **Sensitive items must never reach the search index.** Enforced at write time,
  at read time, and by a purge migration for databases predating the rule.
- **Errors shown to users must never contain paths.** The daemon socket path
  discloses the local username.
- **Fail closed on crypto.** A wrong key, a wrong AAD or a wrong key version
  must produce an authentication failure, never a fallback read.

## 5. Module boundaries, not a line-count game

**A production module owns one responsibility and exposes the smallest API that
the next layer needs.** A file is not a module boundary if its siblings are
private implementation fragments behind one `mod.rs`, one service object or one
catch-all public API. Tests do not count towards size, but they must live beside
the responsibility they exercise.

Before adding a second concern to an existing module, name the two concerns,
their callers and the dependency direction. If they can change independently,
they are separate modules. Split by ownership and stable concepts — for example
wire decoding, persistence, retry policy and orchestration — never merely by
method order or file length. A façade may compose those modules, but it may not
re-export their combined internals as a new god API.

The review triggers are deliberately earlier than failure:

- At 300 production lines, state the module's single responsibility and inspect
  its public surface before adding more.
- At 500 production lines, extraction is required before the change lands.
- A module with unrelated callers, bidirectional dependencies, or a public type
  that coordinates storage, transport and UI is a god module at any length and
  must be split.

The only exemption to 500 is a genuinely indivisible data table or generated
binding. It needs a module-header line naming the boundary considered and why
it cannot be extracted. "It's cohesive" and "the files are small now" are not
arguments: every god module feels cohesive to its author.

v1 reached ~25 production files over 1000 lines — `daemon/p2p/mod.rs` at 2415,
`ipc.rs` at ~12,500. Edits carried a blast radius across unrelated concerns and
cohesive logic stayed buried instead of being reused, which is one of the ways
the duplication in rule 1 accumulated. `scripts/check-file-size.sh` enforces
the emergency ceiling; review enforces the actual boundary.

Splitting is behaviour-preserving: define the seam, keep only the intentional
public surface, move each test with the code it exercises, then compile and test
after every extraction.

## 6. A feature is not done until it has a UI

**Ship the interface with the capability, in the same stretch of work — not as a
later phase.**

Peer sync is the example this rule was written from. The transport, the merge,
discovery, the daemon wiring and the CLI commands all landed and passed their
tests, and the feature was called done while the only way to pair two devices
was to type a command into a terminal. For a clipboard manager on macOS and
Android, that is not a shipped feature; it is a feature with no users.

The UI is also what exposes the awkward parts of an API, and finding them a
month later means changing a contract other code depends on. A pairing screen
has to show a code, a QR, a progress state, a failure and a list of known
devices; none of those needs was visible while the work was one CLI verb.

The CLI does not satisfy this rule. It is for scripting and tests, not the
product surface.

## 7. Scope

macOS, Android and Windows ([ADR-0013](docs/adr/0013-windows-as-a-third-platform.md)).
Not Linux desktop, which is a test surface (`browser-webkitgtk.yml`) and never a
shipped target.

Every dependency added must work on all three, or be behind a platform cfg with
every other side implemented.

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

Long prose rots faster than code because nothing compiles it.
`copypaste-cloud/src/sync/mod.rs` carried an eighty-line header whose central
claim, the merge ordering, had gone stale in two places while the code stayed
correct. A comment that is wrong is worse than one that is missing.

### The budgets

**No comment block over 12 lines. No module header over 20. No file more
than 30% comment.** Tests excluded, as with rule 5. A file that is all comment
and no code fails at any size — that is the `mod.rs` layout table banned above.

`scripts/check-comments.sh` enforces this from `.githooks/pre-commit` and CI,
against a baseline in `scripts/comment-budget.txt` that can only shrink. The
check explains itself; it is not restated here.

The budgets are not a licence to strip. Cutting a comment that records a
defect someone paid for is the more expensive mistake, and it is the one to
expect once a number exists. When a comment is mostly narration wrapped
around a real reason, **rewrite it to the reason** — deleting both is how a
budget turns into a regression.

## 9. What may reach `main`

**Every commit on `main` compiles, passes its tests, and contains what its
message says.** No exceptions for work in progress.

Measured, not hypothetical. In one day of parallel agent work `main` received
commits labelled "in flight", a tree that did not compile, `e2e/node_modules`
staged by a `git add -A` that ran while an agent was installing, and three
commits that swept in other agents' half-finished changes under a message
describing one agent's work. All of them came from the same habit: staging
everything while somebody else was writing.

### The rules

1. **Name the paths on `git commit`, not on `git add`.** The index is shared,
   so staging explicit paths is not enough — somebody else's `git add` or
   `git mv` is already in the index when you get there, and `git commit` takes
   the whole index. `git commit -F msg -- <paths>` commits exactly those paths
   and ignores everything else staged. `git add -A` remains banned outright.

   This is not hypothetical: `cdd9d1ff` said "fix the macOS clipboard backend"
   and also carried a `git mv` another agent had staged, which broke the build
   on `main`.

   **A pathspec does not protect a file two agents are editing.** It commits
   that file's whole working-tree content, including the lines somebody else
   just added. `9d610fc7` named four paths and still carried a check another
   agent had written for a script that was not yet tracked, so `main` went red
   under a message that did not mention it. Read `git diff HEAD -- <paths>`
   before every commit and confirm every hunk is yours.
2. **Verify before committing, not after.** Build, tests, and whatever
   end-to-end checks exist. A red tree goes to a scratch branch, never here.
   `.githooks/pre-commit` enforces the one part of this that is mechanical:
   staged Rust files must be rustfmt-clean. It reports only staged paths,
   because formatting one file also formats its submodules and those may
   belong to somebody else.
3. **The message must match the diff.** If a commit turns out to contain more
   than it claims, say so in a follow-up rather than leaving the record wrong.
   Rewriting shared history to fix prose is worse than the inaccuracy.
4. **Unverified work is labelled in the message, not in the branch name.**
   "Never executed on its target platform" is a fact worth recording; "in
   flight" means it should not be here at all.

### Snapshotting without disturbing anyone

Losing work to a reclaimed container is a real risk, and the fix is not to
commit it early. Build the snapshot as an object and move a scratch branch to
it, touching neither the working tree nor the index:

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

Do not batch several agents' work, or several subsystems, into one commit. This
is where the length came from: the three longest messages here each described
three or four subsystems, because a wide `git add` had made a wide commit
inevitable. Rule 9 and this rule are the same rule from two ends.

### Enforced, not merely written

`scripts/check-commit-msg.sh` runs from `.githooks/commit-msg` and from CI.
Enable the hook once per clone:

```sh
git config core.hooksPath .githooks
git config commit.template .gitmessage
```

## 11. Worktrees

**Choose one task slug and reuse it everywhere the launcher accepts a name.**
The slug is lowercase kebab-case (`[a-z0-9-]+`) and describes the task. Do not
replace it with a second alias, a generated number or a date suffix.

- For Orca-managed work, use `orca worktree create --name <task-slug>` or the
  orchestration `worker-start` equivalent and accept the branch and directory
  returned by Orca. Do not rename them to another convention.
- For a manually-created Codex worktree, use branch `codex/<task-slug>` and
  directory `~/.codex/worktrees/<task-slug>/CopyPaste`.
- The primary checkout stays on `main`; concurrent change work happens in
  separate worktrees so each task has its own working tree and index.
- Do not create persistent task worktrees in `/tmp`, inside the repository, or
  under another naming scheme.

Create a new task worktree from the intended base ref (`main` by default):

```sh
task_slug=fix-sync-cadence
git worktree add -b "codex/$task_slug" \
  "$HOME/.codex/worktrees/$task_slug/CopyPaste" main
```

The launcher owns its own branch prefix and workspace root. The task slug, the
Linear link and the returned worktree identity are the stable task identity.

## Shared agent operating flow

`AGENTS.md` is the complete source of truth for repository and agent rules.
Read and follow the whole file before planning, dispatching or changing work;
this file is only the Claude bootstrap copy plus that pointer.

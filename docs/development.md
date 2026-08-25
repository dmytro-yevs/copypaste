# Product development

Run `python3 scripts/check-feature-ledger.py` before opening a pull request. The
machine-readable record is `docs/feature-ledger.json`; every shipped Tauri
command must belong to exactly one feature.

A product feature lands as one change containing its shared backend/Tauri
contract, macOS, Android, and Windows behavior, UI and accessibility states,
unit and contract tests, and a hands-on scenario for each platform. Each
scenario must
produce a screenshot, accessibility-tree or screen-reader log, and measured
latency against a stated p95 budget. Restart, offline, one relevant failure,
and release evidence are mandatory. A platform may be absent only when the
capability is removed from the product and the ledger marks it `removed`.

Use `npm --prefix crates/copypaste-ui run dev:android` for the Android emulator
and `npm --prefix crates/copypaste-ui run dev:native` on macOS or Windows. Run
the scenario named by the feature ledger and retain its evidence under
`artifacts/native/`; the nightly and release workflows upload the same evidence
shape.

For Android, set `ANDROID_HOME` to the SDK and `NDK_HOME` to its installed NDK
before running the command. The pinned Tauri toolchain also accepts
`ANDROID_SDK_ROOT` as an SDK fallback; CI exports `ANDROID_NDK_HOME` alongside
`NDK_HOME` for downstream Android tools. Android Studio may write
`crates/copypaste-ui/src-tauri/gen/android/local.properties`, which is ignored
and must remain untracked.

Performance credit is platform-specific. A credited p95 names a JSON runtime
measurement artifact with samples; configured budgets, source-code strings,
and prose reports are not evidence. An unmeasured platform stays `uncredited`
rather than borrowing another platform's number.

Run `npm --prefix crates/copypaste-ui run test:native-parity` to exercise the
fail-closed receipt gate without native hardware. Release publication requires
same-commit macOS, physical Android, and installed Windows release receipts.
The Windows receipt installs the canonical NSIS package, launches the installed
UI with its installed sidecar, verifies artifact and update-feed integrity, and
performs an in-place package update before uninstalling it. Manual nightly
Windows evidence exercises the same unsigned package path; unsigned packages
deliberately omit updater metadata.

Incomplete records fail validation. Do not encode completion with TODOs,
waivers, placeholders, skipped assertions, or green jobs that lack evidence.
`evidence_status: verified` is a runtime claim, not ledger prose. Its receipt
must list every credited state as `{feature_id, state}` under `feature_states`;
the receipt's outer `source.commit` and `source.run_id` bind those states to the
publication run. Missing, additional, or duplicate states fail publication.
`python3 scripts/check-feature-ledger.py --receipt-expectations` prints the
exact `platform:feature=state` arguments expected by the receipt gate.

## Reclaiming build artifacts

`python3 scripts/worktree-hygiene.py` reports what per-worktree build output
could be reclaimed and what it is preserving, with byte counts and a reason per
preserved path. It removes nothing without `--apply`.

Run the report at wave completion, once every worker terminal is released, and
apply it only after reading which paths it names. Manual recovery is the same
command; it is idempotent, so a second run after a partial failure finishes the
job.

It never touches the primary checkout. Bound that with
`scripts/target-budget.py`, below.

A worktree with uncommitted work, a running build, or any file whose content is
in no git object is preserved whole. `--root` overrides the searched worktree
parents; a root that does not exist is skipped rather than guessed at.

## Keeping `target/` bounded

`target/` grows one artifact generation per *configuration* — a version bump, a
`Cargo.lock` change, a different feature set, a toolchain change. Code edits
overwrite in place and cost nothing.

Mark each configuration you build, then sweep what no mark named:

```sh
python3 scripts/target-budget.py --mark -- build --workspace --tests --locked
python3 scripts/target-budget.py --mark -- clippy --workspace --all-targets --locked
python3 scripts/target-budget.py            # report; removes nothing
python3 scripts/target-budget.py --apply    # remove
```

Mark clippy separately even though it builds the same crates. The two share
only 164 units of 781 and 859, so marking `build` alone sweeps 695 clippy units
and the next lint gate rebuilds nearly everything.

A marked configuration keeps building at its normal cost; a configuration never
marked pays a full rebuild. The sweep refuses while any `.cargo-lock` under
`target/` is held, and nothing runs it for you. See
[ADR-0026](adr/0026-bound-the-primary-checkout-target-directory.md).

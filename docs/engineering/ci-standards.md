# CI engineering standards

Correctness and security precede reliability, performance, maintenance cost,
and runner cost. These rules apply to workflows, composite actions, build
scripts, platform adapters, and future AI-assisted changes.

## Validation layers

`CI Policy Gate` runs on every PR, merge queue entry, and main push. It validates
workflow and embedded shell syntax with actionlint/ShellCheck, runs zizmor's
high-severity/high-confidence security findings as errors, and tests parsed
repository contracts. Lower-confidence/informational findings remain visible
in a full `zizmor --offline .github` audit; they are not evidence of a confirmed
vulnerability. Pin tool versions in `scripts/ci/requirements.txt` and the
checksum-verified actionlint installer.

`CI` selects native validation conservatively. Ordinary documentation edits
avoid native compilation; release/platform/engineering contracts and every
unknown path require the full existing native matrix. Core Rust, generated
bridge sources, lockfiles, shared dependencies, build scripts, and any platform
code therefore always select all native checks. This initial selector favors
correctness over narrow per-package inference. `CI required` checks the actual
results of all selected jobs and rejects skipped, failed, or cancelled required
work. Configure branch protection to require `CI Policy Gate` and `CI required`
alongside existing security checks; repository settings are managed separately.

CodeQL keeps Swift and C++ traced compilation on PR/main and scheduled runs.
Supply-chain checks keep PR dependency review and secret/advisory coverage.
Mutation tests and experimental Linux matrices remain explicit scheduled or
manual work. Changes to experimental platform adapters require running their
matching opt-in workflow before claiming support.

## Build and release contracts

Build a production configuration once at an exact commit and promote those
bytes through qualification and publication. Preserve production optimization
and distinguish evidence/test profiles. Qualification may compile a documented
test harness; it may not silently rebuild the shipped application. Receipts
must bind platform, version, source, run, filename, size, and SHA-256. Signing
checks are independent trust checks, not redundant compilation.

Preflight must reject invalid versions, impossible input combinations, missing
required signing configuration, invalid external sources, and incompatible
contracts before expensive work where that information is available. Evidence
that depends on produced artifacts belongs after its producers. Model these
dependencies explicitly with `needs`; do not require future evidence up front.

Use reusable workflows and existing adapters. Every `needs` edge must represent
a data dependency or required gate. `always()` is reserved for cleanup,
diagnostics, or aggregators that explicitly reject every unsuccessful required
dependency. Never use advisory steps, retries, timeout inflation, or narrower
assertions to make required validation pass. Producer and publisher concurrency
must preserve active runs; ordinary obsolete PR validation can be cancelled.

Only successfully qualified artifacts may be published. Recovery must verify
the original source and qualified bytes, reject expired/ambiguous artifacts,
and avoid rebuilding successful producers. Repeated publication must verify
existing digests and refuse conflicting assets. Publish complete sets through
a draft transaction where supported; never overwrite a stable release asset.
Test all publisher behavior with fake APIs, without tags or production releases.

## Security and caches

Use full action commit SHAs, read-only workflow permissions, and explicit
job-scoped write permissions. Pass user-controlled inputs through environment
variables instead of interpolating them into shell programs. Checkout should
not retain credentials. Never run untrusted PR code with privileged self-hosted
runners or use its artifacts as trusted release inputs. Artifact transfers must
verify repository, workflow, source commit, successful producer jobs, expiry,
and exact content receipts. Pagination is mandatory for complete inventories.
Use `.github/actions/download-verified-artifact` for artifact transfers. It
resolves retry versions to immutable IDs, checks the archive digest and rejects
unsafe extraction or conflicting merged files. Never select a retry artifact
by the largest ID or silently fall back to an older expired/failed generation.

Dependency caches, compiler outputs, generated state, and signed artifacts are
different objects. Only the first two are ordinarily reusable across jobs.
Compiler keys must isolate OS, architecture, toolchain, profile, ABI and flags;
lockfiles must participate. Cargo remains responsible for validating its own
incremental/dependency fingerprints. Never restore signing material, secrets,
or credentials from caches. Untrusted branches may restore trusted dependency
caches but must not write release cache namespaces. Cache misses must build
cleanly; corrupt caches must fail validation or be discarded, never accepted as
qualified output. Measure restore/save cost and storage/eviction before adding
another cache layer. Benchmark sccache separately before adoption.

## Artifact retention

`config/ci-policy.json` owns each artifact producer's explicit retention period.
Diagnostics, performance reports, PR candidates and standalone preview builds
expire after three days. Intermediate signed builds expire after seven days.
The final qualified package set and assembled qualification receipts retain the
public repository maximum of 90 days. The original Linux package producer also
retains 90 days because upgrade qualification authenticates that producer against
the previous published release. Shorter retention must not remove that baseline.
Per-scenario diagnostic artifacts expire after three days; successful assembly
preserves the required qualification evidence in the final evidence artifact.

Published installer assets are separate from Actions storage. Application release
cleanup retains the five newest published stable `v*` application releases while
preserving their Git tags. Module releases and the signed marketplace are separate
distribution dependencies and must not be counted against that application limit.
Retention changes apply to new artifacts; existing artifacts require a separately
reviewed cleanup inventory. Keep repository-wide check/run/log retention unchanged
so shortening temporary artifact storage does not erase release identity metadata.

## Platform registration

Stable Linux eligibility is owned by `packaging/linux/release-contract.json`
and `scripts/release/linux_release_contract.py`; packaging and installed native
qualification must agree with them. macOS, Android and Windows retain their
existing release adapters, ABI/signing contracts and receipt validators.
Add a target by updating its contract, runner/toolchain/dependencies, build
adapter, artifact names, package formats, signing, and required qualification
together. Add negative fixtures for missing targets, wrong architecture and
skipped qualification. A matrix row alone does not establish support.

Shared actions own SDK/dependency setup; platform adapters own platform-specific
build and packaging. Do not copy a complete workflow to add a target. Keep
GNOME/KDE, X11/Wayland, runtime companions and package compatibility explicit.

## Required local checks

Use Python 3.12 or newer and the pinned requirements:

```sh
python3 -m pip install -r scripts/ci/requirements.txt
python3 scripts/ci/lint-workflows.py --actionlint actionlint
zizmor --offline --min-severity high --min-confidence high .github
python3 scripts/ci/policy.py
python3 -m unittest discover -s scripts/ci/tests -v
python3 -m unittest scripts/test_release_safety.py scripts/test_linux_qualification_bootstrap.py
python3 -m unittest discover -s scripts/release/tests -p 'test_*.py'
python3 scripts/ci/run-gates.py --check
python3 scripts/check-docs.py
```

The policy uses a full YAML parser, rejects duplicate keys and cycles, and
preserves GitHub's `on` key. Actionlint owns expression syntax; security tools
own generic attack-pattern detection. Repository policy owns stable job names,
dependency contracts, action pinning, timeouts and producer concurrency. New
rules require both valid fixtures and mutations that should fail.

GitHub supports `queue: max` for non-cancelling concurrency, retaining up to 100
pending runs. Production locks use it to prevent replacement of a pending
release. Actionlint 1.7.12 does not yet recognize this field. The lint adapter
first validates its enum and cancellation constraints using the YAML policy,
then filters only that obsolete-schema diagnostic. Every invocation proves
with valid/invalid fixtures that expression and ShellCheck failures still block.
All other actionlint diagnostics remain errors. Remove this compatibility filter
when the pinned upstream parser supports the field.
[GitHub queue behavior](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#concurrency)

## Measurement and runner decisions

Collect `python3 scripts/ci/metrics.py --output path/to/metrics.json` before and
after changes. Keep the run IDs, input modes, commit, event, sample count and
cache cohort. Compare workflow wall clock and critical paths, not sums of
parallel jobs. Keep failure/cancellation rates and p95 beside p50. Report
unknown metrics as unknown, never zero. Provisional review thresholds are the
baseline successful p95 plus 20%; they request investigation and do not justify
removing checks. A statistically weak baseline is not a hard timing assertion.

For one completed run, use `--run-id <id> --logs --summary path/to/summary.md`.
Add `--compare docs/engineering/ci-baseline.json` for cohort comparisons. The
comparison refuses percentage claims for unknown cache state or insufficient
successful samples; cold and warm samples are never combined.
The automatic `CI performance report` workflow uploads the same data after each
major workflow. `config/ci-performance-budgets.json` owns the investigation
thresholds. Phase timing preserves command exit status; cache log coverage and
unavailable observations must remain explicit in reports. A trusted reporting
workflow may read PR run metadata/logs but must never execute PR artifacts or code.

Standard GitHub-hosted runners remain the default. Optimize setup and duplicate
compilation first. Larger/persistent runners require measured end-to-end gain,
queue time, costs, maintenance and isolation analysis. No billing change or
paid resource is part of this migration. Self-hosted privileged runners must
never execute untrusted pull requests.

See [the initial audit](ci-audit.md) and [baseline data](ci-baseline.json).
Reference behavior comes from [GitHub workflow reuse](https://docs.github.com/en/actions/reference/workflows-and-actions/reusing-workflow-configurations),
[artifact transfer](https://docs.github.com/en/actions/tutorials/store-and-share-data),
[cache isolation](https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching),
[Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html),
[Gradle configuration cache adoption](https://docs.gradle.org/current/userguide/configuration_cache_adoption.html),
and [CodeQL compiled languages](https://docs.github.com/en/code-security/how-tos/find-and-fix-code-vulnerabilities/manage-your-configuration/codeql-for-compiled-languages).

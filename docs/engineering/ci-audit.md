# CI audit and measurement method

The initial source is `852aae607d7e3914001a1d0623c1d676640b4884`.
The [machine-readable baseline](ci-baseline.json) contains 111 completed runs,
job/step durations, conclusions, source SHAs, URLs, and the complete workflow
job dependency inventory. Up to 30 recent runs per workflow were enumerated;
job details cover the latest 10 completed runs per workflow where available.
No runs were cancelled or retried for this audit. Workflows without historical
runs are represented explicitly with empty samples.

`scripts/ci/metrics.py` can refresh the measurements through the GitHub API.
It paginates job inventories, separates unsuccessful runs and release reuse
from builds, and uses job completion times instead of workflow `updated_at`.
Scheduling delay includes dependency waiting; only the initial delay is called
initial queue time. Cache state is **unknown** unless supported by cache logs.
Unknown samples must never be relabelled warm or cold. The nearest-rank p95 of
a small sample is descriptive, not a reliable population estimate.

| Workflow | Successful samples | p50 wall clock | p95 wall clock |
| --- | ---: | ---: | ---: |
| CI | 7 | 24m 05s | 25m 49s |
| CodeQL | 9 | 40m 20s | 47m 58s |
| CI configuration | 10 | 20s | 24s |
| Compositor producer | 10 | 3m 38s | 4m 27s |
| Linux native qualification | 2 | 4m 25s | 6m 00s |

Provider-module timings include two different modules and must not be used as
a comparable build benchmark without separating their inputs. Successful
release builds can reuse earlier Linux evidence. Neither those runs nor
publication-only recovery measures a complete end-to-end fresh release.
There is no defensible historical fresh-release success percentile.

## Confirmed failure and bottlenecks

[Release 38779236382](https://github.com/dmytro-yevs/copypaste/actions/runs/38779236382)
failed first in `Linux full parity evidence gate` at 17:50:40 UTC on
2026-10-15: `ERROR: provide linux_evidence_run_id from the real desktop
qualification run described in packaging/linux/native-qualification.md`.
The complete run log and causal job log were inspected. The input was empty.
The missing input was detected after native builds had started. Downstream
qualification/publication were skipped consequences, not independent failures.
The source verifier simultaneously required a completed release run before
desktop qualification could start: orchestration forced multiple runs.

In [CI 38776064536](https://github.com/dmytro-yevs/copypaste/actions/runs/38776064536),
Android foundation dominated at 1,364 seconds (1,243 seconds inside the
combined format/analyze/test/build step). Windows foundation took 773 seconds;
Windows Rust capabilities took 700 seconds. These execute in parallel and
must not be added to estimate workflow wall clock.

In [CodeQL 38776064264](https://github.com/dmytro-yevs/copypaste/actions/runs/38776064264),
Swift's job took 2,207 seconds, including 1,940 seconds compiling under CodeQL.
C++ took 1,770 seconds, including 1,345 seconds building Flutter. Removing
traced native compilation requires a coverage comparison; no such speedup is
assumed. Production compiler caches must not hide traced source compilation.

In [Linux qualification 38783834206](https://github.com/dmytro-yevs/copypaste/actions/runs/38783834206),
the GNOME row took 326 seconds: dependencies 158 seconds, exact-package
scenarios 101 seconds, camera provisioning 35 seconds. The stable matrix has
only GNOME 46/X11/x86_64, but installs KDE/Plasma packages as well.

## Deduplication inventory

| Operation | Existing reason | Decision and measurement requirement |
| --- | --- | --- |
| Two Android Flutter/Gradle release invocations | Universal APK and two ARM splits | Preserve certificate, ABI contents, shared version code and updater names; evaluate AGP universal-plus-splits before switching |
| `pub get` before Flutter builds | Explicit lockfile enforcement plus implicit build resolution | Retain explicit locked resolution; subsequent builds may use `--no-pub` |
| Flutter format/analyze in every native matrix row | Combined foundation script | Portable checks can run once; retain platform-native tests/builds |
| GNOME installs KDE packages | Former wider Linux matrix | Select dependencies by the actual session contract |
| Rust module workspaces | Separate distributable modules and lockfiles | Cache each workspace; do not merge incompatible targets/profiles |
| Rust release and evidence builds | Production performance versus behavior tests | Preserve both profiles; benchmark before changing production optimization |
| Linux AppImage/DEB/RPM | Multiple delivery formats | Existing packaging already reuses one Flutter bundle; retain format-specific install checks |
| Runtime companion source build | Patched compositor provenance | Preserve source receipts and exact package binding |
| Signature/receipt verification at each boundary | Independent trust boundaries | Required correctness work; never remove as duplication |
| Qualified artifact download/upload during recovery | Preserve original bytes and run identity | Resume from qualified artifacts; avoid rebuilding successful producers |

No saved-time estimates are reported as measurements before an equivalent
post-change run exists. The raw baseline supports future comparison, including
unsuccessful/cancelled cohorts and per-step cache transfer overhead.

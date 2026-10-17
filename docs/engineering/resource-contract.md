# Resource contract and optimization lifecycle

CopyPaste must use **less than 300,000,000 bytes across all its processes in
ordinary operation**. OCR and semantic inference have separately measured peak
budgets. Their workers must return the application to the ordinary budget after
completion or the required reuse session. An idle retained model is not an
indefinite AI exception. Functional and UI parity on macOS, Android and Windows
remains mandatory.

## Authoritative records

- `config/optimization-registry.json` owns optimization IDs, implementation
  status, owner modules, invariant, platform coverage and evidence references.
- `config/runtime-resource-budgets.json` owns runtime thresholds and required
  scenarios. A null threshold is unresolved and cannot qualify a release.
- `config/ci-performance-budgets.json` and `scripts/ci/metrics.py` continue to own
  CI wall-clock measurements. Build time is not a runtime resource measurement.
- `docs/ui-memory.md`, `docs/semantic-inference.md` and the existing platform and
  release contracts own feature lifecycle and artifact/provenance requirements.

Do not duplicate executable thresholds in multiple configuration files.
Implementation limits, such as cache sizes and concurrency, are allocation
controls. They are not measured release budgets or proof of total RAM usage.

## Change lifecycle

Every resource-impacting change must update its existing registry entry or add
a stable ID. Record the owner, exact sources, allocation lifetime, cancellation,
shutdown, byte/queue/concurrency bounds, affected platforms, functional contract,
tradeoffs and remaining work. Retire superseded records explicitly.

Keep implementation and validation separate: proposed, partially implemented,
implemented, functional tests passed, native measured per platform, budget
qualified per platform, then release qualified. Regressed and superseded entries
remain visible. Evidence must identify the exact commit, artifact digest, build
mode, signing identity, OS/device, deterministic fixture, scenario, cache state,
sample interval, process start identity and budget version. Dirty source can
support investigation, but cannot be promoted as exact-commit release evidence.

New resource work needs a baseline and candidate comparison with equivalent
inputs. Measure CPU, startup/action latency, wakeups, disk/network I/O, memory
peak, steady state, recovery and sustained growth. Process isolation and reuse
are decisions based on these measurements. Do not weaken correctness, skip
coverage, silently discard data, raise a budget to hide regression, or infer
improvement from source changes alone.

## Native measurement

Measure UI, daemon, inference and other workers separately and as a process
family. Sample simultaneous totals; do not add unrelated process lifetime peaks.
Discover process creation/exit and retain coverage gaps. A low-frequency sampler
needs native tracing for transient peaks that can occur between samples.

- macOS: physical footprint, plus allocation/VM/GPU evidence for retained native
  memory. RSS alone misses compressed/swapped charges.
- Physical Android: PSS and SwapPss across the application and its services,
  with OS accounting validated before adopting the combined metric. Emulator
  results are supplemental.
- Installed Windows: private commit is the retained-memory guard in the current
  contract. Record private working set and shared/GPU attribution alongside it;
  private commit is not equivalent to physical resident RAM.

These platform metrics are deliberately explicit and are not interchangeable.
CPU, wakeup, I/O, network, AI peak, recovery and growth thresholds are currently
unresolved. Establish them from repeatable native release measurements; do not
invent a passing threshold. The existing 60-second lifecycle deadlines are
implementation behavior, not proof of native recovery within a measured budget.

Required scenarios are idle, active, repeated, recovery, hidden, sustained and
OCR/AI peak on all three platforms. Include large History and media, Quick Paste
cycles, capture bursts, sync/offline recovery, model switching/cancellation,
Android screenshot backlog and background/foreground transitions. Preserve
test fixture and functional assertions across baseline and candidate runs.

## Offline receipt gate

`scripts/profile/check-resource-receipts.py` checks the full scenario matrix,
exact source/build/budget identity, per-platform artifact consistency, combined
memory and the configured non-memory metrics. Missing evidence or thresholds
produce a nonzero exit, never a pass. This is an offline gate: the release
pipeline must first verify provenance and authenticity using its existing
qualification contracts. A string naming a provenance receipt is not signature
verification. Native collectors and automatic publication wiring remain open
work in `GOV-001`; this checker alone does not claim that wiring exists.

Each receipt is a JSON object containing:

```json
{
  "platform": "macos",
  "scenario": "idle",
  "commit": "<full source SHA>",
  "dirty": false,
  "build_mode": "release",
  "budget_version": "2026-10-17.1",
  "artifact_sha256": "<digest of the qualified artifact>",
  "signing_identity": "<verified identity>",
  "provenance_receipt": "<verified release receipt reference>",
  "device": "<test target>",
  "os_version": "<OS build>",
  "fixture_sha256": "<deterministic workload digest>",
  "controlled": true,
  "process_coverage_complete": true,
  "memory_metric": "physical_footprint_bytes",
  "samples": [
    {"elapsed_seconds": 0, "processes": [{"identity": "<pid:start>", "memory_bytes": 0}]},
    {"elapsed_seconds": 1, "processes": [{"identity": "<pid:start>", "memory_bytes": 0}]}
  ],
  "metrics": {
    "cpu_percent": 0,
    "wakeups_per_second": 0,
    "disk_bytes_per_second": 0,
    "network_bytes_per_second": 0
  }
}
```

The zeros above illustrate numeric fields only; they are not measurements.
Android also requires `physical_device: true`. Recovery and sustained scenarios
require `recovery_seconds` and `sustained_growth_bytes`, respectively. CPU is
percent of one logical core, summed across the process family; wakeups and I/O
are process-family rates over the recorded scenario. Include raw counters and
native tracing in the referenced provenance bundle.

Run the checker with `--commit <SHA> --output <task-artifact-path> <receipts...>`.
Its own regression tests run with
`python3 -m unittest discover -s scripts/profile -p 'test_resource_receipts.py' -v`.
The checker does not currently enforce scenario duration or repetition counts;
those must be established with the native baseline protocol before release
qualification. Two samples only establish structural validity, not a sustained
resource claim.

## Current implementation boundary

### Repeatable macOS developer diagnostic

Use `scripts/profile/collect-macos-resources.py --pid <Dev PID> --bundle
<absolute Dev.app path> --scenario <name> --duration 120 --interval 0.25
--source-receipt <source.json> --artifact-receipt <artifact.json> --output
<new artifact path>`. The collector never builds, launches, signs, changes
preferences, or accesses a key store. It fails if the root PID belongs to another
bundle. It includes bundle executables and descendants, remembers observed
helpers after reparenting, and aborts on PID reuse. It reads the existing memory
budget and exits 1 on an observed breach or 2 on incomplete collection.

The output includes simultaneous physical-footprint totals and raw CPU, wakeup,
RSS and disk counters. Network is explicitly missing. Sub-interval workers and
peaks can be missed, and initial/final process counter gaps remain. Referenced
source/artifact receipts are hashed, not authenticated. This diagnostic cannot
produce release qualification. Run its tests with
`python3 -m unittest discover -s scripts/profile -p 'test_macos_resources.py' -v`.

Developer measurements must use a separate Dev bundle, data store and file-key
backend, with ad-hoc signing. A production-key readiness failure is not a low-RAM
pass. Local optimized developer diagnostics currently enable only
`profile.release.package.copypaste-core.debug-assertions=true` to select the
existing development file key; Flutter still uses AOT Release. Record this
hybrid explicitly, preserve the full dirty-source manifest, and never promote
it as a production release receipt.

### Implemented controls and open work

History preview/icon caches have byte limits; protected source icon retention
also has a count ceiling. Visibility stops presentation-only Settings reads while notifications and runtime
event streams stay active. Desktop tray capture-state polling remains necessary
until typed private-mode change events exist; hiding must not stale tray controls. Refreshes are single
flight and obsolete visibility responses cannot update state. Core thumbnail
decoding is serialized per process. Android screenshot candidates are consumed
in 32-row metadata batches using timestamp/ID continuation; later scans retry
pending/failed rows from the policy cutoff.

Main and Quick Paste engines apply 16 MiB decoded-image and Skia-cache requests.
macOS explicitly selects Skia Metal. These limits exclude live surfaces and
cannot establish a process-family bound. Inspector images decode to display
dimensions; matching row preview pixels can be reused across layout changes.
Media completion updates local futures instead of notifying the entire History
controller. Selected full bodies are no longer retained in list rows, and
spoiler cards retain grapheme-safe summaries matching the native preview rule.

These changes still need native qualification. Reloadable History page windows,
aggregate transient-byte admission, bounded Android
cleanup executors, native retention profiling and complete platform collectors
remain tracked work. The 300 MB target must not be reported achieved until the
same release artifact passes the required native matrix.

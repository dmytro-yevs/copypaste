# Daemon Soak Test

Long-running stress test for `copypaste-daemon` that drives a steady
insert/list/delete workload while sampling RSS and CPU. Used to catch
slow leaks, file-descriptor exhaustion, and CPU drift before a release.

## When to run

- **Pre-release** (any `v0.x.0-beta` or `v0.x.0` tag) — required gate
- After landing changes that touch:
  - `crates/copypaste-daemon` ipc/runtime loop
  - `crates/copypaste-core` storage / SQLite (pool, WAL, observers)
  - clipboard observers (macOS / Linux / Windows backends)
- After bumping `sqlx`, `tokio`, `rusqlite`, or any long-lived runtime dep
- When investigating a "daemon RSS keeps climbing" bug report

Not needed for: docs-only changes, CLI-only changes, test-only changes.

## Quick start

```bash
# default: 1h run, 10 ops/sec, sample RSS every 30s
bash scripts/soak-daemon.sh

# short smoke (10 min) to validate the rig before a real soak
bash scripts/soak-daemon.sh --duration 600 --rate 5

# preview plan without running
bash scripts/soak-daemon.sh --dry-run
```

CSV samples land in `reports/perf/soak-<epoch>.csv` and the analyzer
runs automatically at the end. To re-analyze an existing CSV:

```bash
bash scripts/soak-report.sh --input reports/perf/soak-1716480000.csv
```

## Flags

`soak-daemon.sh`:

| Flag | Default | Meaning |
|------|---------|---------|
| `--duration <s>` | 3600 | total run length in seconds |
| `--rate <ops/s>` | 10 | driver loop ops per second |
| `--sample-interval <s>` | 30 | ps sampling interval |
| `--threshold <pct>` | 10 | RSS growth percent that triggers regression |
| `--report-file <path>` | `reports/perf/soak-<epoch>.csv` | output CSV |
| `--daemon-bin <path>` | `target/release/copypaste-daemon` | binary to soak |
| `--cli-bin <path>` | `target/release/copypaste` | driver CLI |
| `--dry-run` | off | print plan, exit 0 |
| `--help` | — | usage |

`soak-report.sh`:

| Flag | Default | Meaning |
|------|---------|---------|
| `--input <csv>` | required | CSV produced by soak-daemon |
| `--threshold <pct>` | 10 | RSS growth percent that triggers regression |
| `--width <cols>` | 60 | ASCII curve width |

## How to read the output

The analyzer prints a header block, then an ASCII RSS curve, then a
verdict line:

```
soak report — reports/perf/soak-1716480000.csv
  samples         : 120
  duration        : 3600s
  peak rss        : 48312 KB (47.2 MB)
  mean rss        : 46221 KB (45.1 MB)
  p99  rss        : 47990 KB (46.9 MB)
  first-stable rss: 45100 KB
  last rss        : 45980 KB
  growth          : +1.95% (threshold 10%)
  mean cpu        : 0.84%
  peak cpu        : 3.20%

rss curve (60 cols, min=45040 KB peak=48312 KB):
     30s | ##                                                        | 45100 KB
     60s | ###                                                       | 45230 KB
   ...
   3600s | ###                                                       | 45980 KB

OK: rss growth within threshold
```

### Interpreting

- **first-stable rss** — second sample (first is post-startup spike)
- **growth** — `(last - first_stable) / first_stable`. Compared to `--threshold`
- **peak vs mean** — large gap suggests bursty allocations; investigate
  whether retry / batch paths hold buffers too long
- **p99 ≈ peak** — sustained high-water; healthy
- **p99 << peak** — single spike; check daemon log for incident at that time
- **mean cpu > 5%** at default `--rate 10` — suspect tight loop or
  unindexed query; profile with `cargo flamegraph`

### Exit codes

- `0` — no regression (growth < threshold)
- `2` — daemon failed to start (check `reports/perf/soak-daemon-<pid>.log`)
- `3` — regression: RSS growth >= threshold

CI should fail on any non-zero exit from `soak-daemon.sh`.

## Tuning the workload

The default `--rate 10` is a sustained but not pathological load — it
exercises the storage write path, observer pipeline, and pruning
without saturating CPU. Crank it for stress runs:

```bash
bash scripts/soak-daemon.sh --rate 50 --duration 1800   # 30 min @ 50 ops/s
```

Past `--rate 100` the driver itself becomes the bottleneck; use a
benchmark harness (`crates/copypaste-bench`) instead.

## See also

- `scripts/perf-baseline.sh` — short-form criterion benches
- `crates/copypaste-bench` — read-only bench harness
- `docs/perf/README.md` — overall perf strategy

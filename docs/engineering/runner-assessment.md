# Runner assessment

Keep standard GitHub-hosted runners. This public repository currently gets
Linux/Windows x64 runners with 4 CPUs and 16 GB RAM, and macOS ARM64 runners
with 3 M1 CPUs and 7 GB RAM. Standard public-repository runners are free.
[GitHub specifications](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
were checked on 2026-10-15.

For comparison, current larger-runner prices are $0.022/min for Linux x64
8-core, $0.042/min for Windows x64 8-core and $0.102/min for macOS M2 Pro
5-core. A **hypothetical 20-minute job** costs $0.44, $0.84 and $2.04 respectively;
these are cost calculations, not measured build-duration forecasts. Larger
runners are billed for public repositories and require an eligible organization
plan. GitHub Pro alone is not a CPU upgrade.
[GitHub pricing](https://docs.github.com/en/billing/reference/actions-runner-pricing)

| Strategy | Potential benefit | Evidence and decision |
| --- | --- | --- |
| Standard hosted | Fresh isolated environments, no runner maintenance | Baseline and validation use these runners; retain |
| Larger hosted | More CPU/RAM for proven parallel compiler bottlenecks | No same-input paid-runner benchmark; do not purchase or recommend migration |
| Self-hosted | Persistent dependencies/compiler state | Security, patching, capacity and recovery costs unmeasured; never privileged untrusted PR execution |
| Dependency images | Less desktop/toolchain installation | Image update/storage cost and compatibility coverage unmeasured; remove unrelated dependencies first |

The cache inventory observed during validation contained 19 records totaling
10,780,479,490 bytes. The largest were Flutter SDK archives (about 2.07 GiB on
macOS and 1.60 GiB on Linux). Additional compiler caches would compete for
retention space. Measure restore/save overhead and eviction before adding a
cache tier. Existing main-branch caches were not deleted during this migration.

Changing runner sizes requires repeated comparable cold/warm runs, queue-time
measurements, peak memory and end-to-end critical-path results. Faster isolated
compilation is insufficient if another job or external qualification dominates.
No billing setting, paid resource or self-hosted runner configuration changed.

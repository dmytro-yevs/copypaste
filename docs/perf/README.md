# Perf baseline workflow

Tooling for tracking criterion benchmark baselines across commits and flagging
regressions in PRs.

## Components

| Artifact | Role |
|---|---|
| `crates/copypaste-bench/` | Read-only criterion harness (encryption, hash, storage). Owned by the bench worker — do not edit here. |
| `scripts/perf-baseline.sh` | Runs the harness, consolidates `target/criterion/**/new/estimates.json` into a single JSON, optionally diffs against the tracked baseline. |
| `scripts/perf-compare.sh` | Standalone diff tool — takes two baseline JSONs and prints a markdown table with delta %. Exits non-zero on regression. |
| `reports/perf/baseline-main.json` | Tracked baseline that PR runs compare against. Refreshed via `--update-baseline` on a known-good rev. |
| `reports/perf/baseline-<git-rev>.json` | Per-run artifact, one per invocation. |

`reports/` is gitignored (orchestrator owns `reports/STATUS.md`); the
`baseline-main.json` file is intentionally tracked and committed when refreshed.

## Local run

```bash
# Sanity check, no benches executed:
bash scripts/perf-baseline.sh --dry-run

# Full run on current HEAD, diff against baseline-main.json:
bash scripts/perf-baseline.sh

# Stricter threshold (default 10%):
bash scripts/perf-baseline.sh --threshold 5

# Refresh the tracked baseline after a known-good change:
bash scripts/perf-baseline.sh --update-baseline
git add reports/perf/baseline-main.json
git commit -m "perf: refresh baseline (<reason>)"
```

## Diffing two arbitrary baselines

```bash
bash scripts/perf-compare.sh \
  reports/perf/baseline-main.json \
  reports/perf/baseline-abcdef0.json \
  --threshold 10
```

Exit codes:

- `0` no regressions at or above threshold
- `1` usage / setup error
- `3` at least one bench regressed by `>= threshold`

## PR workflow

1. CI checks out the PR branch.
2. Runs `bash scripts/perf-baseline.sh --threshold 10`.
3. On exit code `3`, parse the table from stdout and post as a PR comment
   (e.g. via `gh pr comment $PR --body-file <(bash scripts/perf-compare.sh ...)`).
4. Block merge until the comment is acknowledged or the regression is fixed.

Suggested GitHub Actions sketch (not committed here — orchestrator owns CI):

```yaml
- name: perf baseline
  run: bash scripts/perf-baseline.sh --threshold 10 | tee perf-report.md
- name: post regression
  if: failure()
  run: gh pr comment ${{ github.event.pull_request.number }} --body-file perf-report.md
  env:
    GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

## Baseline JSON schema

```json
{
  "schema": "copypaste-perf-baseline/v1",
  "git_rev": "abcdef0",
  "git_branch": "release/v0.2.0-beta",
  "timestamp_utc": "2026-05-23T09:00:00Z",
  "benches": {
    "encrypt/1KB":   { "mean_ns": 1234.5, "median_ns": 1230.0, "std_dev_ns": 12.0 },
    "decrypt/100KB": { "mean_ns": ...,    "median_ns": ...,    "std_dev_ns": ... }
  }
}
```

Bench ids are the relative path under `target/criterion/` (e.g. group/id) with
`/` separators, matching criterion's directory layout.

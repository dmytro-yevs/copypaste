# Performance baseline — design-system redesign

Recorded in **Slice 1** (bd `CopyPaste-g27b.8`, design.md Decision 15 / task 1.18).
The **thresholds are fixed here, up front** — not chosen after seeing the result.
Slice 6 (task 6.15) re-measures against this baseline and gates on the budgets.

## Budget policy (release gate — slice 6)

| Metric | Budget |
|--------|--------|
| Popup open→first-render p95 regression | ≤ **max(15%, +40 ms)** over baseline |
| CSS gzip delta (per entry) | ≤ **20 KB** |
| JS gzip delta (per entry) | ≤ **30 KB** |

Any exception must be documented and justified with reviewer sign-off in the
slice-6 PR.

## Baseline — bundle size (gzip)

Measured with `pnpm exec vite build` on the pre-redesign bare-strip state (empty
`index.css`), before any Slice-1 styling landed. Method: gzip of each emitted
entry/asset chunk as reported by Vite's build summary.

| Asset (role) | raw | **gzip (baseline)** |
|--------------|-----|---------------------|
| CSS (shared `styles/index.css`) | 0.00 kB | **0.02 kB** |
| `main` entry JS | 113.78 kB | **34.91 kB** |
| shared vendor JS (react/react-dom) | 208.80 kB | **66.71 kB** |
| `popup` entry JS | 12.26 kB | **4.75 kB** |
| shared `webview` JS | 16.27 kB | **3.59 kB** |

### Reference — current sizes (informational, not the baseline)

Slice 1 + the early slice-2 token scales (tokens + base layers; primitives/
patterns/shell still empty):

| Asset | gzip (current) | Δ vs baseline |
|-------|----------------|---------------|
| CSS | 2.40 kB | **+2.38 kB** (budget 20 KB) |
| `main` entry JS | 34.99 kB | +0.08 kB |
| shared vendor JS | 66.98 kB | +0.27 kB |
| `popup` entry JS | 4.98 kB | +0.23 kB |

All deltas are comfortably within budget.

## Baseline — popup open→first-render latency

**Methodology (fixed now):** a `performance.mark` pair around popup mount
(`performance.measure("popup-open-to-render", "popup-mount-start",
"popup-first-render")`, instrumented in `src/popup/main.tsx`), sampled over
**10 warm-cache runs**, reporting **p50 and p95**.

**Concrete baseline (measured now)** — `http://localhost:1420/popup.html?mock=1`,
10 reload iterations, `performance.getEntriesByName("popup-open-to-render")`:

| Stat | ms |
|------|----|
| **p50** | **9.9** |
| **p95** | **37.9** |

Raw samples (ms): 54.7, 6.2, 7.4, 1.5, 10.7, 9.0, 12.9, 11.2, 17.3, 7.4. The p95
is dominated by a single first-navigation cold-start outlier (54.7 ms, JIT/module
warmup); the 9 warm samples cluster in 1.5–17.3 ms.

**Scope:** this is a **Chromium / Vite-dev-server QA-harness** measurement, not the
packaged Tauri/WKWebView popup latency. The **packaged** number is the release-gate
measurement re-captured in slice 6 (task 6.15) against this instrumentation; the
p95-regression budget above is applied there. This browser baseline is recorded now
per Decision 15 ("baseline numbers recorded in slice 1").

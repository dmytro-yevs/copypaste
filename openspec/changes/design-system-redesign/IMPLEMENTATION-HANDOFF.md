# Implementation handoff — `design-system-redesign`

**Status:** spec is implementation-ready. `openspec validate design-system-redesign --strict` passes.
Six staff-review rounds (STAFF-REVIEW*.md) are all resolved. Start slice-by-slice.

## Where you are

- **Worktree:** `/Users/dmytro/Documents/CopyPaste/.claude/worktrees/wt-99994`
- **Branch:** `worktree-99994` (isolated; base = a WIP snapshot of the user's tree). Do NOT touch the
  user's own branches. Integration back to main is via
  `git cherry-pick <snap>..worktree-99994` (read `.claude-wt-base` for `snap`; NEVER merge the
  snapshot commit).
- **Spec (source of truth):** `openspec/changes/design-system-redesign/`
  - `proposal.md` (why/what) · `design.md` (16 Decisions + inventory + risks + migration plan) ·
    `tasks.md` (the implementation checklist, 6 slices + cross-cutting) ·
    `specs/{design-tokens,component-library,preview-gallery}/spec.md` (normative requirements).
- **Design reference (visual source of truth):** `copypaste-design-reference.html` (repo root, 979
  lines) + `docs/design/STYLEGUIDE.md`. `docs/design/DESIGN-SYSTEM-v2.md` is SUPERSEDED — do not use.

## How to start

Run **`/opsx:apply`** (OpenSpec apply) for this change, OR follow `tasks.md` directly. Implement
**slice by slice** — each slice compiles, tests, and leaves the app usable before the next:

1. **Slice 1** — `src/styles/{reset,tokens,base}.css` + cascade `@layer` + external
   `public/theme-bootstrap.js` + `UIPrefs` additive fields/validation + **remove legacy v1/v2/v3
   migration** (task 1.10a) + perf baseline.
2. **Slice 2** — typed primitives + shared `Dialog` (ref-counted scroll-lock) + disclosure/tab a11y
   foundations + **minimal DEV+MOCK gallery shell** (task 2.13).
3. **Slice 3** — History + Popup via shared clipboard units (`normalizeContentKind`/`ContentTile`/
   `ClipPreview`/`ClipMetadata`) + sensitive-masking contract.
4. **Slice 4** — Devices (device action table: unpair/revoke/revoke_and_rotate).
5. **Slice 5** — Settings + sidebar + About + Logs + banners + toast + Appearance (Theme/Accent/
   Translucency).
6. **Slice 6** — full gallery + automated Playwright suite (`@axe-core/playwright`) + packaged-Tauri
   smoke gate + perf budget check.

## DO NOT reopen these settled contracts (6 review rounds)

- Theme bootstrap is an **external** same-origin classic script `public/theme-bootstrap.js` — NOT
  inline (CSP is `script-src 'self'`). It sets `dataset.themeBootstrapped="1"`; the module asserts it.
- **No `DevViewId` in `store.ts`**; `view` stays `ProductionViewId`, in-memory. Gallery = dev-only
  nav branch (`DEV && MOCK`) OUTSIDE the production view registry, dynamic-imported.
- **`@axe-core/playwright`** is the single new dev/test dependency (test-only, not shipped).
- **Performance budgets are fixed:** popup p95 ≤ max(15%, +40ms); CSS gzip ≤ 20 KB; JS gzip ≤ 30 KB.
- **Packaged-Tauri smoke (`pnpm test:tauri-smoke` + macOS CI) is the product release gate**; browser
  `?mock=1` Playwright is an internal QA harness that supplements, not replaces, it.
- **No back-compat:** additive `UIPrefs` fields on the current `copypaste-ui-prefs-v4` key; the
  existing v1/v2/v3 legacy migration branches are intentionally REMOVED (old-key users reset to
  defaults — accepted).
- Packaged product target = **macOS 13+**. `bundle.targets:"all"` mismatch is tracked in
  `CopyPaste-4w1a` (do not rely on it as a support signal).

## Repo state caveats

- **Web bare-strip (CopyPaste-3sys) is DONE + committed** — components are bare semantic HTML (the
  clean canvas this redesign styles).
- **Android bare-strip (CopyPaste-g5u1) edits are committed but NOT compile-verified** — this worktree
  has no Android SDK and JDK 26 (AGP needs ≤21); verify via the user's Android toolchain / Docker.
  Independent of this web redesign.
- **Preview infra (CopyPaste-fm0s) DONE** — real app runs in browser at `localhost:1420/?mock=1`
  (mock data, no daemon) or `?bridge=1` (live daemon). Use `?mock=1` for gallery/visual work.
- Popup browser crash fixed (CopyPaste-z01l).

## bd tracking

- Epic: **`CopyPaste-g27b`** (UI redesign). Children incl. `g27b.2` gallery, `g27b.3` tokens.css,
  `g27b.4` functional-inline-styles follow-up, `g27b.5` a11y 48dp tap-targets.
- bd memories (read at session start): `redesign-quality-bar`, `redesign-process`,
  `redesign-spec-decisions`, `android-g5u1-ruleset`.
- Follow-ups: `CopyPaste-4w1a` (tauri targets:"all"), `CopyPaste-z01l` (closed).

## Verification per slice

- `pnpm -C crates/copypaste-ui exec tsc --noEmit` + `pnpm -C crates/copypaste-ui test` after each
  slice; visual check via `ui-verifier` on `localhost:1420/?mock=1`; keep each slice's app usable.

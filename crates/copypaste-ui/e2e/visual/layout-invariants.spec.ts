/**
 * Assertive layout + a11y gate (CopyPaste-g27b.13.2 / .13.3).
 *
 * WHY THIS EXISTS: the pixel-diff visual specs (ojas.1) only compared each
 * surface to a golden baseline. When the baseline itself encodes a defect
 * (text clipped at a card edge, a value cut off at a narrow width, the page
 * scrolling sideways) the diff still passes — which is exactly how a batch of
 * real layout bugs (user screenshots #7–#14) shipped "green". This gate makes
 * NO reference to a baseline. It measures the live DOM geometry and FAILS when
 * the layout is objectively broken, across the responsive × theme matrix that
 * the pixel specs never resized through.
 *
 * Invariants asserted per surface × width × theme:
 *   1. No horizontal document overflow (a desktop window must never scroll
 *      sideways).
 *   2. No hard-clipped text — an element whose text is wider than its box and
 *      is clipped (overflow hidden/clip) WITHOUT the intentional single-line
 *      `text-overflow: ellipsis` truncation contract.
 *   3. No content spilling past a clipping ancestor's edge (the "value cut off
 *      on the right" class — user #7/#11).
 *   4. Structural composition checks (CopyPaste-7w060.9): geometry-only probes
 *      above never look at how two elements relate to each other, so a toast
 *      overlapping the sidebar footer, a view rendering both its empty and
 *      populated states at once, an illegibly-short or runaway-tall row, and
 *      an unbounded-width shell all shipped green under invariants 1-3 alone.
 *      This class asserts: toast-stack vs sidebar/footer bounding-box
 *      non-intersection, Devices populated-list XOR empty-state, History row
 *      height bounds, and rendered content width vs the --content-max-width
 *      token.
 * Plus an @axe-core/playwright scan: zero serious/critical a11y violations.
 *
 * Runs headless against the Vite mock harness (?mock=1) — fully local, no
 * daemon, no packaged Tauri build. This is the part of the Slice-6 gate that
 * can and MUST run on every CI push; the packaged-Tauri smoke (tauri-driver)
 * is a separate, heavier probe (e2e/tauri-smoke).
 */

import { test, expect, type Page } from "playwright/test";
import AxeBuilder from "@axe-core/playwright";
import {
  gotoMockApp,
  gotoMockPopup,
  applyTheme,
  navigateToView,
  clickSettingsTab,
  triggerHistoryToast,
} from "./helpers";

// Narrow (a tight window / small display), normal, and wide. The narrow width
// is where "value clipped on the right" (user #7) reproduces; the pixel specs
// only ever ran at the fixed 1280 canvas and never saw it.
const WIDTHS = [400, 900, 1440] as const;
const THEMES = ["dark", "light"] as const;
const HEIGHT = 800;

// ---------------------------------------------------------------------------
// The geometry probe. Serialised into the page and executed via page.evaluate.
// Returns the objective evidence: which elements clip, which spill past a
// clipping ancestor, and whether the document itself scrolls sideways.
// ---------------------------------------------------------------------------
type GeomHit = {
  el: string;
  sw: number;
  cw: number;
  ovX: string;
  txt: string;
};
type GeomSpill = {
  el: string;
  right: number;
  parentRight: number;
  parent: string;
  txt: string;
};
type GeomResult = {
  viewportW: number;
  docOverflowX: { sw: number; cw: number } | null;
  clipped: GeomHit[];
  spill: GeomSpill[];
};

async function probe(page: Page): Promise<GeomResult> {
  return page.evaluate(() => {
    const TOL = 2;
    const short = (e: Element): string => {
      let s = e.tagName.toLowerCase();
      if ((e as HTMLElement).id) s += "#" + (e as HTMLElement).id;
      const cls = (e.getAttribute("class") || "").trim();
      if (cls) s += "." + cls.split(/\s+/).slice(0, 3).join(".");
      return s;
    };
    const isVisible = (e: Element): boolean => {
      const r = e.getBoundingClientRect();
      const cs = getComputedStyle(e);
      return (
        r.width > 1 &&
        r.height > 1 &&
        cs.visibility !== "hidden" &&
        cs.display !== "none" &&
        cs.opacity !== "0"
      );
    };
    const de = document.documentElement;
    const docOverflowX =
      de.scrollWidth > de.clientWidth + TOL
        ? { sw: de.scrollWidth, cw: de.clientWidth }
        : null;

    const clipped: GeomHit[] = [];
    const spill: GeomSpill[] = [];
    const all = Array.from(document.querySelectorAll("body *")).filter(
      isVisible,
    );
    for (const e of all) {
      const cs = getComputedStyle(e);
      // (2) hard-clipped text — leaf text node, clipped, NOT ellipsis-truncated.
      if (e.scrollWidth > e.clientWidth + TOL) {
        const clips = cs.overflowX === "hidden" || cs.overflowX === "clip";
        const ellipsisContract =
          cs.whiteSpace.startsWith("nowrap") &&
          cs.textOverflow === "ellipsis";
        const leafText =
          e.childElementCount === 0 && (e.textContent || "").trim().length > 0;
        if (clips && leafText && !ellipsisContract) {
          clipped.push({
            el: short(e),
            sw: e.scrollWidth,
            cw: e.clientWidth,
            ovX: cs.overflowX,
            txt: (e.textContent || "").trim().slice(0, 60),
          });
        }
      }
      // (3) spill past a clipping ancestor's right edge. Only a HARD clip
      // (hidden/clip) hides content invisibly — that is the bug. Exclude:
      //   • auto/scroll parents (intentional scroll regions, e.g. .set-tabs)
      //   • the single-line ellipsis contract (nowrap + text-overflow:ellipsis,
      //     e.g. .row__title/.row__meta) which truncates visibly and on purpose.
      const p = e.parentElement;
      if (p) {
        const pcs = getComputedStyle(p);
        const pHardClip =
          pcs.overflowX === "hidden" || pcs.overflowX === "clip";
        const pEllipsis =
          pcs.whiteSpace.startsWith("nowrap") &&
          pcs.textOverflow === "ellipsis";
        if (pHardClip && !pEllipsis) {
          const r = e.getBoundingClientRect();
          const pr = p.getBoundingClientRect();
          if (r.right > pr.right + TOL || r.left < pr.left - TOL) {
            spill.push({
              el: short(e),
              right: Math.round(r.right),
              parentRight: Math.round(pr.right),
              parent: short(p),
              txt: (e.textContent || "").trim().slice(0, 40),
            });
          }
        }
      }
    }
    return {
      viewportW: window.innerWidth,
      docOverflowX,
      clipped: clipped.slice(0, 40),
      spill: spill.slice(0, 40),
    };
  });
}

function fmt(r: GeomResult): string {
  const lines: string[] = [];
  if (r.docOverflowX)
    lines.push(
      `  DOC scrolls sideways: scrollWidth=${r.docOverflowX.sw} > clientWidth=${r.docOverflowX.cw}`,
    );
  for (const c of r.clipped)
    lines.push(
      `  CLIPPED ${c.el} [sw=${c.sw} cw=${c.cw} overflowX=${c.ovX}] "${c.txt}"`,
    );
  for (const s of r.spill)
    lines.push(
      `  SPILL   ${s.el} right=${s.right} > parent ${s.parent} right=${s.parentRight} "${s.txt}"`,
    );
  return lines.join("\n");
}

// ---------------------------------------------------------------------------
// Surfaces. Each `open` leaves the page on the target surface, ready to probe.
// ---------------------------------------------------------------------------
type Surface = { name: string; open: (page: Page) => Promise<void> };

const SURFACES: Surface[] = [
  { name: "history", open: async (p) => { await gotoMockApp(p); } },
  {
    name: "devices",
    open: async (p) => {
      await gotoMockApp(p);
      await navigateToView(p, "Devices");
    },
  },
  {
    name: "devices-expanded",
    open: async (p) => {
      await gotoMockApp(p);
      await navigateToView(p, "Devices");
      // Expand the first paired device to reveal the OS/IP/Fingerprint grid —
      // the prime "value clipped on the right at narrow width" suspect (#7).
      const first = p.locator('[data-testid="device-card"], .dev-card, .device-row').first();
      if (await first.count()) await first.click().catch(() => {});
    },
  },
  {
    name: "settings-general",
    open: async (p) => {
      await gotoMockApp(p);
      await navigateToView(p, "Settings");
      await clickSettingsTab(p, "General");
    },
  },
  {
    name: "settings-display",
    open: async (p) => {
      await gotoMockApp(p);
      await navigateToView(p, "Settings");
      await clickSettingsTab(p, "Display");
    },
  },
  {
    name: "settings-sync",
    open: async (p) => {
      await gotoMockApp(p);
      await navigateToView(p, "Settings");
      await clickSettingsTab(p, "Sync");
    },
  },
  {
    name: "settings-storage",
    open: async (p) => {
      await gotoMockApp(p);
      await navigateToView(p, "Settings");
      await clickSettingsTab(p, "Storage");
    },
  },
  {
    name: "settings-about",
    open: async (p) => {
      await gotoMockApp(p);
      await navigateToView(p, "Settings");
      await clickSettingsTab(p, "About");
    },
  },
  {
    name: "settings-logs",
    open: async (p) => {
      await gotoMockApp(p);
      await navigateToView(p, "Settings");
      await clickSettingsTab(p, "Logs");
    },
  },
  { name: "popup", open: async (p) => { await gotoMockPopup(p); } },
];

for (const surface of SURFACES) {
  for (const width of WIDTHS) {
    for (const theme of THEMES) {
      test(`geometry: ${surface.name} @ ${width} / ${theme}`, async ({ page }) => {
        await page.setViewportSize({ width, height: HEIGHT });
        await surface.open(page);
        await applyTheme(page, theme, "indigo");
        await page.waitForTimeout(120);
        const r = await probe(page);
        const problems =
          (r.docOverflowX ? 1 : 0) + r.clipped.length + r.spill.length;
        expect(
          problems,
          `Layout defects on ${surface.name} @ ${width}px / ${theme}:\n${fmt(r)}`,
        ).toBe(0);
      });
    }
  }
}

// a11y — axe scan per surface at normal width, in BOTH themes (color-contrast
// differs by theme, so a dark-only scan misses light-theme failures). Serious /
// critical violations fail the gate (the "contrast / name-role-value /
// nested-interactive" net the pixel specs never had).
for (const surface of SURFACES) {
  for (const theme of THEMES) {
    test(`a11y: ${surface.name} / ${theme}`, async ({ page }) => {
    await page.setViewportSize({ width: 900, height: HEIGHT });
    await surface.open(page);
    await applyTheme(page, theme, "indigo");
    await page.waitForTimeout(120);
    const results = await new AxeBuilder({ page })
      .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
      .analyze();
    const bad = results.violations.filter(
      (v) => v.impact === "serious" || v.impact === "critical",
    );
    const msg = bad
      .map(
        (v) =>
          `  [${v.impact}] ${v.id}: ${v.help} (${v.nodes.length} node(s)) e.g. ${v.nodes[0]?.target?.join(" ")}`,
      )
      .join("\n");
    expect(
      bad.length,
      `a11y violations on ${surface.name} / ${theme}:\n${msg}`,
    ).toBe(0);
    });
  }
}

// --- Targeted regression guards for the specific defects fixed in g27b.30/.31/.32.
// These encode the audit's findings as permanent checks so the classes can't
// silently come back (the pixel baselines never caught them).

// g27b.30 — history rows must be tall enough for their own content: no row's
// content may overflow its allocated box (that is what produced the 2-3px
// inter-row overlap at normal width and the illegible stacking at narrow width).
for (const width of WIDTHS) {
  test(`row-fit: history rows contain their content @ ${width}`, async ({ page }) => {
    await page.setViewportSize({ width, height: HEIGHT });
    await gotoMockApp(page);
    await page.waitForTimeout(150);
    const overflows = await page.evaluate(() => {
      const rows = Array.from(document.querySelectorAll(".row"));
      return rows
        .filter((r) => r.scrollHeight > r.clientHeight + 2)
        .map((r) => ({
          cls: (r.getAttribute("class") || "").slice(0, 40),
          scrollH: r.scrollHeight,
          clientH: r.clientHeight,
          text: (r.textContent || "").trim().slice(0, 40),
        }));
    });
    expect(
      overflows.length,
      `History rows whose content overflows the allocated row height @ ${width}px:\n` +
        overflows
          .map((o) => `  ${o.cls} scrollH=${o.scrollH} clientH=${o.clientH} "${o.text}"`)
          .join("\n"),
    ).toBe(0);
  });
}

// g27b.32 — Sync credential inputs must be wide enough that their default value
// is fully visible (no native horizontal text-scroll hiding content).
for (const theme of THEMES) {
  test(`input-fit: sync credential inputs show their value / ${theme}`, async ({ page }) => {
    await page.setViewportSize({ width: 900, height: HEIGHT });
    await gotoMockApp(page);
    await navigateToView(page, "Settings");
    await clickSettingsTab(page, "Sync");
    await applyTheme(page, theme, "indigo");
    await page.waitForTimeout(150);
    const clipped = await page.evaluate(() => {
      const inputs = Array.from(
        document.querySelectorAll('.set-pane input:not([type="range"]):not([type="checkbox"])'),
      ) as HTMLInputElement[];
      return inputs
        .filter((i) => i.value && i.scrollWidth > i.clientWidth + 2)
        .map((i) => ({
          name: i.getAttribute("aria-label") || i.getAttribute("placeholder") || i.name || "input",
          scrollW: i.scrollWidth,
          clientW: i.clientWidth,
        }));
    });
    expect(
      clipped.length,
      `Sync inputs whose default value is clipped (too narrow) / ${theme}:\n` +
        clipped.map((c) => `  ${c.name} scrollW=${c.scrollW} clientW=${c.clientW}`).join("\n"),
    ).toBe(0);
  });
}

// g27b.31 — at the app's real minimum window width (720 per tauri.conf.json) the
// Settings sub-tab bar must not overflow horizontally (it wraps instead of
// hiding tabs behind an affordance-less scroll).
test("tabbar-fit: settings tablist fits at 720px minWidth", async ({ page }) => {
  await page.setViewportSize({ width: 720, height: 460 });
  await gotoMockApp(page);
  await navigateToView(page, "Settings");
  await page.waitForSelector('[role="tablist"]', { timeout: 10_000 });
  await page.waitForTimeout(150);
  const tabs = await page.evaluate(() => {
    const tl = document.querySelector('[role="tablist"]') as HTMLElement | null;
    if (!tl) return null;
    return { scrollW: tl.scrollWidth, clientW: tl.clientWidth };
  });
  expect(tabs, "no [role=tablist] found").not.toBeNull();
  expect(
    tabs!.scrollW,
    `Settings tab bar overflows horizontally at 720px (scrollW=${tabs!.scrollW} > clientW=${tabs!.clientW}) — tabs hidden behind a scroll instead of wrapping`,
  ).toBeLessThanOrEqual(tabs!.clientW + 2);
});

// --- Structural composition assertions (CopyPaste-7w060.9 / A1.3). --------
// The geometry probe above only ever measures ONE element's own box against
// itself or its ancestor. None of the below classes intersect that: they
// need two elements' rects compared, a mutually-exclusive-state check across
// a whole subtree, a per-row height band, or a live token-vs-render compare.

/** Axis-aligned rectangle intersection (same rounding tolerance style as `probe`). */
function rectsIntersect(
  a: { top: number; right: number; bottom: number; left: number },
  b: { top: number; right: number; bottom: number; left: number },
): boolean {
  return a.left < b.right && a.right > b.left && a.top < b.bottom && a.bottom > b.top;
}

// 7w060.2 fixed the toast stack bleeding into the sidebar footer's band at
// narrow window widths — anchoring it bottom-right instead of center. Assert
// this permanently: the toast-stack must never overlap either sidebar rect.
for (const width of [720, 900] as const) {
  test(`composition: toast-stack does not overlap sidebar @ ${width}`, async ({ page }) => {
    await page.setViewportSize({ width, height: HEIGHT });
    await gotoMockApp(page);
    await triggerHistoryToast(page);
    const rects = await page.evaluate(() => {
      const rectOf = (sel: string) => {
        const el = document.querySelector(sel);
        if (!el) return null;
        const r = el.getBoundingClientRect();
        return { top: r.top, right: r.right, bottom: r.bottom, left: r.left };
      };
      return {
        toast: rectOf(".toast-stack"),
        sidebar: rectOf(".sb"),
        sidebarFoot: rectOf(".sb__foot"),
      };
    });
    expect(rects.toast, ".toast-stack not found after triggering bulk-copy toast").not.toBeNull();
    expect(rects.sidebar, ".sb (sidebar) not found").not.toBeNull();
    expect(rects.sidebarFoot, ".sb__foot not found").not.toBeNull();
    const overlapsSidebar = rectsIntersect(rects.toast!, rects.sidebar!);
    const overlapsFooter = rectsIntersect(rects.toast!, rects.sidebarFoot!);
    expect(
      overlapsSidebar || overlapsFooter,
      `toast-stack ${JSON.stringify(rects.toast)} overlaps sidebar ${JSON.stringify(rects.sidebar)} / footer ${JSON.stringify(rects.sidebarFoot)} @ ${width}px`,
    ).toBe(false);
  });
}

// The Devices "no other devices paired" EmptyState and the populated peer
// list (.devrow) must be strictly mutually exclusive — never both, never
// neither. Exercised via the `?peersEmpty=1` mock scenario flag (mockIpc.ts),
// which makes the previously-unreachable empty branch testable.
test("composition: devices — populated list XOR empty state (default fixture)", async ({ page }) => {
  await page.setViewportSize({ width: 900, height: HEIGHT });
  await gotoMockApp(page);
  await navigateToView(page, "Devices");
  await page.waitForTimeout(150);
  const hasRows = (await page.locator(".dev-list").first().locator(".devrow:not(.this)").count()) > 0;
  const hasEmpty = (await page.locator(".empty").count()) > 0;
  expect(
    hasRows !== hasEmpty,
    `expected exactly one of {populated .devrow list, .empty state} with the default (non-empty) peers fixture; got hasRows=${hasRows} hasEmpty=${hasEmpty}`,
  ).toBe(true);
  expect(hasRows, "default fixture has peers — expected .devrow rows to render").toBe(true);
});

test("composition: devices — populated list XOR empty state (?peersEmpty=1)", async ({ page }) => {
  await page.setViewportSize({ width: 900, height: HEIGHT });
  await page.goto("/?mock=1&peersEmpty=1");
  await page.waitForSelector("nav button", { timeout: 15_000 });
  await page.waitForTimeout(200);
  await navigateToView(page, "Devices");
  await page.waitForTimeout(150);
  const hasRows = (await page.locator(".dev-list").first().locator(".devrow:not(.this)").count()) > 0;
  const hasEmpty = (await page.locator(".empty").count()) > 0;
  expect(
    hasRows !== hasEmpty,
    `expected exactly one of {populated .devrow list, .empty state} with an empty peers fixture; got hasRows=${hasRows} hasEmpty=${hasEmpty}`,
  ).toBe(true);
  expect(hasEmpty, "empty fixture (?peersEmpty=1) — expected the EmptyState, not .devrow rows").toBe(true);
});

// History row density bounds — a row can be internally non-overflowing (the
// existing row-fit check passes) yet still be visually absurd: collapsed to
// illegibility or stretched unboundedly tall. Bounds derived from the row
// primitive's own contract: patterns.css's default --row-max is 74px; a
// multi-line/image row may override --row-max higher, so the ceiling is
// generous enough to cover that case without being meaningless.
const ROW_MIN_HEIGHT = 40;
const ROW_MAX_HEIGHT = 160;
for (const width of WIDTHS) {
  test(`composition: history row heights within sane bounds @ ${width}`, async ({ page }) => {
    await page.setViewportSize({ width, height: HEIGHT });
    await gotoMockApp(page);
    await page.waitForTimeout(150);
    const heights = await page.evaluate(() => {
      return Array.from(document.querySelectorAll(".row")).map((r) => {
        const rect = r.getBoundingClientRect();
        return {
          cls: (r.getAttribute("class") || "").slice(0, 40),
          height: rect.height,
          text: (r.textContent || "").trim().slice(0, 40),
        };
      });
    });
    const outOfBand = heights.filter(
      (h) => h.height < ROW_MIN_HEIGHT || h.height > ROW_MAX_HEIGHT,
    );
    expect(
      outOfBand.length,
      `History rows outside the [${ROW_MIN_HEIGHT}, ${ROW_MAX_HEIGHT}]px band @ ${width}px:\n` +
        outOfBand
          .map((h) => `  ${h.cls} height=${h.height.toFixed(1)} "${h.text}"`)
          .join("\n"),
    ).toBe(0);
  });
}

// Content max-width — the shared --content-max-width token (tokens.css,
// wired to Settings' .set-body by CopyPaste-7w060.7, and to Devices' .dev-scroll
// by CopyPaste-7w060.14) must actually cap the rendered content column on a
// wide desktop; an unbounded shell was one of the audited defect classes.
test("composition: settings content column respects --content-max-width @ 1440", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: HEIGHT });
  await gotoMockApp(page);
  await navigateToView(page, "Settings");
  await clickSettingsTab(page, "General");
  await page.waitForTimeout(150);
  const result = await page.evaluate(() => {
    const tokenRaw = getComputedStyle(document.documentElement)
      .getPropertyValue("--content-max-width")
      .trim();
    const body = document.querySelector(".set-body");
    const width = body ? body.getBoundingClientRect().width : null;
    return { tokenRaw, width };
  });
  expect(result.tokenRaw, "--content-max-width is not defined on :root").not.toBe("");
  const tokenPx = parseFloat(result.tokenRaw);
  expect(Number.isNaN(tokenPx), `--content-max-width value "${result.tokenRaw}" is not a parseable px length`).toBe(false);
  expect(result.width, ".set-body not found on the Settings surface").not.toBeNull();
  const TOL = 2;
  expect(
    result.width!,
    `Settings content column width=${result.width}px exceeds --content-max-width token (${tokenPx}px) at 1440px viewport`,
  ).toBeLessThanOrEqual(tokenPx + TOL);
});

// Devices view: same contract as Settings above — .dev-scroll must respect
// --content-max-width so device rows don't stretch full-pane-width and
// detach identity (left) from summary/actions (right) across an empty gap.
test("composition: devices content column respects --content-max-width @ 1440", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: HEIGHT });
  await gotoMockApp(page);
  await navigateToView(page, "Devices");
  await page.waitForTimeout(150);
  const result = await page.evaluate(() => {
    const tokenRaw = getComputedStyle(document.documentElement)
      .getPropertyValue("--content-max-width")
      .trim();
    const scroll = document.querySelector(".dev-scroll");
    const width = scroll ? scroll.getBoundingClientRect().width : null;
    return { tokenRaw, width };
  });
  expect(result.tokenRaw, "--content-max-width is not defined on :root").not.toBe("");
  const tokenPx = parseFloat(result.tokenRaw);
  expect(Number.isNaN(tokenPx), `--content-max-width value "${result.tokenRaw}" is not a parseable px length`).toBe(false);
  expect(result.width, ".dev-scroll not found on the Devices surface").not.toBeNull();
  const TOL = 2;
  expect(
    result.width!,
    `Devices content column width=${result.width}px exceeds --content-max-width token (${tokenPx}px) at 1440px viewport`,
  ).toBeLessThanOrEqual(tokenPx + TOL);
});

// Storage tab sliders/actions (CopyPaste-7w060.12): before the fix every
// .srow used the two-column space-between layout, which left the control
// (slider or Export/Import/Vacuum/Clear button) stranded far to the right of
// its label with a large dead zone in between. The fullWidth SettingsRow
// variant stacks title above control, so the two columns must now overlap
// horizontally (no side-by-side dead zone) with the control's top at or
// below the label's bottom.
test("composition: Storage tab sliders/actions sit near their labels (no far-right stranding)", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: HEIGHT });
  await gotoMockApp(page);
  await navigateToView(page, "Settings");
  await clickSettingsTab(page, "Storage");
  await page.waitForTimeout(150);
  const rows = await page.evaluate(() => {
    const pane = document.querySelector(".set-pane.on");
    if (!pane) return null;
    return Array.from(pane.querySelectorAll(".srow")).map((row) => {
      const label = row.querySelector(".srow__l");
      const ctl = row.querySelector(".srow__c");
      const lr = label ? label.getBoundingClientRect() : null;
      const cr = ctl ? ctl.getBoundingClientRect() : null;
      return {
        title: label ? (label.textContent || "").trim().slice(0, 40) : "?",
        lLeft: lr ? lr.left : null,
        lBottom: lr ? lr.bottom : null,
        cLeft: cr ? cr.left : null,
        cTop: cr ? cr.top : null,
      };
    });
  });
  expect(rows, ".set-pane.on not found on the Storage surface").not.toBeNull();
  expect(rows!.length, "no .srow rows found in the Storage pane").toBeGreaterThan(0);
  const TOL = 2;
  const offenders = rows!.filter((r) => {
    // "Database" is a read-only status line, intentionally left as a normal
    // two-column row (not a slider or destructive action) — see plan step
    // for CopyPaste-7w060.12.
    if (r.title === "Database") return false;
    if (r.lLeft === null || r.lBottom === null || r.cLeft === null || r.cTop === null) return false;
    // Stacked layout: control starts at/after the label's bottom edge, and
    // shares the label's left edge (no horizontal space-between gap).
    const stacked = r.cTop >= r.lBottom - TOL;
    const leftAligned = Math.abs(r.cLeft - r.lLeft) <= TOL;
    return !(stacked && leftAligned);
  });
  expect(
    offenders.length,
    `Storage rows still using the far-right two-column layout instead of stacked fullWidth:\n` +
      offenders.map((o) => `  "${o.title}" label=(${o.lLeft},${o.lBottom}) ctl=(${o.cLeft},${o.cTop})`).join("\n"),
  ).toBe(0);
});

// CopyPaste-7w060.11 — the originally-reported defect: Supabase URL/anon
// key/email/password/relay URL spanning the pane, Save/Save & test detached
// from their fields, and the passphrase eye-toggle/"Set passphrase" button
// drifting to the far right of an unconstrained row. CopyPaste-7w060.7
// capped .set-body at --content-max-width with a fixed label rail, which
// already bounds all of this — this test pins that contract to the Sync
// tab specifically so a regression here is caught by id.
test("composition: Sync tab credential fields and actions stay grouped (CopyPaste-7w060.11)", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: HEIGHT });
  await gotoMockApp(page);
  await navigateToView(page, "Settings");
  await clickSettingsTab(page, "Sync");
  await page.waitForTimeout(150);
  const result = await page.evaluate(() => {
    const pane = document.querySelector(".set-pane.on");
    if (!pane) return null;
    const panels = Array.from(pane.querySelectorAll(".set-grp"));
    // Cloud sync is the panel containing the Supabase URL field.
    const cloudPanel = panels.find((p) => p.querySelector('input[placeholder*="supabase.co"]')) ?? null;
    if (!cloudPanel) return { cloudPanelFound: false };
    const panelRect = cloudPanel.getBoundingClientRect();
    const fields = Array.from(cloudPanel.querySelectorAll(".field--grow-full input")) as HTMLInputElement[];
    const fieldRects = fields.map((f) => f.getBoundingClientRect().width);
    const saveBtn = Array.from(cloudPanel.querySelectorAll("button")).find((b) =>
      (b.textContent || "").includes("Save") && !(b.textContent || "").includes("test"),
    );
    const testBtn = Array.from(cloudPanel.querySelectorAll("button")).find((b) =>
      (b.textContent || "").includes("test"),
    );
    const passInput = cloudPanel.querySelector('input[placeholder="Shared passphrase…"]');
    const setPassBtn = Array.from(cloudPanel.querySelectorAll("button")).find((b) =>
      (b.textContent || "").includes("Set passphrase"),
    );
    return {
      cloudPanelFound: true,
      panelLeft: panelRect.left,
      panelRight: panelRect.right,
      fieldWidths: fieldRects,
      saveInPanel: saveBtn ? cloudPanel.contains(saveBtn) : false,
      testInPanel: testBtn ? cloudPanel.contains(testBtn) : false,
      passLeft: passInput ? passInput.getBoundingClientRect().left : null,
      setPassBtnLeft: setPassBtn ? setPassBtn.getBoundingClientRect().left : null,
    };
  });
  expect(result, ".set-pane.on not found on the Sync surface").not.toBeNull();
  expect(result!.cloudPanelFound, "Cloud sync panel (containing the Supabase URL field) not found").toBe(true);
  const tokenPx = 640;
  const panelWidth = result!.panelRight! - result!.panelLeft!;
  expect(
    panelWidth,
    `Cloud sync panel width=${panelWidth}px exceeds the --content-max-width contract (${tokenPx}px)`,
  ).toBeLessThanOrEqual(tokenPx + 2);
  expect(result!.saveInPanel, "Save button is not inside the Cloud sync panel it acts on").toBe(true);
  expect(result!.testInPanel, "Save & test connection button is not inside the Cloud sync panel it acts on").toBe(true);
  // The passphrase input and its "Set passphrase" button must sit on the same
  // row within a small horizontal span — not drifted across an unbounded window.
  const passGap = result!.setPassBtnLeft! - result!.passLeft!;
  expect(
    passGap,
    `"Set passphrase" button is ${passGap}px right of the passphrase input — expected them grouped on the same row`,
  ).toBeLessThan(400);
});

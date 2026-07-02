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

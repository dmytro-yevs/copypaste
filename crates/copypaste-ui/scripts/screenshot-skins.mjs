/**
 * Visual QA screenshot harness for CopyPaste skin redesign (CopyPaste-hidl).
 *
 * Captures all 3 skins × {light,dark} × 4 views = 24 PNGs.
 * Starts the Vite dev server internally with VITE_MOCK=1.
 *
 * Usage (from crates/copypaste-ui/):
 *   node scripts/screenshot-skins.mjs
 *
 * Output:  docs/ux/screenshots/<skin>-<theme>-<view>.png
 */

import { chromium } from "playwright";
import { spawn } from "child_process";
import { mkdirSync } from "fs";
import { resolve, dirname } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const UI_ROOT = resolve(__dirname, "..");
const REPO_ROOT = resolve(UI_ROOT, "../..");
const OUT_DIR = resolve(REPO_ROOT, "docs/ux/screenshots");
const PORT = 1420;
const BASE_URL = `http://localhost:${PORT}`;

const SKINS = ["classic", "quiet", "vapor"];
const THEMES = ["light", "dark"];
const VIEWS = [
  { id: "history",  label: "History" },
  { id: "devices",  label: "Devices" },
  { id: "settings", label: "Settings" },
  { id: "about",    label: "About" },
];

// ---------------------------------------------------------------------------
// Wait for the dev server to be ready (GET /)
// ---------------------------------------------------------------------------
async function waitForServer(url, timeoutMs = 60_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const res = await fetch(url);
      if (res.status < 500) return; // 200 or even 404 is "up"
    } catch {
      // not ready yet
    }
    await new Promise((r) => setTimeout(r, 400));
  }
  throw new Error(`Dev server did not become ready within ${timeoutMs / 1000}s`);
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------
async function main() {
  mkdirSync(OUT_DIR, { recursive: true });

  // ── Start dev server ───────────────────────────────────────────────────────
  console.log("[screenshot] Starting VITE_MOCK=1 dev server on :1420 …");
  const server = spawn("pnpm", ["dev"], {
    cwd: UI_ROOT,
    env: { ...process.env, VITE_MOCK: "1", FORCE_COLOR: "0" },
    stdio: ["ignore", "pipe", "pipe"],
    detached: false,
  });

  let serverStarted = false;
  server.stdout.on("data", (d) => {
    const s = d.toString();
    if (s.includes("1420")) serverStarted = true;
    process.stdout.write("[dev] " + s);
  });
  server.stderr.on("data", (d) => {
    process.stderr.write("[dev-err] " + d.toString());
  });

  // Give Vite a chance to emit its port line, then poll
  await new Promise((r) => setTimeout(r, 2000));

  try {
    await waitForServer(BASE_URL + "/", 60_000);
    console.log("[screenshot] Dev server is up.");
  } catch (e) {
    server.kill("SIGTERM");
    throw e;
  }

  // ── Launch Chromium ────────────────────────────────────────────────────────
  const browser = await chromium.launch({
    headless: true,
    args: ["--no-sandbox", "--disable-setuid-sandbox"],
  });

  const results = [];
  const errors = [];

  try {
    for (const skin of SKINS) {
      for (const theme of THEMES) {
        console.log(`\n[screenshot] ── skin=${skin}  theme=${theme} ──`);

        const context = await browser.newContext({
          viewport: { width: 1024, height: 768 },
          reducedMotion: "reduce",
        });
        const page = await context.newPage();
        page.on("console", (m) => {
          if (m.type() === "error") console.error("  [page-err]", m.text());
        });

        // ── Prime localStorage before React hydrates ─────────────────────────
        await page.goto(`${BASE_URL}/?mock=1`, { waitUntil: "domcontentloaded" });

        await page.evaluate(
          ({ skin, theme, PREFS_KEY }) => {
            const prefs = {
              skin,
              theme,
              translucency: false,
              motionReduced: true,
              palette: "graphite-mist",
              density: "compact",
              previewLinesApp: 2,
              previewLinesPopup: 1,
              previewSize: 28,
              maskSensitive: true,
              imageMaxHeight: 40,
              playSoundOnCopy: false,
              notifyOnCopy: false,
              historyDisplayLimit: 1000,
              showSensitiveWarnings: true,
            };
            localStorage.setItem(PREFS_KEY, JSON.stringify(prefs));
          },
          { skin, theme, PREFS_KEY: "copypaste-ui-prefs-v3" },
        );

        // ── Reload so the store picks up localStorage ─────────────────────────
        await page.reload({ waitUntil: "networkidle" });

        // Wait for React to settle (aurora animation + list entrance)
        await page.waitForTimeout(1200);

        // Confirm attributes applied
        const [appliedSkin, appliedTheme] = await page.evaluate(() => [
          document.documentElement.getAttribute("data-skin"),
          document.documentElement.getAttribute("data-theme"),
        ]);
        console.log(`  data-skin="${appliedSkin}"  data-theme="${appliedTheme}"`);
        if (appliedSkin !== skin || appliedTheme !== theme) {
          errors.push(`  [warn] skin=${skin} theme=${theme}: applied skin="${appliedSkin}" theme="${appliedTheme}"`);
        }

        // ── Screenshot each view ──────────────────────────────────────────────
        for (const view of VIEWS) {
          // Click the nav button by its visible text label
          try {
            // The sidebar nav items are <button> elements with a <span> text child
            await page.getByRole("button", { name: view.label, exact: true }).click();
          } catch {
            // Fallback: text content search
            const btns = page.locator("nav button, aside button");
            const count = await btns.count();
            let clicked = false;
            for (let i = 0; i < count; i++) {
              const text = (await btns.nth(i).textContent()) ?? "";
              if (text.trim().toLowerCase().includes(view.id)) {
                await btns.nth(i).click();
                clicked = true;
                break;
              }
            }
            if (!clicked) {
              const msg = `Could not click nav for view=${view.id} skin=${skin} theme=${theme}`;
              console.warn("  [warn]", msg);
              errors.push(msg);
            }
          }

          // Let the view transition settle
          await page.waitForTimeout(500);

          const filename = `${skin}-${theme}-${view.id}.png`;
          const outPath = resolve(OUT_DIR, filename);
          await page.screenshot({ path: outPath, fullPage: false });
          console.log(`  [ok] ${filename}`);
          results.push({ skin, theme, view: view.id, path: outPath });
        }

        await context.close();
      }
    }
  } finally {
    await browser.close();
    server.kill("SIGTERM");
    // Give the process a moment to die
    await new Promise((r) => setTimeout(r, 500));
  }

  // ── Summary ─────────────────────────────────────────────────────────────────
  console.log("\n[screenshot] ── Complete ──");
  console.log(`Screenshots saved to: ${OUT_DIR}`);
  for (const r of results) {
    console.log(`  ${r.skin}-${r.theme}-${r.view}.png`);
  }
  if (errors.length > 0) {
    console.log("\nWarnings/errors:");
    for (const e of errors) console.log(" ", e);
  }
  console.log(`\nTotal: ${results.length} PNGs`);
}

main().catch((e) => {
  console.error("[screenshot] Fatal:", e);
  process.exit(1);
});

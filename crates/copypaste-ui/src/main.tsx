/** INV-22 — the external bootstrap applies appearance before this module. */
// First, and a side effect: `@dnd-kit/dom` calls `replaceChildren` on the drag
// placeholder, and the Chromium 74 WebView baseline (vite.config.ts) predates
// it by twelve releases. Lowering syntax cannot add a method, so the pinned
// `lib` cannot see this one either — it is a DOM API, and `DOM` is one lib.
import "@ungap/replace-children";

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import {
  applyFlexGapSupportState,
  flexGapQaForcesUnsupported,
} from "@/lib/flexGapSupport";
import { atStartup, reportStartupFailure } from "@/startupFailure";
import "@/styles/index.css";

applyFlexGapSupportState(document, flexGapQaForcesUnsupported());

if (document.documentElement.dataset.themeBootstrapped !== "1") {
  console.warn("[copypaste] appearance bootstrap did not run before the app module");
}

const container = document.getElementById("root");
if (!container) throw new Error("missing #root");
// Rebound because the render below runs inside a closure, where the check
// above no longer narrows the type away from null.
const root: HTMLElement = container;

const isQuickPaste =
  import.meta.env.VITE_ANDROID_BUILD !== "1" &&
  new URLSearchParams(window.location.search).get("surface") === "quick-paste";

/**
 * The screens are imported here rather than above so that nothing they reach
 * is evaluated before the legacy polyfills are installed. `lib/format.ts`
 * constructs an `Intl.RelativeTimeFormat` while its module body runs, and on
 * API 24's Chromium 53 that throws before a component ever renders — a static
 * import would put it ahead of the only code that can supply it.
 *
 * `import.meta.env.LEGACY` is a marker plugin-legacy replaces per output, so the
 * branch leaves the module build — after chunking, which is why `vite.config.ts`
 * has to drop the chunk it left behind for modern engines to pay nothing.
 */
async function boot(): Promise<void> {
  if (import.meta.env.LEGACY) {
    await atStartup("polyfills", () => import("@/legacyPolyfills"));
  }

  const { initializePlatform } = await atStartup("platform", () =>
    import("@/lib/platform"),
  );
  await atStartup("platform-data", initializePlatform);

  const [{ default: App }, { AppProviders }] = await atStartup("screens", () =>
    Promise.all([import("@/app/App"), import("@/app/AppProviders")]),
  );
  const quickPaste = isQuickPaste
    ? await atStartup("screens", () => import("@/desktopQuickPaste")).then(
        ({ loadQuickPaste }) => loadQuickPaste(),
      )
    : null;
  const QuickPasteScreen = quickPaste?.QuickPasteScreen;

  await atStartup("render", () =>
    createRoot(root, {
      onUncaughtError: () => reportStartupFailure(root, { startupStage: "render" }),
    }).render(
      <StrictMode>
        <AppProviders>
          {QuickPasteScreen ? <QuickPasteScreen /> : <App />}
        </AppProviders>
      </StrictMode>,
    ),
  );
}

// Every way the app can fail to appear ends here, because a rejected import
// leaves `#root` empty and a blank window is not a diagnosis. `startupFailure`
// is statically imported so this path never fetches anything.
void boot().catch((failure: unknown) => {
  reportStartupFailure(root, failure);
});

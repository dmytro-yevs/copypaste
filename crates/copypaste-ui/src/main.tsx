/** INV-22 — the external bootstrap applies appearance before this module. */
// First, and a side effect: `@dnd-kit/dom` calls `replaceChildren` on the drag
// placeholder, and the Chromium 74 WebView baseline (vite.config.ts) predates
// it by twelve releases. Lowering syntax cannot add a method, so the pinned
// `lib` cannot see this one either — it is a DOM API, and `DOM` is one lib.
import "@ungap/replace-children";

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import { IpcFailure } from "@/lib/errors";
import { atStartup, reportStartupFailure } from "@/startupFailure";
import { isQuickPasteSurface } from "@/surface";
import "@/index.css";

if (document.documentElement.dataset.themeBootstrapped !== "1") {
  console.warn("[copypaste] appearance bootstrap did not run before the app module");
}

/**
 * INV-34 — `not_ready` is the one condition worth retrying: the service is up
 * but the store is not open yet. Every other kind propagates on the first
 * failure, because retrying arbitrary errors masks bugs — including
 * `unavailable`, which is structural and would never succeed on a retry.
 */
const RETRY_BACKOFF_MS = [250, 500, 1000, 2000, 2000];

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // INV-27: a hidden window polls zero times, and refreshes immediately on
      // becoming visible. React Query v5's focus manager is driven by
      // visibilitychange, so these two options are the whole gate.
      refetchIntervalInBackground: false,
      refetchOnWindowFocus: true,
      // INV-2: identical data must not produce a new array reference.
      structuralSharing: true,
      retry: (failureCount, error) =>
        error instanceof IpcFailure &&
        error.kind === "not_ready" &&
        failureCount < RETRY_BACKOFF_MS.length,
      retryDelay: (attempt) => RETRY_BACKOFF_MS[attempt] ?? 2000,
    },
    mutations: { retry: false },
  },
});

const container = document.getElementById("root");
if (!container) throw new Error("missing #root");
// Rebound because the render below runs inside a closure, where the check
// above no longer narrows the type away from null.
const root: HTMLElement = container;

const isQuickPaste = isQuickPasteSurface(window.location.search);

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

  const [{ default: App }, { QuickPasteApp }, { AppToaster }] = await atStartup("screens", () =>
    Promise.all([
      import("@/App"),
      import("@/components/quick-paste/QuickPasteApp"),
      import("@/components/shell/AppToaster"),
    ]),
  );

  await atStartup("render", () =>
    createRoot(root, {
      onUncaughtError: () => reportStartupFailure(root, { startupStage: "render" }),
    }).render(
      <StrictMode>
        <QueryClientProvider client={queryClient}>
          {isQuickPaste ? <QuickPasteApp /> : <App />}
          <AppToaster />
        </QueryClientProvider>
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

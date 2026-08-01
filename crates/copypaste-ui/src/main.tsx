/**
 * INV-22 — the persisted appearance is applied to `<html>` at module scope,
 * before `createRoot().render`, so there is no default-theme flash.
 */
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Toaster } from "sonner";

import App from "@/App";
import { QuickPasteApp } from "@/components/quick-paste/QuickPasteApp";
import { IpcFailure } from "@/lib/errors";
import { applyAppearance } from "@/lib/theme";
import { readPrefs } from "@/store/prefs";
import { isQuickPasteSurface } from "@/surface";
import "@/index.css";

applyAppearance(readPrefs());

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

const root = document.getElementById("root");
if (!root) throw new Error("missing #root");

const isQuickPaste = isQuickPasteSurface(window.location.search);

createRoot(root).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      {isQuickPaste ? <QuickPasteApp /> : <App />}
      {!isQuickPaste && (
        // One app-level stack. Bottom-right, not centred: centring bled into
        // the sidebar footer at narrow widths (manifest §3.7). Quick Paste is
        // its own compact surface and deliberately does not mount this shell.
        <Toaster
          position="bottom-right"
          closeButton
          duration={3000}
          offset="var(--s-4)"
          mobileOffset={{
            top: "calc(var(--inset-top) + var(--s-3))",
            right: "calc(var(--inset-right) + var(--s-3))",
            bottom:
              "calc(var(--tabbar-h) + var(--inset-bottom) + var(--s-3))",
            left: "calc(var(--inset-left) + var(--s-3))",
          }}
          toastOptions={{ className: "font-sans" }}
        />
      )}
    </QueryClientProvider>
  </StrictMode>,
);

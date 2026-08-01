/** INV-22 — the external bootstrap applies appearance before this module. */
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import App from "@/App";
import { QuickPasteApp } from "@/components/quick-paste/QuickPasteApp";
import { AppToaster } from "@/components/shell/AppToaster";
import { IpcFailure } from "@/lib/errors";
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

const root = document.getElementById("root");
if (!root) throw new Error("missing #root");

const isQuickPaste = isQuickPasteSurface(window.location.search);

createRoot(root).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      {isQuickPaste ? <QuickPasteApp /> : <App />}
      <AppToaster />
    </QueryClientProvider>
  </StrictMode>,
);

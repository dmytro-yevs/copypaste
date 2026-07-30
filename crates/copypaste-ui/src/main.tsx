/**
 * Bootstrap: appearance before first paint, then the query client, then React.
 *
 * INV-22 — the persisted appearance is on `<html>` **before** anything is
 * painted, so there is no default-theme flash. The two statements below run at
 * module scope, before `createRoot().render`, with an empty `<body>`; v1 needed
 * a separate dependency-free pre-paint script because its prefs schema lived
 * inside the app bundle, and AT-54's whole risk was that second copy drifting
 * from the first. There is one schema here.
 */
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Toaster } from "sonner";

import App from "@/App";
import { IpcFailure } from "@/lib/errors";
import { applyAppearance } from "@/lib/theme";
import { readPrefs } from "@/store/prefs";
import "@/index.css";

applyAppearance(readPrefs());

/**
 * INV-34 — retry policy. v1 retried `migration_in_progress` on the ladder below
 * and *nothing else*, because retrying arbitrary errors masks bugs. v2 has no
 * migrations (CLAUDE.md rule 3), so the transient startup condition is
 * `not_ready`: the service is up but the store is not open yet. Every other
 * kind propagates on the first failure, exactly as before — including
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

createRoot(root).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <App />
      {/* Manifest §3.7: one app-level stack, bottom-right — centring bled into
          the sidebar footer at narrow widths — pausing on hover and on focus
          within, and below any open dialog in the z-order. */}
      <Toaster
        position="bottom-right"
        closeButton
        duration={3000}
        toastOptions={{ className: "font-sans" }}
      />
    </QueryClientProvider>
  </StrictMode>,
);

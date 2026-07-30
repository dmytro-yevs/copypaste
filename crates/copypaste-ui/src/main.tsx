import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Toaster } from "sonner";

import App from "./App";
import { IpcFailure } from "./lib/errors";
import "./index.css";

/**
 * INV-22 — the persisted appearance must be on `<html>` before first paint, so
 * there is no default-theme flash. v1 needed a separate pre-paint script
 * because it had a persisted preference to read; v2 has no settings screen, so
 * the system preference is the whole answer and this module — which runs
 * before React renders anything into an empty body — is early enough.
 *
 * The tokens define dark at `:root` and light at `:root[data-theme="light"]`,
 * so the attribute is the only thing that switches themes. No colour is
 * written here or anywhere else in the app.
 */
const lightMedia = window.matchMedia("(prefers-color-scheme: light)");
function applyTheme() {
  const theme = lightMedia.matches ? "light" : "dark";
  document.documentElement.dataset.theme = theme;
  document.documentElement.dataset.themePref = "system";
}
applyTheme();
lightMedia.addEventListener("change", applyTheme);

/**
 * INV-34 — retry policy. v1 retried `migration_in_progress` on the ladder
 * below and *nothing else*, because retrying arbitrary errors masks bugs. v2
 * has no migrations (CLAUDE.md rule 3), so the transient startup condition is
 * `not_ready`: the service is up but the store is not open yet. Every other
 * code propagates on the first failure, exactly as before.
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
          the footer at narrow widths — pausing on hover and on focus within. */}
      <Toaster
        position="bottom-right"
        closeButton
        duration={3000}
        toastOptions={{ className: "font-sans" }}
      />
    </QueryClientProvider>
  </StrictMode>,
);

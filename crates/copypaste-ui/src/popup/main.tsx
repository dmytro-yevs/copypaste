import React from "react";
import ReactDOM from "react-dom/client";
import "../styles/index.css";
import { ErrorBoundary } from "../components/ErrorBoundary";
import { Popup } from "./Popup";
import { assertBootstrapRanBeforeModule } from "../lib/theme/assertBootstrap";

// Ordering check (design.md task 1.15) — the popup window must also apply the
// persisted appearance before first paint via its own theme-bootstrap.js include.
assertBootstrapRanBeforeModule("popup");

const rootEl = document.getElementById("popup-root");
if (rootEl) {
  // Perf instrumentation (design.md Decision 15 / task 1.18): mark popup mount so
  // the slice-6 harness can measure open→first-render latency (p50/p95 over 10
  // warm runs) against the recorded baseline. Guarded — `performance` may be
  // absent in some test contexts.
  performance?.mark?.("popup-mount-start");
  ReactDOM.createRoot(rootEl).render(
    <React.StrictMode>
      {/* The popup is the global-shortcut quick-paste surface; a render or
          effect throw here would blank it with no recourse. Wrap it so a crash
          shows the readable fallback instead. */}
      <ErrorBoundary label="Quick paste">
        <Popup />
      </ErrorBoundary>
    </React.StrictMode>
  );
  requestAnimationFrame(() => {
    performance?.mark?.("popup-first-render");
    try {
      performance?.measure?.(
        "popup-open-to-render",
        "popup-mount-start",
        "popup-first-render",
      );
    } catch {
      /* marks may be missing under reduced-instrumentation contexts */
    }
  });
}

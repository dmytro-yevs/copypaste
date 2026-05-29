import React from "react";
import ReactDOM from "react-dom/client";
import "../index.css";
import { ErrorBoundary } from "../components/ErrorBoundary";
import { Popup } from "./Popup";

const rootEl = document.getElementById("popup-root");
if (rootEl) {
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
}

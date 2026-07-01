import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles/index.css";
import { assertBootstrapRanBeforeModule } from "./lib/theme/assertBootstrap";

// Ordering check (design.md task 1.15): the pre-paint theme-bootstrap.js sets
// dataset.themeBootstrapped="1" synchronously before this module runs. Capture
// it at module-eval time — BEFORE React mounts — so we assert script order, not
// post-paint state. Packaged-Tauri smoke (Slice 6) turns this into a hard gate.
assertBootstrapRanBeforeModule("main");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);

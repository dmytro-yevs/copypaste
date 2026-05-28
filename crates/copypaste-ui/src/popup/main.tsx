import React from "react";
import ReactDOM from "react-dom/client";
import "../index.css";
import { Popup } from "./Popup";

const rootEl = document.getElementById("popup-root");
if (rootEl) {
  ReactDOM.createRoot(rootEl).render(
    <React.StrictMode>
      <Popup />
    </React.StrictMode>
  );
}

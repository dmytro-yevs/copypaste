import type { ComponentType } from "react";
import { useUI, type ViewId } from "./store";
import { Sidebar } from "./components/Sidebar";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { HistoryView } from "./views/HistoryView";
import { DevicesView } from "./views/DevicesView";
import { SettingsView } from "./views/SettingsView";
import { AboutView } from "./views/AboutView";

const VIEWS: Record<ViewId, { Component: ComponentType; label: string }> = {
  history: { Component: HistoryView, label: "History" },
  devices: { Component: DevicesView, label: "Devices" },
  settings: { Component: SettingsView, label: "Settings" },
  about: { Component: AboutView, label: "About" }
};

export default function App() {
  const view = useUI((s) => s.view);
  const { Component: View, label } = VIEWS[view];
  return (
    // Outer boundary is the last line of defence: even if the chrome (Sidebar)
    // itself throws, the window shows a fallback instead of going blank.
    <ErrorBoundary>
      <div className="flex h-screen w-screen overflow-hidden text-ide-text">
        <Sidebar />
        <div className="flex min-w-0 flex-1 flex-col bg-ide-bg/35">
          <div data-tauri-drag-region className="h-9 shrink-0" />
          <main className="min-h-0 flex-1 overflow-hidden">
            {/* Per-view boundary keyed on the view id: a crash in one screen
                stays contained, and navigating away then back (new key)
                remounts a fresh, non-crashed subtree. */}
            <ErrorBoundary key={view} label={label}>
              <View />
            </ErrorBoundary>
          </main>
        </div>
      </div>
    </ErrorBoundary>
  );
}

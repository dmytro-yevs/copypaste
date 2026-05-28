import type { ComponentType } from "react";
import { useUI, type ViewId } from "./store";
import { Sidebar } from "./components/Sidebar";
import { HistoryView } from "./views/HistoryView";
import { DevicesView } from "./views/DevicesView";
import { SettingsView } from "./views/SettingsView";
import { AboutView } from "./views/AboutView";

const VIEWS: Record<ViewId, ComponentType> = {
  history: HistoryView,
  devices: DevicesView,
  settings: SettingsView,
  about: AboutView
};

export default function App() {
  const view = useUI((s) => s.view);
  const View = VIEWS[view];
  return (
    <div className="flex h-screen w-screen overflow-hidden text-ide-text">
      <Sidebar />
      <div className="flex min-w-0 flex-1 flex-col bg-ide-bg/35">
        <div data-tauri-drag-region className="h-9 shrink-0" />
        <main className="min-h-0 flex-1 overflow-hidden">
          <View />
        </main>
      </div>
    </div>
  );
}

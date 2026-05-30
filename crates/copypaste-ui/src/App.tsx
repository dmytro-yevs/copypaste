import { useEffect, useState, type ComponentType } from "react";
import { useUI, type ViewId } from "./store";
import { Sidebar } from "./components/Sidebar";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { RestartDaemonButton } from "./components/RestartDaemonButton";
import { appVersion, detectStaleDaemonFromStatus, api } from "./lib/ipc";
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

  // App-launch stale check: if an OLD daemon survived an upgrade and is still
  // serving old code, show a single dismissible banner offering a restart.
  // Uses detectStaleDaemonFromStatus (strictly OLDER semver only) so a newer
  // daemon after a rollback does not trigger the "restart" banner.
  // Minimal + non-annoying: we never auto-kill the daemon, and the banner is
  // dismissible. Best-effort — any error yields no banner.
  const [staleDaemon, setStaleDaemon] = useState<string | null>(null);
  const [dismissed, setDismissed] = useState(false);
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const [myVer, status] = await Promise.all([
          appVersion().catch(() => null),
          api.status().catch(() => null),
        ]);
        if (cancelled) return;
        if (myVer !== null) {
          setStaleDaemon(detectStaleDaemonFromStatus(status, myVer));
        }
      } catch {
        // Best-effort — never show a banner on error.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);
  const showStaleBanner = staleDaemon !== null && !dismissed;

  return (
    // Outer boundary is the last line of defence: even if the chrome (Sidebar)
    // itself throws, the window shows a fallback instead of going blank.
    <ErrorBoundary>
      <div className="flex h-screen w-screen overflow-hidden text-ide-text">
        <Sidebar />
        <div className="flex min-w-0 flex-1 flex-col bg-ide-bg/35">
          <div data-tauri-drag-region className="h-9 shrink-0" />
          {showStaleBanner && (
            <div className="mx-3 mb-2 flex items-start justify-between gap-3 rounded-ide border border-ide-warning/40 bg-ide-warning/5 px-3 py-2 text-[13px] text-ide-warning">
              <span>
                CopyPaste was updated but an older background daemon is still
                running
                {staleDaemon !== "unknown" ? ` (build ${staleDaemon})` : ""}.
                Restart it to use the new version.
              </span>
              <div className="flex shrink-0 items-center gap-2">
                <RestartDaemonButton
                  onRestarted={() => {
                    setStaleDaemon(null);
                    setDismissed(true);
                  }}
                />
                <button
                  type="button"
                  onClick={() => setDismissed(true)}
                  className="rounded-ide border border-ide-border bg-ide-panel px-2.5 py-1 text-[12px] text-ide-text hover:bg-ide-hover"
                >
                  Dismiss
                </button>
              </div>
            </div>
          )}
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

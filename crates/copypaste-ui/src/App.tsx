import { useEffect, useState, type ComponentType } from "react";
import { useUI, type ViewId } from "./store";
import { Sidebar } from "./components/Sidebar";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { RestartDaemonButton } from "./components/RestartDaemonButton";
import { appVersion, detectStaleDaemonFromStatus, api, checkAccessibilityPermission, requestAccessibilityPermission, getDaemonError, type PairSasStatus } from "./lib/ipc";
import { listen } from "@tauri-apps/api/event";
import { startPeerPresencePolling, stopPeerPresencePolling } from "./lib/peerPresence";
import { HistoryView } from "./views/HistoryView";
import { DevicesView } from "./views/DevicesView";
import { SettingsView } from "./views/SettingsView";
import { AboutView } from "./views/AboutView";
import { LogView } from "./views/LogView";

// Views that take no extra props — routed generically via ComponentType.
// DevicesView is rendered separately below so it can receive `incomingPairing`.
const VIEWS: Record<ViewId, { Component: ComponentType; label: string }> = {
  history: { Component: HistoryView, label: "History" },
  devices: { Component: DevicesView, label: "Devices" },
  settings: { Component: SettingsView, label: "Settings" },
  about: { Component: AboutView, label: "About" },
  logs: { Component: LogView, label: "Logs" },
};

export default function App() {
  const view = useUI((s) => s.view);
  const setView = useUI((s) => s.setView);
  const translucency = useUI((s) => s.prefs.translucency);
  const theme = useUI((s) => s.prefs.theme);
  const { Component: View, label } = VIEWS[view];

  // The popup window emits "open-settings" (after showing this main window) when
  // the user clicks its footer gear. Navigate to the Settings view in response.
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;
    void listen("open-settings", () => {
      if (!cancelled) setView("settings");
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    }).catch(() => {
      // Best-effort — popup gear is a convenience, not critical path.
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [setView]);

  // ---------------------------------------------------------------------------
  // Incoming pairing (responder side) — always-on background detection
  // ---------------------------------------------------------------------------
  // The Tauri backend polls `pair_get_sas` every ~1 s and emits
  // `"incoming-pairing"` when it observes state="awaiting_sas"+role="responder".
  // We switch to the Devices tab and pass the payload to DevicesView so the SAS
  // modal opens regardless of which tab was active when the request arrived.
  const [incomingPairing, setIncomingPairing] = useState<PairSasStatus | null>(null);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;
    void listen<PairSasStatus>("incoming-pairing", (event) => {
      if (cancelled) return;
      const payload = event.payload;
      // Only act on responder+awaiting_sas payloads (belt-and-suspenders guard;
      // the Rust poller already filters, but defensive check here too).
      if (payload.state === "awaiting_sas" && payload.role === "responder") {
        setIncomingPairing(payload);
        setView("devices");
      }
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    }).catch(() => {
      // Best-effort — not critical path; the in-component poll in DevicesView
      // serves as a fallback when the user is already on the Devices tab.
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [setView]);

  // ---------------------------------------------------------------------------
  // Live peer-presence polling (app-global, always-on)
  // ---------------------------------------------------------------------------
  // Polls `poll_peer_events` every ~1 s so the DevicesView online dots update
  // in real time without requiring the user to open the Devices page.
  // The polling is intentionally app-global (not per-view) so presence state
  // persists across tab switches and the sidebar badge (if added later) works.
  useEffect(() => {
    startPeerPresencePolling();
    return () => { stopPeerPresencePolling(); };
  }, []);

  // Apply/remove the no-translucency root class whenever the pref changes.
  // When translucency is OFF we add "no-translucency" on <html> so the CSS
  // can key off it to swap transparent/blur surfaces for solid ones.
  useEffect(() => {
    if (translucency) {
      document.documentElement.classList.remove("no-translucency");
    } else {
      document.documentElement.classList.add("no-translucency");
    }
  }, [translucency]);

  // Apply the data-theme attribute whenever the pref changes.
  // CSS custom property overrides in :root[data-theme="light"] take effect
  // immediately; no JS class toggling needed beyond setting this one attribute.
  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme ?? "light");
  }, [theme]);

  // ---------------------------------------------------------------------------
  // Daemon spawn error banner (non-dismissible, installation-incomplete)
  // ---------------------------------------------------------------------------
  // On mount: call getDaemonError() for the case where the app launched,
  // tried to start the daemon, failed, and we loaded after the event fired.
  // Also listen for the real-time "daemon-spawn-result" event for the case
  // where the app is still starting when this component mounts.
  const [daemonError, setDaemonError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    // Fallback: read whatever error was stored by ensure_daemon_running_async.
    void getDaemonError().then((err) => {
      if (!cancelled && err) setDaemonError(err);
    }).catch(() => {
      // Best-effort — never block on this.
    });

    // Real-time: listen for the daemon-spawn-result Tauri event so we show
    // the banner immediately if the daemon fails while the UI is already open.
    let unlisten: (() => void) | null = null;
    void listen<{ ok: boolean; error?: string }>("daemon-spawn-result", (event) => {
      if (cancelled) return;
      if (!event.payload.ok && event.payload.error) {
        setDaemonError(event.payload.error);
      } else if (event.payload.ok) {
        // Daemon started successfully — clear any stale error.
        setDaemonError(null);
      }
    }).then((fn) => {
      unlisten = fn;
    }).catch(() => {
      // Best-effort.
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  // ---------------------------------------------------------------------------
  // Stale-daemon banner (dismissible)
  // ---------------------------------------------------------------------------
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

  // ---------------------------------------------------------------------------
  // Accessibility permission banner (macOS only)
  // ---------------------------------------------------------------------------
  // We check once on mount; the Tauri command always returns `true` on
  // non-macOS so the banner never appears there.  After the user opens
  // System Settings and grants the permission we re-check every 3 s until
  // it's granted (or they dismiss the banner).
  const [axGranted, setAxGranted] = useState<boolean>(true); // assume OK until checked
  const [axDismissed, setAxDismissed] = useState(false);

  useEffect(() => {
    let cancelled = false;

    const check = async () => {
      try {
        const granted = await checkAccessibilityPermission();
        if (!cancelled) setAxGranted(granted);
      } catch {
        // Best-effort — never block startup on this check.
      }
    };

    void check();

    // Poll every 3 s so the banner disappears automatically once the user
    // grants the permission in System Settings (without needing an app restart).
    const interval = setInterval(() => { void check(); }, 3000);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, []);

  const showAxBanner = !axGranted && !axDismissed;

  const handleOpenAxSettings = async () => {
    try {
      await requestAccessibilityPermission();
    } catch {
      // Fire-and-forget — opening System Settings can't really fail in a
      // meaningful way; if it does, the user can navigate there manually.
    }
  };

  return (
    // Outer boundary is the last line of defence: even if the chrome (Sidebar)
    // itself throws, the window shows a fallback instead of going blank.
    <ErrorBoundary>
      <div className="flex h-screen w-screen overflow-hidden text-ide-text">
        <Sidebar />
        {/* surface-glass = canonical §3 translucency recipe (rgba(19,20,26,.72)+blur(30px) saturate(180%)) */}
        <div
          className="surface-glass flex min-w-0 flex-1 flex-col"
        >
          <div data-tauri-drag-region className="h-9 shrink-0" />

          {/* Daemon spawn error — non-dismissible, installation-incomplete */}
          {daemonError !== null && (
            <div className="mx-3 mb-2 flex items-start gap-3 rounded-ide border border-red-500/40 bg-red-500/5 px-3 py-2 text-[13px] text-red-400">
              <span className="shrink-0 font-semibold">Background service error:</span>
              <span>{daemonError}</span>
            </div>
          )}

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

          {/* Accessibility permission banner — macOS only, dismissed once granted */}
          {showAxBanner && (
            <div className="mx-3 mb-2 flex items-start justify-between gap-3 rounded-ide border border-ide-warning/40 bg-ide-warning/5 px-3 py-2 text-[13px] text-ide-warning">
              <span>
                Accessibility permission is required for the global paste shortcut
                and hotkey capture. Grant it in System Settings to enable these
                features.
              </span>
              <div className="flex shrink-0 items-center gap-2">
                <button
                  type="button"
                  onClick={() => { void handleOpenAxSettings(); }}
                  className="rounded-ide border border-ide-warning/50 bg-ide-elevated px-2.5 py-1 text-[12px] text-ide-warning hover:bg-ide-hover"
                >
                  Open Settings
                </button>
                <button
                  type="button"
                  onClick={() => setAxDismissed(true)}
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
              {view === "devices" ? (
                // DevicesView gets `incomingPairing` so the SAS modal opens
                // even when the user wasn't on the Devices tab when the request
                // arrived.  We clear it once DevicesView mounts (the prop is
                // stable for that render cycle; DevicesView owns the modal
                // lifetime after that).
                <DevicesView incomingPairing={incomingPairing} />
              ) : (
                <View />
              )}
            </ErrorBoundary>
          </main>
        </div>
      </div>
    </ErrorBoundary>
  );
}

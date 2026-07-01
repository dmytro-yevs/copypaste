import { useEffect, useState, type ComponentType, type ReactNode } from "react";
import { useUI, type ViewId } from "./store";
import { applyAppearanceToRoot } from "./lib/theme/applyTheme";
import { Sidebar } from "./components/Sidebar";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { RestartDaemonButton } from "./components/RestartDaemonButton";
import { appVersion, detectStaleDaemonFromStatus, api, checkAccessibilityPermission, requestAccessibilityPermission, getDaemonError, setProtocolMismatchHandler, type PairSasStatus } from "./lib/ipc";
import { AccessibilityBanner } from "./components/AccessibilityBanner";
import { listen } from "@tauri-apps/api/event";
import { startPeerPresencePolling, stopPeerPresencePolling } from "./lib/peerPresence";
import { HistoryView } from "./views/HistoryView";
import { DevicesView } from "./views/DevicesView";
import { SettingsView } from "./views/SettingsView";
import { AboutView } from "./views/AboutView";
import { LogView } from "./views/LogView";

// audit P1-7: the Tauri event plugin is only present inside the Tauri runtime.
// In a plain browser / ?mock=1 harness, listen() rejects and logs a console
// error on every mount. Feature-detect the runtime (same gate History/Settings
// use) and skip the subscriptions in the browser — nothing emits those events
// there anyway.
const HAS_TAURI =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

// ---------------------------------------------------------------------------
// ViewTransitionWrapper — plain wrapper preserving data-testid (CopyPaste-2bhh / 4r48)
// ---------------------------------------------------------------------------
// Animation stripped in design-demolition pass (CopyPaste-h1n3). The wrapper
// remains so tests can find data-testid="view-transition" and so routing works.
export function ViewTransitionWrapper({
  viewKey: _viewKey,
  children,
}: {
  viewKey: string;
  children: ReactNode;
}) {
  return (
    <div data-testid="view-transition">
      {children}
    </div>
  );
}

// ---------------------------------------------------------------------------
// CrossfadeContainer — plain wrapper (animation stripped in CopyPaste-h1n3)
// ---------------------------------------------------------------------------
function CrossfadeContainer({
  viewKey,
  children,
}: {
  viewKey: string;
  children: ReactNode;
}) {
  return (
    <ViewTransitionWrapper viewKey={viewKey}>
      {children}
    </ViewTransitionWrapper>
  );
}

// Views that take no extra props — routed generically via ComponentType.
// DevicesView is rendered separately below so it can receive `incomingPairing`.
const VIEWS: Record<ViewId, { Component: ComponentType; label: string }> = {
  history: { Component: HistoryView, label: "History" },
  devices: { Component: DevicesView, label: "Devices" },
  settings: { Component: SettingsView, label: "Settings" },
  about: { Component: AboutView, label: "About" },
  logs: { Component: LogView, label: "Logs" },
};

// Dev-only component gallery activation (design.md Decision 6). NOT a production
// ViewId and NOT in the store — a `?view=gallery` URL check, gated by
// import.meta.env.DEV so Vite dead-code-eliminates the dynamic import (and the
// gallery chunk) from production builds. Open it at `?mock=1&view=gallery`.
function galleryActive(): boolean {
  if (!import.meta.env.DEV || typeof window === "undefined") return false;
  return new URLSearchParams(window.location.search).get("view") === "gallery";
}

export default function App() {
  const view = useUI((s) => s.view);
  const setView = useUI((s) => s.setView);
  const { Component: View, label } = VIEWS[view];

  // Live appearance sync (task 1.16). The pre-paint bootstrap owns FIRST paint;
  // this re-applies theme/accent/translucency to <html> on mount and whenever a
  // Settings change updates prefs, so the running window updates without reload.
  const theme = useUI((s) => s.prefs.theme);
  const accent = useUI((s) => s.prefs.accent);
  const translucency = useUI((s) => s.prefs.translucency);
  useEffect(() => {
    applyAppearanceToRoot(document.documentElement, { theme, accent, translucency });
  }, [theme, accent, translucency]);

  // Dev-only gallery branch (design.md Decision 6). Dynamic-imported behind the
  // DEV gate so it is tree-shaken from production (mirrors transport.ts's mockIpc
  // import). The gallery lives OUTSIDE the production view registry above.
  const galleryOn = galleryActive();
  const [GalleryComp, setGalleryComp] = useState<ComponentType | null>(null);
  useEffect(() => {
    if (import.meta.env.DEV && galleryOn && GalleryComp === null) {
      void import("./views/GalleryView").then((m) => setGalleryComp(() => m.GalleryView));
    }
  }, [galleryOn, GalleryComp]);

  // The popup window emits "open-settings" (after showing this main window) when
  // the user clicks its footer gear. Navigate to the Settings view in response.
  useEffect(() => {
    if (!HAS_TAURI) return;
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
  const [incomingPairing, setIncomingPairing] = useState<PairSasStatus | null>(null);

  useEffect(() => {
    if (!HAS_TAURI) return;
    let cancelled = false;
    let unlisten: (() => void) | null = null;
    void listen<PairSasStatus>("incoming-pairing", (event) => {
      if (cancelled) return;
      const payload = event.payload;
      if (payload.state === "awaiting_sas" && payload.role === "responder") {
        setIncomingPairing(payload);
        setView("devices");
      }
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    }).catch(() => {
      // Best-effort.
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [setView]);

  // ---------------------------------------------------------------------------
  // Live peer-presence polling (app-global, always-on)
  // ---------------------------------------------------------------------------
  useEffect(() => {
    startPeerPresencePolling();
    return () => { stopPeerPresencePolling(); };
  }, []);

  // ---------------------------------------------------------------------------
  // Protocol-version mismatch banner (dismissible)
  // ---------------------------------------------------------------------------
  const [protocolMismatch, setProtocolMismatch] = useState<number | null>(null);
  const [mismatchDismissed, setMismatchDismissed] = useState(false);

  useEffect(() => {
    setProtocolMismatchHandler((daemonVersion) => {
      setProtocolMismatch(daemonVersion);
    });
    return () => { setProtocolMismatchHandler(null); };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const showMismatchBanner = protocolMismatch !== null && !mismatchDismissed;

  // ---------------------------------------------------------------------------
  // Daemon spawn error banner (non-dismissible, installation-incomplete)
  // ---------------------------------------------------------------------------
  const [daemonError, setDaemonError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    void getDaemonError().then((err) => {
      if (!cancelled && err) {
        // eslint-disable-next-line no-console
        console.error("[CopyPaste] daemon spawn error:", err);
        setDaemonError(err);
      }
    }).catch(() => {});

    let unlisten: (() => void) | null = null;
    if (HAS_TAURI) {
      void listen<{ ok: boolean; error?: string }>("daemon-spawn-result", (event) => {
        if (cancelled) return;
        if (!event.payload.ok && event.payload.error) {
          // eslint-disable-next-line no-console
          console.error("[CopyPaste] daemon-spawn-result error:", event.payload.error);
          setDaemonError(event.payload.error);
        } else if (event.payload.ok) {
          setDaemonError(null);
        }
      }).then((fn) => {
        unlisten = fn;
      }).catch(() => {});
    }

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  // ---------------------------------------------------------------------------
  // Stale-daemon banner (dismissible)
  // ---------------------------------------------------------------------------
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
        // Best-effort.
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
  const [axGranted, setAxGranted] = useState<boolean>(true);
  const [axDismissed, setAxDismissed] = useState(false);

  useEffect(() => {
    if (axGranted || axDismissed) return;

    let cancelled = false;
    let interval: ReturnType<typeof setInterval> | null = null;

    const check = async () => {
      try {
        const granted = await checkAccessibilityPermission();
        if (cancelled) return;
        setAxGranted(granted);
        if (granted && interval !== null) {
          clearInterval(interval);
          interval = null;
        }
      } catch {
        // Best-effort.
      }
    };

    void check();
    interval = setInterval(() => { void check(); }, 3000);
    return () => {
      cancelled = true;
      if (interval !== null) clearInterval(interval);
    };
  }, [axGranted, axDismissed]);

  const handleOpenAxSettings = async () => {
    try {
      await requestAccessibilityPermission();
    } catch {
      // Fire-and-forget.
    }
  };

  // Dev-only: render the gallery instead of the app when ?view=gallery is set.
  // Guarded by import.meta.env.DEV so this whole branch (and the dynamic import
  // above) is eliminated from production bundles.
  if (import.meta.env.DEV && galleryOn) {
    return GalleryComp ? <GalleryComp /> : <div className="empty">Loading gallery…</div>;
  }

  return (
    <ErrorBoundary>
      <div>
        <Sidebar />
        <div>
          {daemonError !== null && (
            <div>
              <span>Background service error:</span>
              <span>The background service failed to start. Please reinstall CopyPaste or restart your Mac.</span>
            </div>
          )}

          {showMismatchBanner && (
            <div data-testid="protocol-mismatch-banner">
              <span>
                CopyPaste app and background service are on incompatible versions
                {protocolMismatch !== null ? ` (service protocol v${protocolMismatch})` : ""}.
                Restart the app or the background service to resolve.
              </span>
              <button
                type="button"
                onClick={() => setMismatchDismissed(true)}
              >
                Dismiss
              </button>
            </div>
          )}

          {showStaleBanner && (
            <div>
              <span>
                CopyPaste was updated but an older background service is still
                running
                {staleDaemon !== "unknown" ? ` (build ${staleDaemon})` : ""}.
                Restart it to use the new version.
              </span>
              <div>
                <RestartDaemonButton
                  onRestarted={() => {
                    setStaleDaemon(null);
                    setDismissed(true);
                  }}
                />
                <button
                  type="button"
                  onClick={() => setDismissed(true)}
                >
                  Dismiss
                </button>
              </div>
            </div>
          )}

          <AccessibilityBanner
            axGranted={axGranted}
            axDismissed={axDismissed}
            onDismiss={() => setAxDismissed(true)}
            onOpenSettings={() => { void handleOpenAxSettings(); }}
          />

          <main>
            <CrossfadeContainer viewKey={view}>
              <ErrorBoundary label={label}>
                {view === "devices" ? (
                  <DevicesView incomingPairing={incomingPairing} />
                ) : (
                  <View />
                )}
              </ErrorBoundary>
            </CrossfadeContainer>
          </main>
        </div>
      </div>
    </ErrorBoundary>
  );
}

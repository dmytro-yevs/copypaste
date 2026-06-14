import { useEffect, useState, type ComponentType, type ReactNode } from "react";
import { useUI, type ViewId } from "./store";
import { Sidebar } from "./components/Sidebar";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { RestartDaemonButton } from "./components/RestartDaemonButton";
import styles from "./ViewTransition.module.css";
import { appVersion, detectStaleDaemonFromStatus, api, checkAccessibilityPermission, requestAccessibilityPermission, getDaemonError, type PairSasStatus } from "./lib/ipc";
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
// ViewTransitionWrapper — smooth crossfade when the active view changes
// (CopyPaste-2bhh)
// ---------------------------------------------------------------------------
// Wraps each view with a gentle opacity+translate entrance so that switching
// sidebar tabs feels Apple-like (~180ms cubic-bezier spring) rather than the
// harsh immediate-remount snap.  Keyed by `viewKey` in App's <main> so React
// unmounts the old wrapper and mounts a new one on every tab change, triggering
// the animation.  prefers-reduced-motion: the animation duration collapses to 0ms
// so no motion occurs.  card-in / reveal-up on ViewShell continue to fire on
// mount (intentional — they are the semantic per-element entrance), but they're
// now visually dominated by the outer crossfade which sets the overall pace.
export function ViewTransitionWrapper({
  viewKey: _viewKey,
  children,
}: {
  viewKey: string;
  children: ReactNode;
}) {
  // Detect reduced-motion preference at render time (synchronous — avoids a
  // flash of animated content before a useEffect could suppress it).
  const prefersReduced =
    typeof window !== "undefined" &&
    window.matchMedia != null &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  return (
    <div
      data-testid="view-transition"
      className={["h-full", !prefersReduced ? styles["view-fade-in"] : ""].join(" ").trim()}
      style={{
        // Inline timing/fill so jsdom tests can assert the values without
        // needing to parse CSS stylesheets.
        animationDuration: prefersReduced ? "0ms" : "180ms",
        animationFillMode: "forwards",
      }}
    >
      {children}
    </div>
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

export default function App() {
  const view = useUI((s) => s.view);
  const setView = useUI((s) => s.setView);
  const translucency = useUI((s) => s.prefs.translucency);
  const theme = useUI((s) => s.prefs.theme);
  const palette = useUI((s) => s.prefs.palette);
  const density = useUI((s) => s.prefs.density);
  const { Component: View, label } = VIEWS[view];

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
  // The Tauri backend polls `pair_get_sas` every ~1 s and emits
  // `"incoming-pairing"` when it observes state="awaiting_sas"+role="responder".
  // We switch to the Devices tab and pass the payload to DevicesView so the SAS
  // modal opens regardless of which tab was active when the request arrived.
  const [incomingPairing, setIncomingPairing] = useState<PairSasStatus | null>(null);

  useEffect(() => {
    if (!HAS_TAURI) return;
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

  // Apply the data-palette attribute whenever the pref changes.
  // Each html[data-palette="<key>"] block in index.css overrides all liquid
  // tokens AND re-derives --ide-*-rgb channels so existing components retheme.
  // Default: "graphite-mist" (set in DEFAULT_PREFS and index.html).
  useEffect(() => {
    const p = palette ?? "graphite-mist";
    document.documentElement.setAttribute("data-palette", p);
  }, [palette]);

  // Apply the data-density attribute whenever the pref changes.
  // html[data-density="<v>"] in index.css scales --pad/--gap/--row-h/--radius
  // so the whole UI tightens/loosens. Default: "compact" (CopyPaste-52mz).
  useEffect(() => {
    document.documentElement.setAttribute("data-density", density ?? "compact");
  }, [density]);

  // Apply the data-theme attribute whenever the pref changes.
  // CSS custom property overrides in :root[data-theme="light"] take effect
  // immediately; no JS class toggling needed beyond setting this one attribute.
  //
  // theme:"system" follows the OS `prefers-color-scheme` LIVE — we resolve it
  // here via matchMedia and re-resolve when the OS preference flips (no manual
  // refresh). "light"/"dark" are applied verbatim. Dark-first default: an
  // absent pref resolves to "dark" (Graphite Mist default — CopyPaste-52mz).
  useEffect(() => {
    const resolve = (t: typeof theme): "light" | "dark" => {
      if (t === "dark" || t === "light") return t;
      // t === "system" (or undefined/legacy): follow the OS preference.
      if (t === "system" && typeof window !== "undefined" && window.matchMedia) {
        return window.matchMedia("(prefers-color-scheme: dark)").matches
          ? "dark"
          : "light";
      }
      // Graphite Mist is the new default — fall back to dark (CopyPaste-52mz).
      return "dark";
    };

    document.documentElement.setAttribute("data-theme", resolve(theme));

    // Only the "system" theme needs to react to OS-preference changes.
    if (theme !== "system" || typeof window === "undefined" || !window.matchMedia) {
      return;
    }
    const mql = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => {
      document.documentElement.setAttribute("data-theme", resolve("system"));
    };
    mql.addEventListener("change", onChange);
    return () => mql.removeEventListener("change", onChange);
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
    // Browser/mock has no Tauri event plugin — skip (audit P1-7).
    let unlisten: (() => void) | null = null;
    if (HAS_TAURI) {
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
    }

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
      {/*
        Floating shell layout — aurora is the fixed background (body gradient),
        all panels are floating glass cards inset ~10px from window edges with
        gaps between them so the aurora shows through. No edge-to-edge surfaces.
        The outer div is transparent so the aurora bleeds through every gap.
      */}
      <div className="flex h-screen w-screen gap-[10px] overflow-hidden p-[10px] text-ide-text">
        <Sidebar />
        {/* Main column: banners + view shell, all floating over the aurora */}
        <div className="flex min-w-0 flex-1 flex-col gap-[10px]">
          {/*
            Thin transparent drag strip covering the 10px gap row at the very top
            of the main column, between the window edge and the ViewShell header.
            This ensures the user can still drag the window by clicking in the top
            strip even when no banners are visible and the ViewShell header starts
            below the 10px inset. h-0 collapses it — the ViewShell header's own
            data-tauri-drag-region covers the draggable zone when rendered.
            The sidebar top drag region also covers the full left stripe so the
            main use case (drag the title bar area) still works.
          */}

          {/* Daemon spawn error — non-dismissible, installation-incomplete */}
          {daemonError !== null && (
            <div className="surface-glass flex shrink-0 items-start gap-3 rounded-ide-lg border border-red-500/40 px-3 py-2 text-[13px] text-red-400">
              <span className="shrink-0 font-semibold">Background service error:</span>
              <span>{daemonError}</span>
            </div>
          )}

          {showStaleBanner && (
            <div className="surface-glass flex shrink-0 items-start justify-between gap-3 rounded-ide-lg border border-ide-warning/40 px-3 py-2 text-[13px] text-ide-warning">
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
            <div className="surface-glass flex shrink-0 items-start justify-between gap-3 rounded-ide-lg border border-ide-warning/40 px-3 py-2 text-[13px] text-ide-warning">
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
            {/* ViewTransitionWrapper is keyed on the view id so React unmounts
                the old wrapper and mounts a new one on every tab switch,
                triggering the gentle crossfade (CopyPaste-2bhh).
                ErrorBoundary is inside the wrapper so a crash in one screen
                stays contained, and navigating away then back remounts a fresh
                non-crashed subtree. */}
            <ViewTransitionWrapper key={view} viewKey={view}>
              <ErrorBoundary label={label}>
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
            </ViewTransitionWrapper>
          </main>
        </div>
      </div>
    </ErrorBoundary>
  );
}

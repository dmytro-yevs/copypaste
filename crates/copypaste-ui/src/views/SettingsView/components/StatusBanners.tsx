// StatusBanners.tsx — extracted from SettingsView.tsx (CopyPaste-g06m.35)
// Renders the stale-daemon, not-ready, offline, degraded, and error banners
// that appear above the tab content in SettingsView.
import { RestartDaemonButton } from "../../../components/RestartDaemonButton";
import type { LoadState } from "../hooks/useSettingsState";

interface StatusBannersProps {
  loadState: LoadState;
  staleDaemon: string | null;
  degradedReason: string | null;
  onRetry: () => void;
}

export function StatusBanners({
  loadState,
  staleDaemon,
  degradedReason,
  onRetry,
}: StatusBannersProps) {
  const notReady = loadState === "not_ready";
  const degraded = loadState === "degraded";

  return (
    <>
      {/* Stale-daemon banner */}
      {staleDaemon !== null && (
        <div className="mb-4 flex items-start justify-between gap-3 border border-ide-warning/40 bg-ide-warning/5 px-3 py-2 text-[13px] text-ide-warning" style={{ borderRadius: "var(--r-ctl)" }}>
          <span>
            A previous CopyPaste background service is still running after an update
            {staleDaemon !== "unknown" ? ` (build ${staleDaemon})` : ""}. Restart
            it to use the latest version.
          </span>
          <RestartDaemonButton onRestarted={onRetry} />
        </div>
      )}

      {/* Not-ready banner (daemon alive but still initialising) */}
      {notReady && (
        <div className="surface-card mb-4 flex items-center justify-between gap-3 px-3 py-2 text-[13px] text-ide-dim shadow-ide-xs" style={{ borderRadius: "var(--r-card)" }}>
          <span>Clipboard service is starting up — settings will be available in a moment.</span>
          <button
            type="button"
            onClick={onRetry}
            className="shrink-0 border border-ide-border bg-ide-panel px-2.5 py-1 text-[12px] text-ide-text hover:bg-ide-raised hover:text-ide-text shadow-ide-xs"
            style={{ borderRadius: "var(--r-ctl)" }}
          >
            Retry
          </button>
        </div>
      )}

      {/* Offline banner — sticky so it stays visible when the user scrolls past it,
          providing context for why all controls are disabled (bdac.12). */}
      {loadState === "offline" && (
        <div className="surface-card mb-4 flex items-center justify-between gap-3 px-3 py-2 text-[13px] text-ide-dim shadow-ide-xs" style={{ borderRadius: "var(--r-card)", position: "sticky", top: 0, zIndex: 10 }}>
          <span>Background service not running — clipboard sync paused.</span>
          <div className="flex shrink-0 items-center gap-2">
            <RestartDaemonButton
              label="Restart"
              onRestarted={onRetry}
            />
            <button
              type="button"
              onClick={onRetry}
              className="shrink-0 border border-ide-border bg-ide-panel px-2.5 py-1 text-[12px] text-ide-text hover:bg-ide-raised hover:text-ide-text shadow-ide-xs"
              style={{ borderRadius: "var(--r-ctl)" }}
            >
              Retry
            </button>
          </div>
        </div>
      )}

      {/* Degraded banner */}
      {degraded && (
        <div className="mb-4 flex items-start justify-between gap-3 border border-ide-warning/40 bg-ide-warning/5 px-3 py-2 text-[13px] text-ide-warning" style={{ borderRadius: "var(--r-ctl)" }}>
          <span>
            Clipboard database unavailable
            {degradedReason ? ` (${degradedReason})` : ""} — its key no longer
            matches. Open History to reset the database and recover.
          </span>
          <div className="flex shrink-0 items-center gap-2">
            <button
              type="button"
              onClick={onRetry}
              className={[
                "border border-ide-warning/40 bg-ide-panel px-2.5 py-1 text-[12px] text-ide-warning",
                "hover:bg-ide-hover",
              ].join(" ")}
              style={{ borderRadius: "var(--r-ctl)" }}
            >
              Retry
            </button>
            <RestartDaemonButton onRestarted={onRetry} />
          </div>
        </div>
      )}

      {/* tk2j: Error banner — daemon is reachable but settings could not be loaded */}
      {loadState === "error" && (
        <div className="surface-card mb-4 flex items-center justify-between gap-3 px-3 py-2 text-[13px] text-ide-dim shadow-ide-xs" style={{ borderRadius: "var(--r-card)" }}>
          <span>Failed to load settings — the background service is running but returned an error.</span>
          <div className="flex shrink-0 items-center gap-2">
            <RestartDaemonButton
              label="Restart"
              onRestarted={onRetry}
            />
            <button
              type="button"
              onClick={onRetry}
              className="shrink-0 border border-ide-border bg-ide-panel px-2.5 py-1 text-[12px] text-ide-text hover:bg-ide-raised hover:text-ide-text shadow-ide-xs"
              style={{ borderRadius: "var(--r-ctl)" }}
            >
              Retry
            </button>
          </div>
        </div>
      )}
    </>
  );
}

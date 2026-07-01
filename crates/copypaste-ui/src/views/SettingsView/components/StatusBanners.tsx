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
        <div>
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
        <div>
          <span>Clipboard service is starting up — settings will be available in a moment.</span>
          <button type="button" onClick={onRetry}>
            Retry
          </button>
        </div>
      )}

      {/* Offline banner — sticky so it stays visible when the user scrolls past it,
          providing context for why all controls are disabled (bdac.12). */}
      {loadState === "offline" && (
        <div>
          <span>Background service not running — clipboard sync paused.</span>
          <div>
            <RestartDaemonButton
              label="Restart"
              onRestarted={onRetry}
            />
            <button type="button" onClick={onRetry}>
              Retry
            </button>
          </div>
        </div>
      )}

      {/* Degraded banner */}
      {degraded && (
        <div>
          <span>
            Clipboard database unavailable
            {degradedReason ? ` (${degradedReason})` : ""} — its key no longer
            matches. Open History to reset the database and recover.
          </span>
          <div>
            <button type="button" onClick={onRetry}>
              Retry
            </button>
            <RestartDaemonButton onRestarted={onRetry} />
          </div>
        </div>
      )}

      {/* tk2j: Error banner — daemon is reachable but settings could not be loaded */}
      {loadState === "error" && (
        <div>
          <span>Failed to load settings — the background service is running but returned an error.</span>
          <div>
            <RestartDaemonButton
              label="Restart"
              onRestarted={onRetry}
            />
            <button type="button" onClick={onRetry}>
              Retry
            </button>
          </div>
        </div>
      )}
    </>
  );
}

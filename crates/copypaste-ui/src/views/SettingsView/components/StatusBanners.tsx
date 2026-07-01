// StatusBanners.tsx — extracted from SettingsView.tsx (CopyPaste-g06m.35)
// Renders the stale-daemon, not-ready, offline, degraded, and error banners
// that appear above the tab content in SettingsView.
import { AlertTriangle, Info, XCircle } from "lucide-react";
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
      {/* Stale-daemon banner — non-dismissible (no dismiss handler is wired here). */}
      {staleDaemon !== null && (
        <div role="alert" className="banner banner--warn">
          <AlertTriangle aria-hidden="true" />
          <span className="banner__x">
            A previous CopyPaste background service is still running after an update
            {staleDaemon !== "unknown" ? ` (build ${staleDaemon})` : ""}. Restart
            it to use the latest version.
          </span>
          <span className="banner__act">
            <RestartDaemonButton onRestarted={onRetry} />
          </span>
        </div>
      )}

      {/* Not-ready banner (daemon alive but still initialising) — informational. */}
      {notReady && (
        <div role="status" className="banner banner--info">
          <Info aria-hidden="true" />
          <span className="banner__x">Clipboard service is starting up — settings will be available in a moment.</span>
          <span className="banner__act">
            <button type="button" className="btn btn--secondary" onClick={onRetry}>
              Retry
            </button>
          </span>
        </div>
      )}

      {/* Offline banner — sticky so it stays visible when the user scrolls past it,
          providing context for why all controls are disabled (bdac.12). */}
      {loadState === "offline" && (
        <div role="alert" className="banner banner--err">
          <XCircle aria-hidden="true" />
          <span className="banner__x">Background service not running — clipboard sync paused.</span>
          <span className="banner__act">
            <RestartDaemonButton
              label="Restart"
              onRestarted={onRetry}
            />
            <button type="button" className="btn btn--secondary" onClick={onRetry}>
              Retry
            </button>
          </span>
        </div>
      )}

      {/* Degraded banner */}
      {degraded && (
        <div role="alert" className="banner banner--err">
          <XCircle aria-hidden="true" />
          <span className="banner__x">
            Clipboard database unavailable
            {degradedReason ? ` (${degradedReason})` : ""} — its key no longer
            matches. Open History to reset the database and recover.
          </span>
          <span className="banner__act">
            <button type="button" className="btn btn--secondary" onClick={onRetry}>
              Retry
            </button>
            <RestartDaemonButton onRestarted={onRetry} />
          </span>
        </div>
      )}

      {/* tk2j: Error banner — daemon is reachable but settings could not be loaded */}
      {loadState === "error" && (
        <div role="alert" className="banner banner--err">
          <XCircle aria-hidden="true" />
          <span className="banner__x">Failed to load settings — the background service is running but returned an error.</span>
          <span className="banner__act">
            <RestartDaemonButton
              label="Restart"
              onRestarted={onRetry}
            />
            <button type="button" className="btn btn--secondary" onClick={onRetry}>
              Retry
            </button>
          </span>
        </div>
      )}
    </>
  );
}

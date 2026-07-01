import { useCallback, useState } from "react";
import { restartDaemon } from "../lib/ipc";

type Phase = "idle" | "restarting" | "ok" | "error";

/**
 * "Restart daemon" control. Calls the Tauri `restart_daemon` command, which
 * stops the app-tracked child process (SIGTERM + reap) and spawns the bundled
 * `copypaste-daemon` binary fresh via `ensure_daemon_running`. No launchctl
 * involved — the app owns the daemon lifecycle entirely.
 *
 * Works even when the daemon is degraded/unresponsive (it operates on the
 * tracked child handle directly, not the daemon IPC socket). Surfaces
 * success/failure loudly via an inline status message so a stale-daemon
 * recovery is never silent.
 */
export function RestartDaemonButton({
  label = "Restart background service",
  onRestarted,
}: {
  label?: string;
  className?: string;
  /** Called after a successful restart so the host view can re-check status. */
  onRestarted?: () => void;
}) {
  const [phase, setPhase] = useState<Phase>("idle");
  const [message, setMessage] = useState<string | null>(null);

  const handleClick = useCallback(async () => {
    setPhase("restarting");
    setMessage(null);
    try {
      await restartDaemon();
      setPhase("ok");
      setMessage("Background service restarted.");
      onRestarted?.();
    } catch (err) {
      setPhase("error");
      setMessage(err instanceof Error ? err.message : "Restart failed.");
    }
  }, [onRestarted]);

  return (
    <div className="ctl">
      <button
        type="button"
        className="btn btn--secondary sm"
        disabled={phase === "restarting"}
        onClick={() => void handleClick()}
      >
        {phase === "restarting" ? "Restarting…" : label}
      </button>
      {message !== null && (
        <span
          role="status"
          className={`field-note ${phase === "error" ? "field-note--err" : "field-note--ok"}`}
        >
          {message}
        </span>
      )}
    </div>
  );
}

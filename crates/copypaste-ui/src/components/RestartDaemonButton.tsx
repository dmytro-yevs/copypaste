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
  className,
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
    <div className="flex flex-col gap-1">
      <button
        type="button"
        disabled={phase === "restarting"}
        onClick={() => void handleClick()}
        className={
          className ??
          [
            // rounded-ide removed; borderRadius driven by --skin-r-ctl (inline style below).
            "shrink-0 border border-ide-border bg-ide-panel px-2.5 py-1 text-[12px] text-ide-text",
            "hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40",
          ].join(" ")
        }
        style={{ borderRadius: "var(--skin-r-ctl)" }}
      >
        {phase === "restarting" ? "Restarting…" : label}
      </button>
      {message !== null && (
        <span
          role="status"
          className={[
            "text-[11px]",
            phase === "error" ? "text-ide-danger" : "text-ide-dim",
          ].join(" ")}
        >
          {message}
        </span>
      )}
    </div>
  );
}

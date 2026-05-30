import { useCallback, useState } from "react";
import { restartDaemon } from "../lib/ipc";

type Phase = "idle" | "restarting" | "ok" | "error";

/**
 * "Restart daemon" control. Calls the Tauri `restart_daemon` command
 * (launchctl kickstart -k, with bootout+bootstrap fallback) so the running
 * daemon is forced to the freshly-installed binary without a reboot.
 *
 * Works even when the daemon is degraded/unresponsive (it talks to launchctl,
 * not the daemon IPC). Surfaces success/failure loudly via an inline status
 * message so a stale-daemon recovery is never silent.
 */
export function RestartDaemonButton({
  label = "Restart daemon",
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
      setMessage("Daemon restarted — using the latest installed build.");
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
            "shrink-0 rounded-ide border border-ide-border bg-ide-panel px-2.5 py-1 text-[12px] text-ide-text",
            "hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40",
          ].join(" ")
        }
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

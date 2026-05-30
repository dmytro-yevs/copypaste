import { useEffect, useState } from "react";
import { ViewShell } from "../components/ViewShell";
import { api, IpcError } from "../lib/ipc";

const FEATURES = [
  "End-to-end encrypted local history",
  "Peer-to-peer device sync",
  "Automatic sensitive-data redaction",
];

export function AboutView() {
  const [daemonStatus, setDaemonStatus] = useState<"pending" | "connected" | "offline">("pending");

  useEffect(() => {
    api
      .status()
      .then(() => setDaemonStatus("connected"))
      .catch((e: unknown) => {
        if (e instanceof IpcError) {
          setDaemonStatus("offline");
        } else {
          setDaemonStatus("offline");
        }
      });
  }, []);

  return (
    <ViewShell title="About">
      <div className="flex h-full flex-col items-center justify-center">
        <div className="flex w-full max-w-sm flex-col gap-6 rounded-ide bg-ide-panel px-8 py-8 border border-ide-border">

          {/* Identity */}
          <div className="flex flex-col items-center gap-1 text-center">
            <h2 className="text-xl font-semibold text-ide-text">CopyPaste</h2>
            <span className="text-[13px] text-ide-faint">0.4.1</span>
            <p className="mt-1 text-[13px] text-ide-dim">Encrypted clipboard manager for macOS</p>
          </div>

          {/* Feature list */}
          <ul className="flex flex-col gap-2">
            {FEATURES.map((feature) => (
              <li key={feature} className="flex items-start gap-2 text-[13px] text-ide-dim">
                <span className="mt-px text-ide-faint select-none">•</span>
                {feature}
              </li>
            ))}
          </ul>

          {/* Daemon status */}
          <div className="text-[13px]">
            {daemonStatus === "pending" && (
              <span className="text-ide-faint">Daemon: checking…</span>
            )}
            {daemonStatus === "connected" && (
              <span className="text-ide-success">Daemon: connected</span>
            )}
            {daemonStatus === "offline" && (
              <span className="text-ide-danger">Daemon: offline</span>
            )}
          </div>

          {/* GitHub link — use window.open so Tauri opens in the system browser.
              URL confirmed from git remote: github.com/dmytro-yevs/copypaste */}
          <button
            type="button"
            onClick={() => window.open("https://github.com/dmytro-yevs/copypaste", "_blank")}
            className="text-left text-[13px] text-ide-accent hover:underline cursor-pointer bg-transparent border-0 p-0"
          >
            github.com/dmytro-yevs/copypaste ↗
          </button>

        </div>
      </div>
    </ViewShell>
  );
}

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
      <div className="flex h-full flex-col items-center justify-center px-6">
        <div className="flex w-full max-w-sm flex-col gap-0 overflow-hidden rounded-ide-lg border border-ide-border bg-ide-elevated shadow-ide-sm">

          {/* Identity */}
          <div className="flex flex-col items-center gap-1 border-b border-ide-divider px-8 py-6 text-center">
            <h2 className="text-[17px] font-semibold text-ide-text">CopyPaste</h2>
            <span className="text-[12px] text-ide-faint">v0.5.2</span>
            <p className="mt-1.5 text-[13px] text-ide-dim">Encrypted clipboard manager for macOS</p>
          </div>

          {/* Feature list */}
          <div className="border-b border-ide-divider px-6 py-4">
            <p className="mb-2 text-[10px] font-semibold uppercase tracking-wider text-ide-accent/80">
              Features
            </p>
            <ul className="flex flex-col gap-1.5">
              {FEATURES.map((feature) => (
                <li key={feature} className="flex items-start gap-2 text-[13px] text-ide-dim">
                  <span className="mt-px shrink-0 select-none text-ide-accent">✓</span>
                  {feature}
                </li>
              ))}
            </ul>
          </div>

          {/* Daemon status */}
          <div className="flex items-center justify-between border-b border-ide-divider px-6 py-3">
            <span className="text-[13px] text-ide-dim">Background daemon</span>
            <span className="text-[13px]">
              {daemonStatus === "pending" && (
                <span className="text-ide-faint">Checking…</span>
              )}
              {daemonStatus === "connected" && (
                <span className="text-ide-success">Connected ✓</span>
              )}
              {daemonStatus === "offline" && (
                <span className="text-ide-danger">Offline</span>
              )}
            </span>
          </div>

          {/* GitHub link — use window.open so Tauri opens in the system browser */}
          <div className="px-6 py-3">
            <button
              type="button"
              onClick={() => window.open("https://github.com/dmytro/CopyPaste", "_blank")}
              className="cursor-pointer border-0 bg-transparent p-0 text-[13px] text-ide-accent transition-colors hover:text-ide-accent-hover hover:underline"
            >
              github.com/dmytro/CopyPaste ↗
            </button>
          </div>

        </div>
      </div>
    </ViewShell>
  );
}

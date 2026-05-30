import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { ViewShell } from "../components/ViewShell";
import { probeStatus, type StatusProbe } from "../lib/ipc";

const FEATURES = [
  "End-to-end encrypted local history",
  "Peer-to-peer device sync",
  "Automatic sensitive-data redaction",
];

// Daemon status as surfaced in this view:
//  - "pending":   probe in flight
//  - "connected": daemon up and its DB is usable
//  - "degraded":  daemon up but DB unavailable (carries a reason)
//  - "offline":   daemon unreachable
type DaemonView =
  | { kind: "pending" }
  | { kind: "connected" }
  | { kind: "degraded"; reason: string | null }
  | { kind: "offline" };

export function AboutView() {
  const [daemon, setDaemon] = useState<DaemonView>({ kind: "pending" });
  // Real app version, pulled at runtime from the Tauri bundle (tauri.conf.json)
  // instead of a hardcoded string that drifts out of date on every release.
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    // A reachable socket alone does NOT mean the daemon is healthy: probeStatus
    // distinguishes a usable DB ("ok") from a degraded daemon whose DB cannot be
    // opened ("degraded") from an unreachable daemon ("offline"). Previously this
    // view showed green "connected" for ANY resolving status call, masking the
    // degraded case entirely.
    probeStatus().then((probe: StatusProbe) => {
      if (cancelled) return;
      if (probe.kind === "ok") setDaemon({ kind: "connected" });
      else if (probe.kind === "degraded")
        setDaemon({ kind: "degraded", reason: probe.reason });
      else setDaemon({ kind: "offline" });
    });

    getVersion().then(
      (v) => {
        if (!cancelled) setVersion(v);
      },
      () => {
        // Tauri command unavailable (e.g. running outside the bundle / in tests).
        // Leave version null rather than show a stale hardcoded number.
      }
    );

    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <ViewShell title="About">
      <div className="flex h-full flex-col items-center justify-center">
        <div className="flex w-full max-w-sm flex-col gap-6 rounded-ide bg-ide-panel px-8 py-8 border border-ide-border">

          {/* Identity */}
          <div className="flex flex-col items-center gap-1 text-center">
            <h2 className="text-xl font-semibold text-ide-text">CopyPaste</h2>
            <span className="text-[13px] text-ide-faint">{version ?? "—"}</span>
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

          {/* Daemon status — distinct degraded state, never a false green. */}
          <div className="text-[13px]">
            {daemon.kind === "pending" && (
              <span className="text-ide-faint">Daemon: checking…</span>
            )}
            {daemon.kind === "connected" && (
              <span className="text-ide-success">Daemon: connected</span>
            )}
            {daemon.kind === "degraded" && (
              <span className="text-ide-warning">
                Daemon: degraded
                {daemon.reason ? ` (${daemon.reason})` : ""} — clipboard database
                unavailable. Open History to reset it.
              </span>
            )}
            {daemon.kind === "offline" && (
              <span className="text-ide-danger">Daemon: offline</span>
            )}
          </div>

          {/* GitHub link — use window.open so Tauri opens in the system browser */}
          <button
            type="button"
            onClick={() => window.open("https://github.com/dmytro/CopyPaste", "_blank")}
            className="text-left text-[13px] text-ide-accent hover:underline cursor-pointer bg-transparent border-0 p-0"
          >
            github.com/dmytro/CopyPaste ↗
          </button>

        </div>
      </div>
    </ViewShell>
  );
}

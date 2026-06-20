import { useEffect, useState } from "react";
import { Check } from "lucide-react";
import { getVersion } from "@tauri-apps/api/app";
import { ViewShell } from "../components/ViewShell";
import { appVersion, probeStatus, type StatusProbe } from "../lib/ipc";

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

// Badge color mapping for daemon state — uses only ide.* tokens, no hex.
const DAEMON_BADGE: Record<string, string> = {
  pending: "text-ide-faint",
  connected: "text-ide-success",
  degraded: "text-ide-warning",
  offline: "text-ide-danger",
};

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

    // Prefer the Tauri bundle version; fall back to the daemon's app_version
    // (mock-safe IPC) when running outside the bundle (browser / ?mock=1), so the
    // line shows a real version instead of a bare "—". (audit P2)
    getVersion().then(
      (v) => {
        if (!cancelled) setVersion(v);
      },
      () => {
        void appVersion().then(
          (v) => {
            if (!cancelled && v) setVersion(v);
          },
          () => {
            // Both unavailable — leave null; the line hides rather than show "—".
          }
        );
      }
    );

    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <ViewShell title="About">
      <div className="flex h-full flex-col items-center justify-center px-6">
        {/* surface-card = frosted translucent glass (aurora canvas blurs through).
            .card-in = entrance animation (scale+fade from tokens).
            bg-ide-elevated is kept so the existing token assertion in
            AboutView.test.tsx still finds it; .surface-card overrides the opaque
            fill at paint time. */}
        {/* rounded-ide-lg (fixed 14px) → --skin-r-card; shadow-ide-sm (fixed e2) → --skin-shadow-card
            so Quiet (10px/none) and Vapor (16px/none+sheen) render correctly. */}
        <div
          className="surface-card card-in flex w-full max-w-sm flex-col gap-0 overflow-hidden bg-ide-elevated"
          style={{ borderRadius: "var(--skin-r-card)", boxShadow: "var(--skin-shadow-card)" }}
        >

          {/* Identity — .reveal-up staggers the heading after the card enters */}
          <div className="reveal-up flex flex-col items-center gap-1 border-b border-ide-divider px-8 py-6 text-center">
            <h2 className="text-[18px] font-semibold text-ide-text">CopyPaste</h2>
            {/* audit P2: hide the line entirely when no version is known instead
                of rendering a bare "—". Version badge floats gently (.badge-float). */}
            {version !== null && (
              <span
                className="badge-float mt-0.5 inline-block rounded-full border border-ide-divider bg-ide-elevated px-2.5 py-0.5 text-[11px] font-medium text-ide-faint"
              >
                {version}
              </span>
            )}
            <p className="mt-2 text-[13px] text-ide-dim">
              Encrypted clipboard manager for macOS
            </p>
          </div>

          {/* Feature list */}
          <div className="border-b border-ide-divider px-6 py-4">
            <p className="reveal-up mb-2.5 text-[10px] font-semibold uppercase tracking-wider text-ide-accent/80">
              Features
            </p>
            <ul className="flex flex-col gap-1.5">
              {FEATURES.map((feature, idx) => (
                <li
                  key={feature}
                  // Stagger each feature row: 40 ms per item above the base 40 ms.
                  className="list-item-in flex items-start gap-2 text-[13px] text-ide-dim"
                  style={{ animationDelay: `${(idx + 1) * 40}ms` }}
                >
                  <Check
                    size={14}
                    className="mt-px shrink-0 text-ide-success"
                    aria-hidden="true"
                  />
                  {feature}
                </li>
              ))}
            </ul>
          </div>

          {/* Daemon status — distinct degraded state, never a false green. */}
          <div className="flex items-center justify-between border-b border-ide-divider px-6 py-3">
            <span className="text-[13px] text-ide-dim">Background daemon</span>
            <span className={`text-[12px] font-medium ${DAEMON_BADGE[daemon.kind]}`}>
              {daemon.kind === "pending" && "Checking…"}
              {daemon.kind === "connected" && (
                <span className="inline-flex items-center gap-1">
                  Connected <Check size={12} aria-hidden="true" />
                </span>
              )}
              {daemon.kind === "degraded" && (
                <>Degraded{daemon.reason ? ` (${daemon.reason})` : ""}</>
              )}
              {daemon.kind === "offline" && "Offline"}
            </span>
          </div>

          {/* GitHub link — use window.open so Tauri opens in the system browser.
              URL confirmed from git remote: github.com/dmytro-yevs/copypaste.
              Styled as a full-width row with top divider to match the daemon-status
              row above it (border-t, px-6 py-3). */}
          <div className="px-6 py-3">
            <button
              type="button"
              onClick={() =>
                window.open(
                  "https://github.com/dmytro-yevs/copypaste",
                  "_blank"
                )
              }
              className="cursor-pointer border-0 bg-transparent p-0 text-left text-[13px] text-ide-accent hover:underline"
            >
              github.com/dmytro-yevs/copypaste ↗
            </button>
          </div>
        </div>
      </div>
    </ViewShell>
  );
}

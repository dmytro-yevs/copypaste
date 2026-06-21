import { useCallback, useEffect, useState } from "react";
import { Check } from "lucide-react";
import { getVersion } from "@tauri-apps/api/app";
import { ViewShell } from "../components/ViewShell";
import { SectionHeader } from "../components/SectionHeader";
import { RestartDaemonButton } from "../components/RestartDaemonButton";
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

// GitHub base for external links. Changelog and privacy policy are linked
// directly to their repo paths since no separate hosted URLs exist.
const GITHUB_BASE = "https://github.com/dmytro-yevs/copypaste";

export function AboutView() {
  const [daemon, setDaemon] = useState<DaemonView>({ kind: "pending" });
  // Real app version, pulled at runtime from the Tauri bundle (tauri.conf.json)
  // instead of a hardcoded string that drifts out of date on every release.
  const [version, setVersion] = useState<string | null>(null);

  // bdac.98: extracted so RestartDaemonButton can call it via onRestarted to
  // re-check daemon health after a restart without unmounting the component.
  const checkStatus = useCallback(() => {
    setDaemon({ kind: "pending" });
    // A reachable socket alone does NOT mean the daemon is healthy: probeStatus
    // distinguishes a usable DB ("ok") from a degraded daemon whose DB cannot be
    // opened ("degraded") from an unreachable daemon ("offline"). Previously this
    // view showed green "connected" for ANY resolving status call, masking the
    // degraded case entirely.
    probeStatus().then((probe: StatusProbe) => {
      if (probe.kind === "ok") setDaemon({ kind: "connected" });
      else if (probe.kind === "degraded")
        setDaemon({ kind: "degraded", reason: probe.reason });
      else setDaemon({ kind: "offline" });
    });
  }, []);

  useEffect(() => {
    let cancelled = false;

    checkStatus();

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
  }, [checkStatus]);

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

          {/* Identity — .reveal-up staggers the heading after the card enters.
              UIC-11: unified to px-6 to match all other sections (was px-8). */}
          <div className="reveal-up flex flex-col items-center gap-1 border-b border-ide-divider px-6 py-6 text-center">
            <h2 className="text-[18px] font-semibold text-ide-text">CopyPaste</h2>
            {/* audit P2: hide the line entirely when no version is known instead
                of rendering a bare "—". Version badge floats gently (.badge-float).
                bdac.77: Android shows "VERSION_NAME (build VERSION_CODE)". macOS shows
                only the version name — Tauri's getVersion() / app_version IPC expose no
                build number, so parity is achieved at the version-name level only. */}
            {version !== null && (
              <span
                className="badge-float mt-0.5 inline-block rounded-full border border-ide-divider bg-ide-elevated px-2.5 py-0.5 text-[11px] font-medium text-ide-faint"
              >
                {version}
              </span>
            )}
            {/* bdac.79: canonical short tagline — no platform suffix (matches Android about_tagline). */}
            <p className="mt-2 text-[13px] text-ide-dim">
              Encrypted clipboard manager
            </p>
          </div>

          {/* Feature list — UIC-2/VISM-13: replaced hand-rolled accent-coloured
              label with shared SectionHeader (11px, text-ide-dim, non-accent). */}
          <div className="border-b border-ide-divider px-6 py-4">
            <SectionHeader label="Features" />
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
          {/* SCRL-5: aria-live="polite" so screen readers announce status changes
              when the async probe resolves (bd CopyPaste-5917.89). */}
          <div className="border-b border-ide-divider px-6 py-3">
            <div className="flex items-center justify-between">
              <span className="text-[13px] text-ide-dim">Background daemon</span>
              <span aria-live="polite" className={`text-[12px] font-medium ${DAEMON_BADGE[daemon.kind]}`}>
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
            {/* bdac.98: offer RestartDaemonButton in degraded/offline states so users
                can recover without navigating away — consistent with all other views. */}
            {(daemon.kind === "offline" || daemon.kind === "degraded") && (
              <div className="mt-2">
                <RestartDaemonButton onRestarted={checkStatus} />
              </div>
            )}
          </div>

          {/* External links — SCRL-6: changelog and privacy policy added alongside
              the GitHub link. Both targets exist in-repo (CHANGELOG.md,
              docs/privacy/telemetry-policy.md); linked via GitHub since no separate
              hosted URLs exist. window.open used so Tauri opens the system browser. */}
          <div className="flex flex-col gap-0 px-6 py-3">
            <button
              type="button"
              onClick={() =>
                window.open(GITHUB_BASE, "_blank")
              }
              className="cursor-pointer border-0 bg-transparent p-0 text-left text-[13px] text-ide-accent hover:underline"
            >
              github.com/dmytro-yevs/copypaste ↗
            </button>
            <button
              type="button"
              onClick={() =>
                window.open(`${GITHUB_BASE}/blob/main/CHANGELOG.md`, "_blank")
              }
              className="mt-1.5 cursor-pointer border-0 bg-transparent p-0 text-left text-[13px] text-ide-accent hover:underline"
            >
              Changelog ↗
            </button>
            <button
              type="button"
              onClick={() =>
                window.open(
                  `${GITHUB_BASE}/blob/main/docs/privacy/telemetry-policy.md`,
                  "_blank"
                )
              }
              className="mt-1.5 cursor-pointer border-0 bg-transparent p-0 text-left text-[13px] text-ide-accent hover:underline"
            >
              Privacy policy ↗
            </button>
          </div>
        </div>
      </div>
    </ViewShell>
  );
}

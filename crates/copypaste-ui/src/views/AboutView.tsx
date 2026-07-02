import { useCallback, useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { Clipboard, ExternalLink } from "lucide-react";
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

// GitHub base for external links. Changelog and privacy policy are linked
// directly to their repo paths since no separate hosted URLs exist.
const GITHUB_BASE = "https://github.com/dmytro-yevs/copypaste";

export function AboutContent() {
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
    probeStatus()
      .then((probe: StatusProbe) => {
        if (probe.kind === "ok") setDaemon({ kind: "connected" });
        else if (probe.kind === "degraded")
          setDaemon({ kind: "degraded", reason: probe.reason });
        else setDaemon({ kind: "offline" });
      })
      // CopyPaste-crh3.116: without this, a network/IPC rejection is unhandled
      // and the view is stuck on {kind:"pending"} (spinner never resolves).
      .catch(() => setDaemon({ kind: "offline" }));
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
    <div className="about">
        {/* Identity */}
        <div className="about__logo">
          <Clipboard aria-hidden="true" />
        </div>
        <h2 className="about__name">CopyPaste</h2>
        {/* audit P2: hide the line entirely when no version is known instead
            of rendering a bare "—".
            bdac.77: Android shows "VERSION_NAME (build VERSION_CODE)". macOS shows
            only the version name — Tauri's getVersion() / app_version IPC expose no
            build number, so parity is achieved at the version-name level only. */}
        {version !== null && (
          <span className="about__ver">
            {version}
          </span>
        )}
        {/* bdac.79: canonical short tagline — no platform suffix (matches Android about_tagline). */}
        <p>
          Encrypted clipboard manager
        </p>

        {/* Feature list */}
        <div>
          <SectionHeader label="Features" />
          <ul className="about__features">
            {FEATURES.map((feature) => (
              <li key={feature}>
                {feature}
              </li>
            ))}
          </ul>
        </div>

        {/* Daemon status — distinct degraded state, never a false green. */}
        {/* SCRL-5: aria-live="polite" so screen readers announce status changes
            when the async probe resolves (bd CopyPaste-5917.89). */}
        <dl className="about__grid">
          <dt>Background daemon</dt>
          <dd aria-live="polite">
            {daemon.kind === "pending" && "Checking…"}
            {daemon.kind === "connected" && "Connected"}
            {daemon.kind === "degraded" && (
              <>Degraded{daemon.reason ? ` (${daemon.reason})` : ""}</>
            )}
            {daemon.kind === "offline" && "Offline"}
          </dd>
        </dl>
        {/* bdac.98: offer RestartDaemonButton in degraded/offline states so users
            can recover without navigating away — consistent with all other views. */}
        {(daemon.kind === "offline" || daemon.kind === "degraded") && (
          <div>
            <RestartDaemonButton onRestarted={checkStatus} />
          </div>
        )}

        {/* External links — SCRL-6: changelog and privacy policy added alongside
            the GitHub link. Both targets exist in-repo (CHANGELOG.md,
            docs/privacy/telemetry-policy.md); linked via GitHub since no separate
            hosted URLs exist. window.open used so Tauri opens the system browser. */}
        <div className="about__links">
          <button
            type="button"
            className="btn sm btn--secondary"
            onClick={() =>
              window.open(GITHUB_BASE, "_blank")
            }
          >
            github.com/dmytro-yevs/copypaste <ExternalLink aria-hidden="true" />
          </button>
          <button
            type="button"
            className="btn sm btn--secondary"
            onClick={() =>
              window.open(`${GITHUB_BASE}/blob/main/CHANGELOG.md`, "_blank")
            }
          >
            Changelog <ExternalLink aria-hidden="true" />
          </button>
          <button
            type="button"
            className="btn sm btn--secondary"
            onClick={() =>
              window.open(
                `${GITHUB_BASE}/blob/main/docs/privacy/telemetry-policy.md`,
                "_blank"
              )
            }
          >
            Privacy policy <ExternalLink aria-hidden="true" />
          </button>
        </div>
      </div>
  );
}
